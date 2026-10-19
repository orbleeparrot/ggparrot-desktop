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
                        let (slot_changed, alert) = {
                            let mut c = me.lock();
                            c.push_activity(&id, a.kind, &a.title, &a.detail, &a.raw, false, a.tone);
                            let alert = c.activity_alert(&id, a.kind, a.tone, &a.title, &a.detail);
                            // 진입 체결("buy") — 잔고가 있었음. 설정 칸에 남은 잔고 부족 글을 지움.
                            (a.kind == "buy" && c.entry_filled(&id), alert)
                        };
                        if let Some(alert) = alert {
                            me.alert_os(&id, alert);
                        }
                        if slot_changed {
                            me.emit();
                            me.refresh_wizard();
                        } else {
                            me.emit_activity_only();
                        }
                    }
                    BotEvent::Status(snap) => {
                        let alert = me.lock().apply_snapshot(&id, snap);
                        if let Some(alert) = alert {
                            me.alert_os(&id, alert);
                        }
                        me.emit_soon();
                    }
                    BotEvent::EntryWait(wait) => {
                        let changed = me.lock().run_mut(&id).and_then(|r| r.data.as_mut()).is_some_and(|d| {
                            let changed = d.entry_wait != wait;
                            d.entry_wait = wait;
                            changed
                        });
                        if changed {
                            me.emit_soon();
                        }
                    }
                    BotEvent::Health(h) => {
                        // 매 틱 오지만 대개 같음 — 바뀌었을 때만 화면을 다시 그림.
                        let changed = me.lock().run_mut(&id).and_then(|r| r.data.as_mut()).is_some_and(|d| {
                            let changed = d.health != h;
                            d.online = h.online;
                            d.health = h;
                            changed
                        });
                        let alert = me.lock().offline_alert(&id);
                        if let Some(alert) = alert {
                            me.alert_os(&id, alert);
                        }
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
            self.alert_os(id, Alert::grouped("실행이 멈췄어요", why));
        }
    }

    /// 봇이 끝났다는 소식 없이 사라짐 — 오류로 끝난 것으로 남기고 OS 알림.
    fn on_bot_vanished(self: &S, id: &str) {
        if let Ok(next) = self.edit_runs(|c| Ok::<_, ()>(c.bot_vanished(id))) {
            self.alert_os(id, Alert::grouped(VANISHED, next));
        }
    }
}

/// 평가손익 알림 구간(%) — ±2%부터(2026-10-20 사용자 결정, 웹 [내 에이전트] 알림은 ±3%부터).
const PNL_BANDS: [f64; 7] = [2.0, 5.0, 10.0, 15.0, 20.0, 30.0, 50.0];

/// 지금 평가손익이 든 가장 깊은 구간(부호 포함). 어느 구간에도 안 들면 0.
fn pnl_band(pct: f64) -> f64 {
    PNL_BANDS.iter().rev().find(|b| pct.abs() >= **b).map_or(0.0, |b| b.copysign(pct))
}

impl Core {
    /// 상태 스냅샷을 반영함. 보유 중 평가손익이 새 구간(±2 · 5 · 10 …%)에 처음 들어가면 알림을 돌려줌 —
    /// 같은 쪽 더 깊은 구간이거나 반대쪽 구간일 때만(경계에서 오르내려도 반복하지 않음). 무포지션이면 처음부터 셈.
    /// 실행기 화면과 같은 수수료 반영 값을 쓰고, 모르면 평단 대비 가격(수수료 전)이라고 밝힘.
    pub(crate) fn apply_snapshot(&mut self, id: &str, snap: Value) -> Option<Alert> {
        let r = self.run_mut(id)?;
        let d = r.data.as_mut()?;
        d.snapshot = snap;
        d.updated_at = now_ms();
        let holding = d.holding();
        let net = d.snapshot.get("unrealized_net_pct").and_then(Value::as_f64);
        let gross = d.snapshot.get("unrealized_pct").and_then(Value::as_f64);
        if !holding {
            r.pnl_band = 0.0;
            return None;
        }
        let pct = net.or(gross)?;
        let (band, last) = (pnl_band(pct), r.pnl_band);
        if band == 0.0 || (last != 0.0 && (band > 0.0) == (last > 0.0) && band.abs() <= last.abs()) {
            return None;
        }
        r.pnl_band = band;
        let title = if band > 0.0 {
            format!("수익이 +{band}%를 넘었어요")
        } else {
            format!("손실이 {band}%를 넘었어요")
        };
        let basis = if net.is_some() { "" } else { " (수수료 전)" };
        Some(Alert::once(&title, &format!("지금 평가손익 {}{basis}", runner_core::money::signed_pct(pct))))
    }

    /// 활동 한 줄을 OS 알림으로 띄울지 — 체결(매수 · 매도)은 늘, 오류 · 위험(빨강) · 서버가 실행을 끝냄도.
    /// 판 체결의 빨강은 손실 색일 뿐이라 체결 알림으로 한 번만 띄움. 서버 연결이 돌아온 소식은 끊겼다고
    /// 알렸을 때만(배포로 잠깐 끊겼다 붙는 건 알리지 않음).
    pub(crate) fn activity_alert(
        &mut self,
        id: &str,
        kind: &str,
        tone: &str,
        title: &str,
        detail: &str,
    ) -> Option<Alert> {
        match kind {
            "buy" | "sell" => Some(Alert::once(title, detail)),
            "error" => Some(Alert::grouped(title, detail)),
            "stop" if tone == "warn" || tone == "danger" => Some(Alert::grouped(title, detail)),
            "connection" if tone == "ok" => {
                let r = self.run_mut(id)?;
                std::mem::take(&mut r.offline_alerted).then(|| Alert::grouped(title, "새 진입도 다시 해요."))
            }
            _ if tone == "danger" => Some(Alert::grouped(title, detail)),
            _ => None,
        }
    }

