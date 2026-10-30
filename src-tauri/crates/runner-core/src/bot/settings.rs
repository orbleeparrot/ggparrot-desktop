//! 구동 중 설정 변경 — 서버가 heartbeat 응답으로 내려 준 값을 반영함.
//!
//! 판단은 서버가 하므로 조건 값(지표 기간·문턱)은 여기 오지 않음. 실행기가 쓰는 것은
//! 주문 크기의 밑이 되는 투자 금액·비율과 로컬 안전망(손절·일일 손실·최대 보유시간)뿐임.
//!
//! **들고 있는 포지션의 수량은 건드리지 않음.** 금액을 바꿔도 이미 산 것은 그대로고,
//! 다음 진입부터 새 금액을 씀 — 포지션 크기를 바꾸는 건 추가 매수·부분 청산이라 다른 일임.

use serde_json::{Map, Value, json};

use super::{Bot, num};
use crate::money::grouped;

/// 서버가 바꿀 수 있는 위험 설정 — 이름이 그대로 매크로의 `risk` 칸임.
/// 없는 값(null)은 **끄는 것**이고, 아예 안 온 칸은 그대로 둠.
const RISK_KEYS: [&str; 5] =
    ["invest_ratio", "stop_loss_pct", "daily_max_loss_pct", "max_holding_hours", "cooldown_minutes"];

impl Bot {
    /// 서버가 내려 준 설정을 매크로에 반영함. 바뀐 게 있으면 true(부르는 쪽이 안전망을 다시 만듦).
    ///
    /// 지문(`digest`)이 지금 들고 있는 것과 같으면 아무것도 하지 않음 — 서버는 heartbeat마다
    /// 같은 설정을 보내므로, 지문을 안 보면 5초마다 같은 값을 다시 쓰고 로그를 채움.
    pub(crate) fn apply_settings(&mut self, settings: &Value) -> bool {
        let digest = settings.get("digest").and_then(Value::as_str).unwrap_or("");
        if digest.is_empty() || digest == self.settings_digest {
            // 첫 heartbeat에서 들고 있던 지문을 기억해 둠(시작 때 받은 매크로가 곧 그 설정임).
            if self.settings_digest.is_empty() {
                self.settings_digest = digest.to_string();
            }
            return false;
        }
        self.settings_digest = digest.to_string();
        // 값이 같으면 아무 일도 없었던 것임 — 서버가 조건 값만 바꿔도 지문은 달라지는데,
        // 그건 서버가 판단에 쓰는 값이라 실행기가 쓸 것은 그대로임.
        let mut changed = self.apply_capital(settings);
        changed |= self.apply_risk(settings);
        if changed {
            let count = settings.get("changed_count").and_then(Value::as_i64).unwrap_or(0);
            self.log(format!("설정이 바뀌었어요(웹·실행기에서 {count}번째) — 새 값으로 이어서 돌아요"));
        }
        changed
    }

    /// 투자 금액. 다음 진입부터 쓰임 — 들고 있는 포지션은 그대로.
    fn apply_capital(&mut self, settings: &Value) -> bool {
        let next = num(settings.get("initial_capital"));
        if next <= 0.0 {
            return false;
        }
        let params = self.macro_.get("params").and_then(Value::as_object).cloned().unwrap_or_default();
        let before = num(params.get("initial_capital"));
        if (before - next).abs() < f64::EPSILON {
            return false;
        }
        let mut params = params;
        params.insert("initial_capital".into(), json!(next));
        self.macro_["params"] = Value::Object(params);
        self.capital = next;
        self.log(format!(
            "투자 금액 {} → {} {} · 다음 진입부터 적용돼요(들고 있는 포지션은 그대로)",
            grouped(before, 0),
            grouped(next, 0),
            self.quote
        ));
        true
    }

    /// 안전망. 서버가 보낸 칸만 바꾸고, 값이 없으면 그 규칙을 끔.
    fn apply_risk(&mut self, settings: &Value) -> bool {
        let mut risk = self.macro_.get("risk").and_then(Value::as_object).cloned().unwrap_or_else(Map::new);
        let mut changed = false;
        for key in RISK_KEYS {
            let Some(value) = settings.get(key) else { continue };
            let next = if value.is_null() { Value::Null } else { json!(num(Some(value))) };
            let before = risk.get(key).cloned().unwrap_or(Value::Null);
            if same_number(&before, &next) {
                continue;
            }
            risk.insert(key.into(), next);
            changed = true;
        }
        if changed {
            self.macro_["risk"] = Value::Object(risk);
        }
        changed
    }
}

