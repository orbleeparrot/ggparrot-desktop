//! 공통 리스크 3종: 하루 최대 손실·최대 보유 시간·손절 뒤 재진입 대기.

use serde_json::Value;

use crate::money::py_num;

fn today() -> String {
    chrono::Local::now().format("%Y-%m-%d").to_string()
}

/// 참·거짓(`py_truthy`)으로 읽은 숫자 설정. None·0·빈 문자열은 없음.
fn setting(v: Option<&Value>) -> Option<(f64, String)> {
    match v {
        Some(Value::Number(n)) => {
            let f = n.as_f64()?;
            if f == 0.0 {
                None
            } else {
                Some((f, n.to_string()))
            }
        }
        Some(Value::String(s)) if !s.is_empty() => s.trim().parse::<f64>().ok().map(|f| (f, s.clone())),
        _ => None,
    }
}

pub struct RiskGuard {
    daily_max_loss: Option<(f64, String)>,
    max_holding_hours: Option<(f64, String)>,
    pub cooldown_minutes: f64,
    pub base: f64,
    day: Option<String>,
    day_pnl: f64,
    halted_day: Option<String>,
    entry_time: Option<f64>,
    cooldown_until: f64,
    clock: Box<dyn Fn() -> f64 + Send + Sync>,
    calendar: Box<dyn Fn() -> String + Send + Sync>,
}

impl RiskGuard {
    pub fn new(risk: &Value, base_capital: f64, cap: f64) -> Self {
        Self::with_clock(risk, base_capital, cap, Box::new(crate::clock::unix_secs), Box::new(today))
    }

    pub fn with_clock(
        risk: &Value,
        base_capital: f64,
        cap: f64,
        clock: Box<dyn Fn() -> f64 + Send + Sync>,
        calendar: Box<dyn Fn() -> String + Send + Sync>,
    ) -> Self {
        let cooldown = match risk.get("cooldown_minutes") {
            Some(Value::Number(n)) => n.as_f64().unwrap_or(0.0),
            Some(Value::String(s)) => s.trim().parse().unwrap_or(0.0),
            _ => 0.0,
        };
        RiskGuard {
            daily_max_loss: setting(risk.get("daily_max_loss_pct")),
            max_holding_hours: setting(risk.get("max_holding_hours")),
            cooldown_minutes: cooldown,
            base: if base_capital > 0.0 { base_capital } else { cap },
            day: None,
            day_pnl: 0.0,
            halted_day: None,
            entry_time: None,
            cooldown_until: 0.0,
            clock,
            calendar,
        }
    }

    pub fn describe(&self) -> String {
        let mut bits = vec![];
        if let Some((_, raw)) = &self.daily_max_loss {
            bits.push(format!("하루 최대 손실 {raw}%"));
        }
        if let Some((_, raw)) = &self.max_holding_hours {
            bits.push(format!("최대 보유 {raw}시간"));
        }
        if self.cooldown_minutes != 0.0 {
            bits.push(format!("손절 뒤 재진입 대기 {}분", py_num(self.cooldown_minutes)));
        }
        if bits.is_empty() {
            "설정 없음".into()
        } else {
            bits.join(" · ")
        }
    }

    pub fn roll_day(&mut self) {
        let today = (self.calendar)();
        if self.day.as_deref() != Some(today.as_str()) {
            self.day = Some(today.clone());
            self.day_pnl = 0.0;
            if self.halted_day.as_ref().is_some_and(|d| *d != today) {
                self.halted_day = None;
            }
        }
    }

    fn daily_loss_pct(&self, unrealized: f64) -> f64 {
        (self.day_pnl + unrealized) / self.base * 100.0
    }

    pub fn entry_blocked(&self) -> (bool, String) {
        if self.halted_day.is_some() && self.halted_day == self.day {
            let limit = self.daily_max_loss.as_ref().map(|(_, r)| format!("({r}%)")).unwrap_or_default();
            return (true, format!("하루 최대 손실{limit}에 닿아 오늘은 새로 진입하지 않아요"));
        }
        let remain = self.cooldown_until - (self.clock)();
        if remain > 0.0 {
            return (true, format!("손절 뒤 재진입 대기 중이에요({:.1}분 남음)", remain / 60.0));
        }
        (false, String::new())
    }