    /// 서버와 5분 넘게 끊겼으면(실전 · 테스트넷) 한 번 알림. 연결이 돌아온 활동에서 다시 켬.
    pub(crate) fn offline_alert(&mut self, id: &str) -> Option<Alert> {
        let r = self.run_mut(id)?;
        let (what, tone) = r.degraded()?;
        if tone != "danger" || !what.starts_with("서버 끊김") || r.offline_alerted {
            return None;
        }
        r.offline_alerted = true;
        Some(Alert::grouped("서버와 5분 넘게 연결이 끊겼어요", "그동안 새 진입은 멈추고 손절만 지켜요."))
    }

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

    fn holding(pct: f64) -> Value {
        json!({"in_position": true, "unrealized_pct": pct + 0.1, "unrealized_net_pct": pct})
    }

    #[tokio::test]
    async fn pnl_alerts_fire_once_per_band_and_reset_when_flat() {
        let mut c = core_running();
        let mut seen = vec![];
        for pct in [0.4, 1.9, 1.1, 2.2, 1.8, 2.4, 4.9, 5.1, -2.3, -4.0, -2.1] {
            if let Some(a) = c.apply_snapshot("a", holding(pct)) {
                seen.push(a.title);
            }
        }
        assert_eq!(seen, ["수익이 +2%를 넘었어요", "수익이 +5%를 넘었어요", "손실이 -2%를 넘었어요"]);
        // 정리 뒤(무포지션) 다시 들면 처음부터 셈
        assert!(c.apply_snapshot("a", json!({"in_position": false})).is_none());
        let a = c.apply_snapshot("a", holding(-2.5)).unwrap();
        assert_eq!(a, Alert::once("손실이 -2%를 넘었어요", "지금 평가손익 -2.50%"));
        assert_eq!(pnl_band(-12.0), -10.0);
        assert_eq!(pnl_band(1.99), 0.0);
    }

    #[tokio::test]
    async fn pnl_alert_says_when_it_uses_the_price_move_before_fees() {
        let mut c = core_running();
        let a = c.apply_snapshot("a", json!({"in_position": true, "unrealized_pct": 2.2})).unwrap();
        assert_eq!(a.body, "지금 평가손익 +2.20% (수수료 전)");
        // 수수료 반영 값이 있으면 그 값으로 — 가격은 +2% 를 넘었어도 수수료를 빼면 아직이면 조용함
        let mut c = core_running();
        assert!(c
            .apply_snapshot("a", json!({"in_position": true, "unrealized_pct": 2.1, "unrealized_net_pct": 1.9}))
            .is_none());
    }

    #[tokio::test]
    async fn fills_errors_and_dangers_alert_but_routine_notes_do_not() {
        let mut c = core_running();
        let buy = c.activity_alert("a", "buy", "ok", "WAL 791개를 64.4에 샀어요", "수수료 25 KRW").unwrap();
        assert!(!buy.group);
        // 손실로 판 체결은 빨강이지만 체결 알림 하나(묶지 않음)
        let sell =
            c.activity_alert("a", "sell", "danger", "WAL 791개를 52.8에 팔았어요", "실현손익 -9,234 KRW").unwrap();
        assert_eq!(sell, Alert::once("WAL 791개를 52.8에 팔았어요", "실현손익 -9,234 KRW"));
        assert!(c.activity_alert("a", "error", "warn", "잔고가 모자라 주문하지 못했어요", "").unwrap().group);
        assert!(c.activity_alert("a", "connection", "danger", "서버가 회원 키를 받지 않아요", "").is_some());
        assert!(c.activity_alert("a", "stop", "warn", "서버가 이 실행을 끝냈어요", "").is_some());
        assert!(c.activity_alert("a", "stop", "muted", "웹에서 종료를 요청했어요", "").is_none());
        assert!(c.activity_alert("a", "signal", "muted", "조건 확인 · 아직", "").is_none());
        assert!(c.activity_alert("a", "connection", "warn", "서버 연결을 다시 시도하고 있어요", "").is_none());
        // 끊겼다고 알리지 않았으면 돌아온 소식도 조용함(배포로 잠깐 끊긴 것)
        assert!(c.activity_alert("a", "connection", "ok", "서버 연결이 돌아왔어요", "").is_none());
    }

    #[tokio::test]
    async fn a_long_server_outage_alerts_once_and_its_recovery_too() {
        let mut c = core_running();
        let r = c.run_mut("a").unwrap();
        r.mode = RunMode::Live;
        r.stopping = false;
        let d = r.data.as_mut().unwrap();
        d.updated_at = now_ms();
        d.online = false;
        d.health.online = false;
        d.health.offline_since = (now_ms() - 6 * 60_000) as u64;
        assert_eq!(c.offline_alert("a").unwrap().title, "서버와 5분 넘게 연결이 끊겼어요");
        assert!(c.offline_alert("a").is_none(), "한 번만");
        let back = c.activity_alert("a", "connection", "ok", "서버 연결이 돌아왔어요", "").unwrap();
        assert_eq!(back.title, "서버 연결이 돌아왔어요");
        assert!(!c.run("a").unwrap().offline_alerted);
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