/// 숫자로 같은가 — `3`과 `3.0`, 없음과 null을 같게 봄.
fn same_number(a: &Value, b: &Value) -> bool {
    match (a.is_null(), b.is_null()) {
        (true, true) => true,
        (false, false) => (num(Some(a)) - num(Some(b))).abs() < f64::EPSILON,
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{Credentials, RunMode};
    use tokio::sync::mpsc;

    fn bot() -> Bot {
        let (tx, _rx) = mpsc::unbounded_channel();
        Bot::new(
            json!({
                "symbol": "BTCUSDT",
                "params": {"initial_capital": 1_000_000.0, "rsi_period": 14},
                "risk": {"invest_ratio": 1.0, "stop_loss_pct": 3.0},
            }),
            Credentials::default(),
            RunMode::Live,
            None,
            tx,
            reqwest::Client::new(),
        )
    }

    /// 시작 직후 처음 받은 지문은 '바뀐 것'이 아님 — 기억만 함.
    fn settled(bot: &mut Bot, digest: &str) {
        bot.apply_settings(&json!({"digest": digest}));
    }

    #[test]
    fn the_first_digest_with_the_same_values_changes_nothing() {
        let mut b = bot();
        assert!(!b.apply_settings(&json!({"digest": "aaa", "initial_capital": 1_000_000.0})));
        assert_eq!(b.settings_digest, "aaa", "지문은 기억해 둬야 다음 변경을 알아봄");
    }

    #[test]
    fn a_change_that_landed_while_starting_is_taken_on_the_first_heartbeat() {
        // 시작하는 동안 웹에서 설정을 바꿨으면 첫 heartbeat의 값이 이미 다름 — 그때도 반영해야 함.
        let mut b = bot();
        assert!(b.apply_settings(&json!({"digest": "aaa", "initial_capital": 2_000_000.0})));
        assert_eq!(b.capital, 2_000_000.0);
    }

    #[test]
    fn the_same_digest_changes_nothing() {
        let mut b = bot();
        settled(&mut b, "aaa");
        assert!(!b.apply_settings(&json!({"digest": "aaa", "initial_capital": 9_999.0})));
        assert_eq!(num(b.macro_["params"].get("initial_capital")), 1_000_000.0);
    }

    #[test]
    fn a_new_capital_is_taken_and_the_position_is_left_alone() {
        let mut b = bot();
        settled(&mut b, "aaa");
        b.in_position = true;
        b.held_qty = 0.5;
        assert!(b.apply_settings(&json!({"digest": "bbb", "initial_capital": 2_000_000.0})));
        assert_eq!(num(b.macro_["params"].get("initial_capital")), 2_000_000.0);
        assert_eq!(b.capital, 2_000_000.0);
        assert_eq!(b.held_qty, 0.5, "들고 있는 수량은 건드리지 않음");
        assert_eq!(num(b.macro_["params"].get("rsi_period")), 14.0, "안 보낸 칸은 그대로");
    }

    #[test]
    fn a_risk_setting_can_be_changed_and_turned_off() {
        let mut b = bot();
        settled(&mut b, "aaa");
        assert!(b.apply_settings(&json!({"digest": "bbb", "stop_loss_pct": 1.5})));
        assert_eq!(num(b.macro_["risk"].get("stop_loss_pct")), 1.5);

        assert!(b.apply_settings(&json!({"digest": "ccc", "stop_loss_pct": Value::Null})));
        assert!(b.macro_["risk"]["stop_loss_pct"].is_null(), "없는 값은 끄는 것");
    }

    #[test]
    fn a_key_the_server_did_not_send_stays_as_it_was() {
        let mut b = bot();
        settled(&mut b, "aaa");
        b.apply_settings(&json!({"digest": "bbb", "stop_loss_pct": 1.5}));
        assert_eq!(num(b.macro_["risk"].get("invest_ratio")), 1.0);
    }

    #[test]
    fn a_new_digest_with_the_same_values_is_not_a_change() {
        // 서버가 조건 값만 바꾸면(지표 기간 등) 실행기가 쓰는 값은 그대로임 — 안전망을 다시 만들 이유가 없음.
        let mut b = bot();
        settled(&mut b, "aaa");
        assert!(!b.apply_settings(&json!({
            "digest": "bbb", "initial_capital": 1_000_000.0, "invest_ratio": 1.0, "stop_loss_pct": 3.0
        })));
        assert_eq!(b.settings_digest, "bbb", "지문은 그래도 따라가야 함");
    }

    #[test]
    fn a_missing_digest_is_ignored() {
        let mut b = bot();
        settled(&mut b, "aaa");
        assert!(!b.apply_settings(&json!({"initial_capital": 2_000_000.0})));
        assert_eq!(num(b.macro_["params"].get("initial_capital")), 1_000_000.0);
    }
}