    pub fn force_close(&self, unrealized: f64) -> (bool, String) {
        if let (Some((hours, raw)), Some(entry)) = (&self.max_holding_hours, self.entry_time) {
            let held = ((self.clock)() - entry) / 3600.0;
            if held >= *hours {
                return (true, format!("최대 보유 시간({raw}시간)을 넘었어요({held:.1}시간 보유)"));
            }
        }
        if let Some((limit, raw)) = &self.daily_max_loss {
            let dd = self.daily_loss_pct(unrealized);
            if dd <= -limit {
                return (true, format!("하루 최대 손실({raw}%)에 닿았어요(오늘 {dd:.2}%)"));
            }
        }
        (false, String::new())
    }

    /// 보유 시간을 재고 있는지(진입을 기록했는지).
    pub fn holding(&self) -> bool {
        self.entry_time.is_some()
    }

    pub fn on_entry(&mut self) {
        self.entry_time = Some((self.clock)());
    }

    pub fn on_exit(&mut self, pnl: f64, was_stop: bool) {
        self.day_pnl += pnl;
        self.entry_time = None;
        if was_stop && self.cooldown_minutes > 0.0 {
            self.cooldown_until = (self.clock)() + self.cooldown_minutes * 60.0;
        }
        self.halt_if_needed();
    }

    /// 부분 청산: 실현손익만 일일 집계에 더하고 보유 시간은 그대로.
    pub fn on_partial_exit(&mut self, pnl: f64) {
        self.day_pnl += pnl;
        self.halt_if_needed();
    }

    fn halt_if_needed(&mut self) {
        if let Some((limit, _)) = &self.daily_max_loss {
            if self.daily_loss_pct(0.0) <= -limit {
                self.halted_day = self.day.clone();
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::sync::Arc;

    fn guard(risk: Value, base: f64) -> (RiskGuard, Arc<AtomicU64>) {
        let t = Arc::new(AtomicU64::new(1_000_000));
        let tc = t.clone();
        let g = RiskGuard::with_clock(
            &risk,
            base,
            100.0,
            Box::new(move || tc.load(Ordering::SeqCst) as f64),
            Box::new(|| "2026-10-14".into()),
        );
        (g, t)
    }

    #[test]
    fn base_falls_back_to_cap() {
        assert_eq!(RiskGuard::new(&json!({}), 0.0, 150000.0).base, 150000.0);
        assert_eq!(RiskGuard::new(&json!({}), 5000.0, 150000.0).base, 5000.0);
    }

    #[test]
    fn describe_matches_python() {
        let (g, _) = guard(json!({"daily_max_loss_pct": 5, "max_holding_hours": 2.5, "cooldown_minutes": 30}), 100.0);
        assert_eq!(g.describe(), "하루 최대 손실 5% · 최대 보유 2.5시간 · 손절 뒤 재진입 대기 30.0분");
        let (g, _) = guard(json!({}), 100.0);
        assert_eq!(g.describe(), "설정 없음");
    }

    #[test]
    fn daily_loss_halts_entries() {
        let (mut g, _) = guard(json!({"daily_max_loss_pct": 5}), 100.0);
        g.roll_day();
        g.on_entry();
        g.on_exit(-6.0, false);
        let (blocked, why) = g.entry_blocked();
        assert!(blocked);
        assert_eq!(why, "하루 최대 손실(5%)에 닿아 오늘은 새로 진입하지 않아요");
    }

    #[test]
    fn cooldown_after_stop() {
        let (mut g, t) = guard(json!({"cooldown_minutes": 10}), 100.0);
        g.roll_day();
        g.on_exit(-1.0, true);
        assert!(g.entry_blocked().0);
        assert_eq!(g.entry_blocked().1, "손절 뒤 재진입 대기 중이에요(10.0분 남음)");
        t.fetch_add(601, Ordering::SeqCst);
        assert!(!g.entry_blocked().0);
    }

    #[test]
    fn holding_time_forces_close() {
        let (mut g, t) = guard(json!({"max_holding_hours": 1}), 100.0);
        g.roll_day();
        g.on_entry();
        assert!(!g.force_close(0.0).0);
        t.fetch_add(3600, Ordering::SeqCst);
        let (forced, why) = g.force_close(0.0);
        assert!(forced);
        assert_eq!(why, "최대 보유 시간(1시간)을 넘었어요(1.0시간 보유)");
    }

    #[test]
    fn unrealized_loss_forces_close() {
        let (mut g, _) = guard(json!({"daily_max_loss_pct": 2}), 100.0);
        g.roll_day();
        let (forced, why) = g.force_close(-2.5);
        assert!(forced);
        assert_eq!(why, "하루 최대 손실(2%)에 닿았어요(오늘 -2.50%)");
    }
}
