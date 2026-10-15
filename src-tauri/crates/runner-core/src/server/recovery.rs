//! 밀린 실행 기록 다시 보내기 — 모든 실행이 같은 저장함의 상태(EventOutbox::recovery)를 함께 씀.

use super::*;

impl ServerClient {
    /// 이전 세션(또는 지금 세션)의 남은 기록을 다시 보냄. 스냅샷·명령 ack는 다시 보내지 않음.
    pub async fn recover_events(&self, max_batches: usize, include_current: bool) {
        let sessions = match self.inner.outbox.sessions(&self.inner.namespace) {
            Ok(s) => s,
            Err(_) => {
                let mut st = self.inner.state.lock().unwrap();
                self.storage_warning_locked(&mut st);
                return;
            }
        };
        let current = self.session_id();
        let mut ordered = sessions.clone();
        {
            let mut rec = self.recovery();
            rec.retry_after.retain(|sid, _| sessions.contains(sid));
            if !ordered.is_empty() {
                let start = rec.cursor % ordered.len();
                ordered.rotate_left(start);
                rec.cursor += 1;
            }
        }
        let mut attempted = 0;
        for sid in ordered {
            if !include_current && Some(sid) == current {
                continue;
            }
            {
                let rec = self.recovery();
                // 다른 실행이 지금 돌리는 세션은 그 실행이 하트비트로 보냄.
                if Some(sid) != current && rec.live.contains(&sid) {
                    continue;
                }
                if rec.retry_after.get(&sid).is_some_and(|d| *d > Instant::now()) {
                    continue;
                }
            }
            while attempted < max_batches {
                let events = self.peek_events(Some(sid));
                if events.is_empty() {
                    break;
                }
                attempted += 1;
                let url = format!("{}/api/runner/sessions/{sid}/events", self.inner.base);
                let resp = self
                    .inner
                    .http
                    .post(url)
                    .headers(self.headers())
                    .json(&json!({"events": events}))
                    .timeout(Duration::from_secs(3))
                    .send()
                    .await;
                let delay = |secs| {
                    self.recovery().retry_after.insert(sid, Instant::now() + Duration::from_secs(secs));
                };
                match resp {
                    Ok(r) if r.status().is_success() => {
                        let data: Value = r.json().await.unwrap_or(Value::Null);
                        self.ack_events(&events, &data, Some(sid));
                        let confirmed =
                            data.get("accepted_event_keys").and_then(Value::as_array).is_some_and(|a| !a.is_empty());
                        if !confirmed {
                            delay(30);
                            break;
                        }
                        if self.peek_events(Some(sid)) == events {
                            delay(30);
                            break;
                        }
                        self.recovery().retry_after.remove(&sid);
                    }
                    Ok(r) => {
                        let code = r.status().as_u16();
                        delay(if [401, 403, 404, 410].contains(&code) { 300 } else { 30 });
                        break;
                    }
                    Err(_) => {
                        delay(30);
                        break;
                    }
                }
            }
            if attempted >= max_batches {
                break;
            }
        }
    }

    pub(super) fn recovery(&self) -> std::sync::MutexGuard<'_, crate::outbox::RecoveryState> {
        self.inner.outbox.recovery.lock().unwrap_or_else(|e| e.into_inner())
    }

    pub(super) fn schedule_event_recovery(&self, include_current: bool) {
        if self.recovery().running {
            return;
        }
        let pending = match self.inner.outbox.sessions(&self.inner.namespace) {
            Ok(p) => p,
            Err(_) => {
                let mut st = self.inner.state.lock().unwrap();
                self.storage_warning_locked(&mut st);
                return;
            }
        };
        let current = self.session_id();
        let now = Instant::now();
        let due = {
            let rec = self.recovery();
            pending.iter().any(|sid| {
                let mine = Some(*sid) == current;
                (include_current || !mine)
                    && (mine || !rec.live.contains(sid))
                    && rec.retry_after.get(sid).is_none_or(|d| *d <= now)
            })
        };
        if !due {
            return;
        }
        let Ok(handle) = tokio::runtime::Handle::try_current() else {
            return;
        };
        {
            let mut rec = self.recovery();
            if rec.running {
                return;
            }
            rec.running = true;
        }
        let me = self.clone();
        handle.spawn(async move {
            me.recover_events(3, include_current).await;
            me.recovery().running = false;
        });
    }

    /// 켤 때 한 번 — 지난 실행에서 못 보낸 기록을 이어서 보냄.
    pub fn resume_pending_events(&self) {
        self.schedule_event_recovery(false);
    }
}
