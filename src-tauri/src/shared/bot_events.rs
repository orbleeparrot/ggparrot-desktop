//! 봇 이벤트 받기 — 로그·활동·상태·연결·끝을 실행 슬롯에 반영함.

use super::*;

/// 봇이 끝났다는 소식 없이 사라졌을 때의 제목.
const VANISHED: &str = "실행이 예기치 않게 끝났어요";

impl Shared {
    pub(crate) fn pump_bot_events(self: &S, id: String, mut rx: tokio::sync::mpsc::UnboundedReceiver<BotEvent>) {
        let me = self.clone();
        tauri::async_runtime::spawn(async move {
            while let Some(ev) = rx.recv().await {
                match ev {
                    BotEvent::Log(l) => {
                        me.lock().push_raw(&id, &l);
                        me.emit_activity_only();
                    }
                    BotEvent::Activity(a) => {
                        let slot_changed = {
                            let mut c = me.lock();
                            c.push_activity(&id, a.kind, &a.title, &a.detail, &a.raw, false, a.tone);
                            // 진입 체결("buy") — 잔고가 있었음. 설정 칸에 남은 잔고 부족 글을 지움.
                            a.kind == "buy" && c.entry_filled(&id)
                        };
                        if slot_changed {
                            me.emit();
                            me.refresh_wizard();
                        } else {
                            me.emit_activity_only();
                        }
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
                        me.on_bot_finished(&id, &status, &note);
                        return;
                    }
                }
            }
            // 봇 작업이 끝났다는 소식 없이 사라짐 — 실행 중으로 남지 않게 정리함.
            me.on_bot_vanished(&id);
        });
    }

    /// 봇이 끝남 — 결과를 남기고 저장함. 오류로 끝났으면 OS 알림도.
    fn on_bot_finished(self: &S, id: &str, status: &str, note: &str) {
        let _ = self.edit_runs(|c| {
            c.bot_finished(id, status, note);
            Ok::<_, ()>(())
        });
        if status == "error" {
            let why = if note.is_empty() { "활동 기록에서 이유를 확인하세요." } else { note };
            self.alert_os(id, "실행이 멈췄어요", why);
        }
    }

    /// 봇이 끝났다는 소식 없이 사라짐 — 오류로 끝난 것으로 남기고 OS 알림.
    fn on_bot_vanished(self: &S, id: &str) {
        if let Ok(next) = self.edit_runs(|c| Ok::<_, ()>(c.bot_vanished(id))) {
            self.alert_os(id, VANISHED, next);
        }
    }
}

impl Core {
    /// 봇이 보낸 끝(stopped·error)을 활동에 남기고 실행을 끝난 상태로 둠.
    pub(crate) fn bot_finished(&mut self, id: &str, status: &str, note: &str) {
        let tone = if status == "error" { "danger" } else { "muted" };
        let title = if status == "error" { "실행이 멈췄어요" } else { "실행을 끝냈어요" };
        let detail = if note.is_empty() { crate::state::finish_label(status) } else { note };
        let raw =
            if note.is_empty() { format!("종료됨 ({status})") } else { format!("종료됨 ({status}) · {note}") };
        self.push_activity(id, "stop", title, detail, &raw, false, tone);
        if let Some(r) = self.run_mut(id) {
            r.mark_ended(status, note);
        }
    }

    /// 봇이 소식 없이 사라진 실행을 오류로 끝냄. 다음에 할 일(활동·OS 알림의 설명)을 돌려줌.
    pub(crate) fn bot_vanished(&mut self, id: &str) -> &'static str {
        if let Some(r) = self.run_mut(id) {
            r.mark_ended("error", VANISHED);
        }
        let mock = self.run(id).is_some_and(|r| r.mode == RunMode::Mock);
        let next = if mock {
            "모의 실행이라 실제 주문 · 포지션은 없어요."
        } else {
            "거래소에서 열린 주문과 포지션을 확인하세요."
        };
        self.push_activity(id, "error", VANISHED, next, "", false, "danger");
        next
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn core_running() -> Core {
        let mut c = Core::new("https://example.test".into(), true, PathBuf::from("/tmp/x"));
        let mut r = Run::new(
            "a".into(),
            json!({"symbol": "BTCUSDT", "exchange": "binance"}),
            "a".into(),
            "file",
            "https://example.test".into(),
        );
        r.bot = Some(runner_core::bot::BotHandle::idle_for_tests("BTCUSDT"));
        r.stopping = true;
        r.data = Some(RunData::started(1, Some(1)));
        c.runs.push(r);
        c
    }

    #[tokio::test]
    async fn a_finished_bot_leaves_the_result_and_detaches() {
        let mut c = core_running();
        c.bot_finished("a", "error", "청산 실패");
        let r = c.run("a").unwrap();
        assert!(!r.running() && !r.stopping);
        let (st, note, _) = r.data.as_ref().unwrap().finished.clone().unwrap();
        assert_eq!((st.as_str(), note.as_str()), ("error", "청산 실패"));
        let a = r.activity.last().unwrap();
        assert_eq!((a.title.as_str(), a.raw.as_str()), ("실행이 멈췄어요", "종료됨 (error) · 청산 실패"));

        let mut c = core_running();
        c.bot_finished("a", "stopped", "");
        let a = c.run("a").unwrap().activity.last().unwrap();
        assert_eq!((a.detail.as_str(), a.raw.as_str()), ("종료", "종료됨 (stopped)"));
    }

    #[tokio::test]
    async fn a_vanished_bot_ends_in_error() {
        let mut c = core_running();
        assert_eq!(c.bot_vanished("a"), "모의 실행이라 실제 주문 · 포지션은 없어요.");
        let r = c.run("a").unwrap();
        assert!(!r.running());
        assert_eq!(r.data.as_ref().unwrap().finished.as_ref().unwrap().1, VANISHED);
    }
}
