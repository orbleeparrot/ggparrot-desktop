//! 시작 직전 검사 — 이 슬롯의 매크로·모드·키로 연결을 확인함(주문 없음).

use super::*;

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
            r.generation += 1;
            r.preflight = None;
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
        enum Next {
            Nothing,
            Warn(String),
            Wizard(WizardTarget),
            Start,
        }
        let next = {
            let mut c = self.lock();
            if c.closed {
                return;
            }
            let Some(run) = c.run(id) else { return };
            let fp = c.run_fingerprint(run);
            let stale = run.generation != generation || result.fingerprint != fp || run.running();
            let (ex, mode, market, target) = (run.exchange(), run.mode, run.market(), run.wizard_target());
            let wizard_open = c.wizard.is_some();
            c.run_mut(id).unwrap().checking = false;
            if stale {
                c.push_activity(
                    id,
                    "info",
                    "확인하는 동안 설정이 바뀌었어요",
                    "매매는 시작하지 않았어요. 다시 시작해 주세요.",
                    "확인 중 설정이 바뀌었어요. 매매는 시작하지 않았어요. 다시 시작해 주세요.",
                    false,
                    "warn",
                );
                Next::Nothing
            } else {
                c.run_mut(id).unwrap().preflight = Some(result.clone());
                // 시작 직전 검사도 같은 칸의 키를 본 것임 — 칸의 확인 모드·시장(`Core::probe_market`)과 같으면
                // 설정의 연결 상태에도 씀. 시장이 다르면 칸이 보는 권한과 달라 남기지 않음.
                if let Some(ex) = ex {
                    let slot = KeySlot::for_run(ex, mode, market);
                    let usable = result.ok || !result.error_name.is_empty();
                    if slot.check_mode() == mode && c.probe_market(slot) == market && usable {
                        let key_fp = c.slot_check_snapshot(slot).3;
                        let mut stripped = result.clone();
                        stripped.broker = None;
                        self.store_slot_check(&mut c, slot, key_fp, &stripped);
                    }
                }
                if result.ok {
                    if start_after {
                        Next::Start
                    } else {
                        Next::Nothing
                    }
                } else {
                    c.push_activity(
                        id,
                        "error",
                        "시작 전 확인에서 막혔어요",
                        &result.reason,
                        &result.reason,
                        false,
                        "danger",
                    );
                    let view = check_view(ex.map(|e| e.as_str()).unwrap_or(""), false, &result.error_name);
                    if start_after && view.problem == "order_size" {
                        Next::Warn(format!("{}\n\n매매는 시작하지 않았어요.", result.reason))
                    } else if let (true, false, Some(target)) = (start_after, wizard_open, target) {
                        // 오류 창 대신 연결 도우미의 결과 화면으로 — 무엇이 막혔고 어떻게 고치는지가 거기 있음.
                        Next::Wizard(target)
                    } else if start_after {
                        // [시작]을 눌러 기다리는 사람에게 막힌 이유를 바로 보임(활동 기록 한 줄로 끝내지 않게).
                        Next::Warn(format!("{}\n\n매매는 시작하지 않았어요.", result.reason))
                    } else {
                        Next::Nothing
                    }
                }
            }
        };
        self.emit();
        self.refresh_wizard();
        match next {
            Next::Nothing => {}
            Next::Warn(m) => self.notice("warn", m),
            Next::Wizard(target) => self.wizard_open(target, "result_fail"),
            Next::Start => match self.start_run(id) {
                StartReply::ConfirmLive { title, message } => {
                    let _ = self.app.emit(EV_CONFIRM, json!({ "runId": id, "title": title, "message": message }));
                }
                StartReply::Blocked { message } if !message.is_empty() => self.notice("warn", message),
                _ => {}
            },
        }
    }
}
