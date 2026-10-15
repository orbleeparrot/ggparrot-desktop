//! 시작 직전 검사 — 이 슬롯의 매크로·모드·키로 연결을 확인함(주문 없음).

use super::*;

/// 시작 직전 검사가 끝난 뒤 할 일.
#[derive(Debug, PartialEq)]
pub(crate) enum RunCheckNext {
    Nothing,
    /// 막힌 이유를 알림으로 보임.
    Warn(String),
    /// 연결 도우미의 결과 화면으로 데려감.
    Wizard(WizardTarget),
    /// 이어서 시작함.
    Start,
}

impl Shared {
    /// 시작 직전 검사(이 슬롯의 매크로·모드·키 그대로). 통과해야 시작됨.
    pub fn begin_run_check(self: &S, id: &str, start_after: bool) {
        let job = {
            let mut c = self.lock();
            let Some(run) = c.run(id) else { return };
            if run.running() || run.checking {
                return;
            }
            let creds = c.run_credentials(run);
            let (m, mode) = (run.macro_.clone(), run.mode);
            let r = c.run_mut(id).unwrap();
            r.invalidate_preflight();
            if mode == RunMode::Mock {
                r.preflight = Some(mock_check(&m, &creds));
                None
            } else {
                r.checking = true;
                let generation = r.generation;
                c.push_activity(
                    id,
                    "info",
                    "키 · 허용 IP · 주문 조건을 확인하고 있어요",
                    "실제 주문은 보내지 않아요",
                    "키 · 허용 IP · 주문 조건을 확인하고 있어요. 실제 주문은 보내지 않아요.",
                    false,
                    "muted",
                );
                Some((generation, m, mode, creds))
            }
        };
        self.emit();
        let Some((generation, m, mode, creds)) = job else { return };
        let me = self.clone();
        let id = id.to_string();
        tauri::async_runtime::spawn(async move {
            let result = perform_connection_check(&m, &creds, mode, &me.check_http).await;
            me.finish_run_check(&id, generation, result, start_after);
        });
    }

    pub(crate) fn finish_run_check(self: &S, id: &str, generation: u64, result: PreflightResult, start_after: bool) {
        let accepted = self.lock().accept_run_check(id, generation, &result, start_after);
        let Some((next, connected)) = accepted else { return };
        self.after_connected(connected);
        self.emit();
        self.refresh_wizard();
        match next {
            RunCheckNext::Nothing => {}
            RunCheckNext::Warn(m) => self.notice_from("run", "warn", m),
            RunCheckNext::Wizard(target) => self.wizard_open(target, "result_fail"),
            RunCheckNext::Start => match self.start_run(id) {
                StartReply::ConfirmLive { title, message } => {
                    let _ = self.app.emit(EV_CONFIRM, json!({ "runId": id, "title": title, "message": message }));
                }
                StartReply::Blocked { message } if !message.is_empty() => self.notice_from("run", "warn", message),
                _ => {}
            },
        }
    }
}

