//! 같은 PC의 체크포인트로 중단된 서버 세션을 복원함.

use super::*;
use runner_core::recovery::{Checkpoint, RecoveryContext, RecoveryStore};

const MISSING: &str = "이 PC에는 해당 실행의 복구 기록이 없어요. 매크로를 실행했던 PC에서 다시 연결하세요. 새 실행으로 대신 시작하지 않았어요.";

impl Shared {
    pub(crate) fn recovery_context(&self, recovery_id: &str) -> RecoveryContext {
        RecoveryContext {
            store: RecoveryStore::new(self.data_dir.join("recovery.sqlite")),
            recovery_id: recovery_id.into(),
        }
    }

    pub fn resume_run(self: &S, id: &str) -> StartReply {
        let blocked = |message: &str| StartReply::Blocked { message: message.into() };
        let (identity, creds, base, local_generation, fingerprint) = {
            let mut c = self.lock();
            let Some(r) = c.run(id) else { return blocked(MISSING) };
            if c.closed || c.claim_busy || r.busy() {
                return blocked("다른 작업을 마친 뒤 다시 이어서 실행하세요.");
            }
            if !r.interrupted() {
                return blocked("중단된 실행만 이어서 실행할 수 있어요.");
            }
            let Some(identity) = r.recovery.clone() else { return blocked(MISSING) };
            let (ready, why, _) = c.readiness(r);
            if !ready {
                return blocked(&why);
            }
            let job = (identity, c.run_credentials(r), r.server_base.clone(), r.generation, c.run_fingerprint(r));
            c.run_mut(id).unwrap().start_pending = true;
            job
        };
        self.emit();
        let me = self.clone();
        let id = id.to_string();
        tauri::async_runtime::spawn(async move {
            me.resume_async(id, identity, creds, base, local_generation, fingerprint).await;
        });
        StartReply::Started
    }

    async fn resume_async(
        self: S,
        id: String,
        identity: crate::state::SavedRecovery,
        creds: Credentials,
        base: String,
        local_generation: u64,
        fingerprint: String,
    ) {
        let context = self.recovery_context(&identity.recovery_id);
        let checkpoint = match context.store.load(&identity.recovery_id) {
            Ok(Some(cp)) if checkpoint_matches(&cp, &identity, &base) => cp,
            _ => {
                self.resume_failed(&id, MISSING);
                return;
            }
        };
        let server = ServerClient::new(&creds.member_key, &base, self.outbox.clone(), self.http.clone());
        let prepared =
            match runner_core::bot::prepare_resume(checkpoint, creds.clone(), self.http.clone(), server, context).await
            {
                Ok(prepared) => prepared,
                Err(why) => {
                    self.resume_failed(&id, &connection::safe_preflight_reason(&why, &creds));
                    return;
                }
            };
        let cp = prepared.checkpoint();
        let next_identity = crate::state::SavedRecovery {
            session_id: cp.session_id,
            recovery_id: cp.recovery_id.clone(),
            generation: cp.generation,
        };
        let snapshot = cp.snapshot();
        let macro_ = cp.macro_.clone();
        let mode = cp.mode;
        let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
        let saved = {
            let mut c = self.lock();
            let valid = c.launch_attach_valid(&id, local_generation, &fingerprint, &creds.member_key);
            if !valid {
                drop(c);
                self.resume_failed(&id, "복구 중 실행 설정이 바뀌어 멈췄어요. 상태를 확인하고 다시 시도하세요.");
                return;
            }
            let r = c.run_mut(&id).unwrap();
            r.recovery = Some(next_identity);
            self.save_runs(&c)
        };
        if !saved {
            self.warn_save_failed(false);
            self.resume_failed(
                &id,
                "복구 정보를 저장하지 못해 매매를 재개하지 않았어요. 디스크 공간과 폴더 권한을 확인하세요.",
            );
            return;
        }
        {
            let mut c = self.lock();
            if !c.launch_attach_valid(&id, local_generation, &fingerprint, &creds.member_key) {
                drop(c);
                self.resume_failed(&id, "복구 중 실행 설정이 바뀌어 멈췄어요. 상태를 확인하고 다시 시도하세요.");
                return;
            }
            let Some(r) = c.run_mut(&id) else { return };
            r.macro_ = macro_;
            r.mode = mode;
            r.bot = Some(prepared.spawn(tx));
            r.start_pending = false;
            r.stopping = false;
            if let Some(d) = r.data.as_mut() {
                d.finished = None;
                d.snapshot = snapshot;
            }
            c.push_activity(
                &id,
                "start",
                "중단된 실행을 이어서 시작했어요",
                "같은 세션의 보유분과 주문 기록을 확인했어요.",
                "",
                false,
                "ok",
            );
        }
        self.emit();
        self.pump_bot_events(id, rx);
    }

    fn resume_failed(&self, id: &str, why: &str) {
        self.lock().start_failed(id, "이어서 실행하지 못했어요", why);
        self.emit();
        self.notice_from("run", "warn", why);
    }
}

fn checkpoint_matches(cp: &Checkpoint, identity: &crate::state::SavedRecovery, base: &str) -> bool {
    !cp.completed
        && cp.session_id == identity.session_id
        && cp.recovery_id == identity.recovery_id
        && cp.server_base.trim_end_matches('/') == base.trim_end_matches('/')
}

impl Core {
    pub(crate) fn launch_attach_valid(&self, id: &str, generation: u64, fingerprint: &str, member_key: &str) -> bool {
        !self.closed
            && !self.claim_busy
            && self.member_key.trim() == member_key
            && self.run(id).is_some_and(|r| {
                !r.running()
                    && r.start_pending
                    && r.cancel_requested.is_none()
                    && r.generation == generation
                    && self.run_fingerprint(r) == fingerprint
                    && self.conflicting_run(r).is_none()
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cancelling_or_changing_account_prevents_a_prepared_recovery_from_spawning() {
        let mut c = Core::new("https://example.test".into(), true, PathBuf::new());
        c.member_key = "member".into();
        let mut r = Run::new(
            "r".into(),
            json!({"symbol":"BTCUSDT","exchange":"binance"}),
            "r".into(),
            "file",
            "https://example.test".into(),
        );
        r.start_pending = true;
        c.runs.push(r);
        let fp = c.run_fingerprint(c.run("r").unwrap());
        assert!(c.launch_attach_valid("r", 0, &fp, "member"));
        c.run_mut("r").unwrap().cancel_requested = Some(StopMode::StopOnly);
        assert!(!c.launch_attach_valid("r", 0, &fp, "member"));
        c.run_mut("r").unwrap().cancel_requested = None;
        c.member_key = "changed".into();
        assert!(!c.launch_attach_valid("r", 0, &fp, "member"));
        c.member_key = "member".into();
        c.closed = true;
        assert!(!c.launch_attach_valid("r", 0, &fp, "member"));
    }
}
