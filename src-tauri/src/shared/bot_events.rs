//! 봇 이벤트 받기 — 로그·활동·상태·연결·끝을 실행 슬롯에 반영함.

use super::*;

impl Shared {
    pub(crate) fn pump_bot_events(self: &S, id: String, mut rx: tokio::sync::mpsc::UnboundedReceiver<BotEvent>) {
        let me = self.clone();
        tauri::async_runtime::spawn(async move {
            let mut finished = false;
            while let Some(ev) = rx.recv().await {
                match ev {
                    BotEvent::Log(l) => {
                        me.lock().push_raw(&id, &l);
                        me.emit_activity_only();
                    }
                    BotEvent::Activity(a) => {
                        me.lock().push_activity(&id, a.kind, &a.title, &a.detail, &a.raw, false, a.tone);
                        me.emit_activity_only();
                        // 손절이 멈춤·결과 모름·정리 실패 같은 위험은 창을 숨겨 둔 사람에게도 알림.
                        if a.tone == "danger" {
                            me.alert_os(&id, &a.title, &a.detail);
                        }
                    }
                    BotEvent::Status(snap) => {
                        if let Some(d) = me.lock().run_mut(&id).and_then(|r| r.data.as_mut()) {
                            d.snapshot = snap;
                            d.updated_at = now_ms();
                        }
                        me.emit_soon();
                    }
                    BotEvent::Health(h) => {
                        // 매 틱 오지만 대개 같음 — 바뀌었을 때만 화면을 다시 그림.
                        let changed = me.lock().run_mut(&id).and_then(|r| r.data.as_mut()).is_some_and(|d| {
                            let changed = d.health != h;
                            d.online = h.online;
                            d.health = h;
                            changed
                        });
                        if changed {
                            me.emit_soon();
                        }
                    }
                    BotEvent::Ending { .. } => {
                        // 서버에 종료를 보고하는 동안 '실행 중'으로 보이지 않게 — 결과는 Finished에서 남김.
                        if let Some(r) = me.lock().run_mut(&id) {
                            r.stopping = true;
                        }
                        me.emit();
                    }
                    BotEvent::Finished { status, note } => {
                        {
                            let mut c = me.lock();
                            let tone = if status == "error" { "danger" } else { "muted" };
                            let title =
                                if status == "error" { "실행이 멈췄어요" } else { "실행을 끝냈어요" };
                            let detail = if note.is_empty() {
                                crate::state::finish_label(&status).to_string()
                            } else {
                                note.clone()
                            };
                            c.push_activity(
                                &id,
                                "stop",
                                title,
                                &detail,
                                &format!(
                                    "종료됨 ({status}){}",
                                    if note.is_empty() { String::new() } else { format!(" · {note}") }
                                ),
                                false,
                                tone,
                            );
                            if let Some(r) = c.run_mut(&id) {
                                if let Some(d) = r.data.as_mut() {
                                    d.finished = Some((status.clone(), note.clone(), now_ms()));
                                }
                                r.bot = None;
                                r.stopping = false;
                            }
                            me.save_runs(&c);
                        }
                        if status == "error" {
                            let why = if note.is_empty() {
                                "활동 기록에서 이유를 확인하세요.".to_string()
                            } else {
                                note
                            };
                            me.alert_os(&id, "실행이 멈췄어요", &why);
                        }
                        me.emit();
                        finished = true;
                        break;
                    }
                }
            }
            if !finished {
                // 봇 작업이 끝났다는 소식 없이 사라짐 — 실행 중으로 남지 않게 정리함.
                {
                    let mut c = me.lock();
                    if let Some(r) = c.run_mut(&id) {
                        r.bot = None;
                        r.stopping = false;
                        if let Some(d) = r.data.as_mut() {
                            d.finished = Some(("error".into(), "실행이 예기치 않게 끝났어요".into(), now_ms()));
                        }
                    }
                    let mock = c.run(&id).is_some_and(|r| r.mode == RunMode::Mock);
                    let next = if mock {
                        "모의 실행이라 실제 주문 · 포지션은 없어요."
                    } else {
                        "거래소에서 열린 주문과 포지션을 확인하세요."
                    };
                    c.push_activity(&id, "error", "실행이 예기치 않게 끝났어요", next, "", false, "danger");
                    me.save_runs(&c);
                    drop(c);
                    me.alert_os(&id, "실행이 예기치 않게 끝났어요", next);
                }
                me.emit();
            }
        });
    }
}