impl Core {
    /// 시작 직전 검사 결과를 실행에 남기고 다음 할 일을 정함.
    /// - 닫는 중이거나 실행이 없으면 None(화면도 다시 그리지 않음).
    /// - 칸의 확인 결과로도 남겼고 연결됨이면 키체인에 남길 일을 함께 돌려줌(잠금을 푼 뒤 처리).
    pub(crate) fn accept_run_check(
        &mut self,
        id: &str,
        generation: u64,
        result: &PreflightResult,
        start_after: bool,
    ) -> Option<(RunCheckNext, Option<ConnectedSlot>)> {
        if self.closed {
            return None;
        }
        let run = self.run(id)?;
        let fp = self.run_fingerprint(run);
        let stale = run.generation != generation || result.fingerprint != fp || run.running();
        let (ex, mode, market, target) = (run.exchange(), run.mode, run.market(), run.wizard_target());
        let wizard_open = self.wizard.is_some();
        self.run_mut(id)?.checking = false;
        if stale {
            self.push_activity(
                id,
                "info",
                "확인하는 동안 설정이 바뀌었어요",
                "매매는 시작하지 않았어요. 다시 시작해 주세요.",
                "확인 중 설정이 바뀌었어요. 매매는 시작하지 않았어요. 다시 시작해 주세요.",
                false,
                "warn",
            );
            return Some((RunCheckNext::Nothing, None));
        }
        self.run_mut(id)?.preflight = Some(result.clone());
        // 시작 직전 검사도 같은 칸의 키를 본 것임 — 칸의 확인 모드·시장(`Core::probe_market`)과 같으면
        // 설정의 연결 상태에도 씀. 시장이 다르면 칸이 보는 권한과 달라 남기지 않음.
        let mut connected = None;
        if let Some(ex) = ex {
            let slot = KeySlot::for_run(ex, mode, market);
            let usable = result.ok || !result.error_name.is_empty();
            if slot.check_mode() == mode && self.probe_market(slot) == market && usable {
                let key_fp = self.slot_fingerprint(slot);
                let mut stripped = result.clone();
                stripped.broker = None;
                connected = self.store_slot_check(slot, key_fp, &stripped);
            }
        }
        if result.ok {
            let next = if start_after { RunCheckNext::Start } else { RunCheckNext::Nothing };
            return Some((next, connected));
        }
        self.push_activity(id, "error", "시작 전 확인에서 막혔어요", &result.reason, &result.reason, false, "danger");
        let view = check_view(ex.map(|e| e.as_str()).unwrap_or(""), false, &result.error_name);
        let warn = || RunCheckNext::Warn(format!("{}\n\n매매는 시작하지 않았어요.", result.reason));
        let next = if start_after && view.problem == "order_size" {
            warn()
        } else if let (true, false, Some(target)) = (start_after, wizard_open, target) {
            // 오류 창 대신 연결 도우미의 결과 화면으로 — 무엇이 막혔고 어떻게 고치는지가 거기 있음.
            RunCheckNext::Wizard(target)
        } else if start_after {
            // [시작]을 눌러 기다리는 사람에게 막힌 이유를 바로 보임(활동 기록 한 줄로 끝내지 않게).
            warn()
        } else {
            RunCheckNext::Nothing
        };
        Some((next, connected))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn core_with_run(mode: RunMode) -> Core {
        let mut c = Core::new("https://example.test".into(), true, PathBuf::from("/tmp/x"));
        c.member_key = "m".into();
        c.slot_mut(KeySlot::Upbit).keys = KeyPair { api_key: "k".into(), api_secret: "s".into() };
        let mut r = Run::new(
            "a".into(),
            json!({"symbol": "KRW-BTC", "exchange": "upbit"}),
            "a".into(),
            "file",
            "https://example.test".into(),
        );
        r.mode = mode;
        r.checking = true;
        c.runs.push(r);
        c
    }

    fn result_for(c: &Core, ok: bool, error_name: &str) -> PreflightResult {
        let fp = c.run_fingerprint(c.run("a").unwrap());
        let mut r = PreflightResult::new(ok, if ok { String::new() } else { "막힘".into() }, fp);
        r.error_name = error_name.into();
        r
    }

    #[test]
    fn a_passing_check_starts_and_records_the_slot() {
        let mut c = core_with_run(RunMode::Live);
        let gen = c.run("a").unwrap().generation;
        let result = result_for(&c, true, "");
        let (next, connected) = c.accept_run_check("a", gen, &result, true).unwrap();
        assert_eq!(next, RunCheckNext::Start);
        assert!(connected.is_some_and(|k| k.slot == KeySlot::Upbit), "연결됨이면 키체인 일을 넘김");
        let r = c.run("a").unwrap();
        assert!(!r.checking && r.preflight.is_some());
        assert!(matches!(c.slot_status(KeySlot::Upbit), SlotStatus::Done(_)), "설정의 칸에도 남김");
    }

    #[test]
    fn a_stale_check_is_dropped_and_a_failure_goes_to_the_wizard() {
        let mut c = core_with_run(RunMode::Live);
        let gen = c.run("a").unwrap().generation;
        let result = result_for(&c, true, "");
        let (next, connected) = c.accept_run_check("a", gen + 1, &result, true).unwrap();
        assert_eq!((next, connected.is_none()), (RunCheckNext::Nothing, true), "판이 바뀐 결과는 버림");
        assert!(c.run("a").unwrap().preflight.is_none());

        let failed = result_for(&c, false, "invalid_key");
        let (next, _) = c.accept_run_check("a", gen, &failed, true).unwrap();
        assert!(matches!(next, RunCheckNext::Wizard(t) if t.slot == KeySlot::Upbit), "{next:?}");
        let (next, _) = c.accept_run_check("a", gen, &failed, false).unwrap();
        assert_eq!(next, RunCheckNext::Nothing, "[시작]을 누르지 않았으면 활동 기록만");

        c.closed = true;
        assert!(c.accept_run_check("a", gen, &failed, true).is_none(), "닫는 중에는 아무것도 하지 않음");
    }
}
