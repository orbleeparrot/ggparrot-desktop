//! 청산 기준 — 매크로의 익절·손절·트레일링 값을 읽어 상태 카드의 '청산 기준' 막대와 요약 한 줄을 만듦.
//! 웹(gg_parrot frontend/src/lib/positionExits.js `exitRules`)을 옮김 — 같은 매크로면 같은 값·같은 문구.
//! - 손절은 모든 유형 공통(`risk.stop_loss_pct`), K 숏은 `short_stop_loss_pct`가 있으면 그 값.
//! - 익절은 유형별: A `take_profit_pct`, H `take_profit`, K 롱/숏 별도, E는 트레일링(발동·고점 대비),
//!   그 밖(F·J 등)은 선택형 `take_profit`이 있으면 그 값.
//! - 손절·익절 %는 평단 대비 가격 기준(수수료와 무관) — 이 앱의 로컬 손절(`Bot::local_stop_loss`)과 같음.

use serde::Serialize;
use serde_json::Value;

use crate::macro_info::{pct_text, rule_type};
use crate::money::positive;
use crate::types::{macro_side, Side};

#[derive(Serialize, Clone, Debug, PartialEq)]
pub struct ExitRule {
    /// 평단 대비 가격 % (양수).
    pub pct: f64,
    /// "손절 -3%"·"익절 +5%"
    pub label: String,
}

#[derive(Serialize, Clone, Debug, PartialEq)]
pub struct TrailingRule {
    /// 발동 수익 % (0이면 처음부터 추적).
    pub activation: f64,
    /// 고점 대비 되돌림 %.
    pub trail: f64,
    /// "발동 +5% · 고점 -2%"
    pub label: String,
}

#[derive(Serialize, Clone, Debug, PartialEq)]
pub struct ExitRules {
    pub sl: Option<ExitRule>,
    pub tp: Option<ExitRule>,
    pub trailing: Option<TrailingRule>,
    /// "two" | "sl-only" | "tp-only" | "none"
    pub kind: &'static str,
    /// 청산 기준 한 줄 — "익절 +5% · 손절 -3%"·"손절 -4% · 익절은 신호"·"전략 신호".
    pub summary: String,
}

/// 매크로의 청산 규칙. 웹 `exitRules(macro, side)`와 같음(방향은 매크로의 position_side).
pub fn exit_rules(m: &Value) -> ExitRules {
    let empty = Value::Object(Default::default());
    let params = m.get("params").filter(|v| v.is_object()).unwrap_or(&empty);
    let risk = m.get("risk").filter(|v| v.is_object()).unwrap_or(&empty);
    let short = macro_side(m) == Side::Short;
    let mut sl = positive(risk.get("stop_loss_pct"));
    let mut tp = 0.0;
    let mut trailing = None;
    match rule_type(m) {
        Some('A') => tp = positive(params.get("take_profit_pct")),
        Some('H') => tp = positive(params.get("take_profit")),
        Some('K') if short => {
            tp = positive(params.get("short_take_profit_pct"));
            let short_sl = positive(params.get("short_stop_loss_pct"));
            if short_sl > 0.0 {
                sl = short_sl;
            }
        }
        Some('K') => tp = positive(params.get("long_take_profit_pct")),
        Some('E') => {
            let activation = positive(params.get("activation_profit"));
            let trail = positive(params.get("trail_percent"));
            if activation > 0.0 || trail > 0.0 {
                trailing = Some((activation, trail));
            }
        }
        _ => {}
    }
    // F(RSI)·J(이동평균) 등은 선택형 익절 `take_profit`을 둘 수 있음 — 값이 있으면 그것이 익절선.
    if tp == 0.0 && trailing.is_none() {
        tp = positive(params.get("take_profit"));
    }
    let sl = (sl > 0.0).then(|| ExitRule { pct: sl, label: format!("손절 -{}%", pct_text(sl)) });
    let tp = (tp > 0.0).then(|| ExitRule { pct: tp, label: format!("익절 +{}%", pct_text(tp)) });
    let trailing = trailing.map(|(activation, trail)| TrailingRule {
        activation,
        trail,
        label: if activation > 0.0 {
            format!("발동 +{}% · 고점 -{}%", pct_text(activation), pct_text(trail))
        } else {
            format!("고점 -{}%", pct_text(trail))
        },
    });
    let upside = tp.is_some() || trailing.is_some();
    let kind = match (sl.is_some(), upside) {
        (true, true) => "two",
        (true, false) => "sl-only",
        (false, true) => "tp-only",
        (false, false) => "none",
    };
    let mut parts = vec![];
    if let Some(t) = &tp {
        parts.push(t.label.clone());
    }
    if let Some(t) = &trailing {
        parts.push(format!("트레일링 {}", t.label));
    }
    if let Some(s) = &sl {
        parts.push(s.label.clone());
    }
    if !upside && sl.is_some() {
        parts.push("익절은 신호".into());
    }
    let summary = if kind == "none" { "전략 신호".to_string() } else { parts.join(" · ") };
    ExitRules { sl, tp, trailing, kind, summary }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    // 웹 frontend/tests/positionExits.test.js와 같은 사례.
    #[test]
    fn a_has_both_sides_from_the_macro() {
        let r = exit_rules(&json!({"rule_type": "A", "params": {"take_profit_pct": 5}, "risk": {"stop_loss_pct": 3}}));
        assert_eq!(r.kind, "two");
        assert_eq!(
            (r.tp.as_ref().unwrap().label.as_str(), r.sl.as_ref().unwrap().label.as_str()),
            ("익절 +5%", "손절 -3%")
        );
        assert_eq!(r.summary, "익절 +5% · 손절 -3%");
    }

    #[test]
    fn stop_only_take_only_and_none() {
        let g = exit_rules(&json!({"rule_type": "G", "params": {"period": 20}, "risk": {"stop_loss_pct": 4}}));
        assert_eq!(g.kind, "sl-only");
        assert_eq!(g.summary, "손절 -4% · 익절은 신호");
        let h = exit_rules(&json!({"rule_type": "H", "params": {"take_profit": 2.5}, "risk": {}}));
        assert_eq!(h.kind, "tp-only");
        assert_eq!(h.summary, "익절 +2.5%");
        let f = exit_rules(&json!({"rule_type": "F", "params": {"rsi_period": 14, "take_profit": 4}, "risk": {}}));
        assert_eq!(f.kind, "tp-only");
        assert_eq!(f.tp.unwrap().label, "익절 +4%");
        let j = exit_rules(&json!({"rule_type": "J", "params": {"fast_period": 5}, "risk": {}}));
        assert_eq!(j.kind, "none");
        assert_eq!(j.summary, "전략 신호");
        assert_eq!(exit_rules(&Value::Null).kind, "none");
    }

    #[test]
    fn trailing_and_k_short_use_their_own_values() {
        let e = exit_rules(
            &json!({"rule_type": "E", "params": {"activation_profit": 5, "trail_percent": 2}, "risk": {"stop_loss_pct": 3}}),
        );
        assert_eq!(e.kind, "two");
        assert_eq!(e.trailing.as_ref().unwrap().label, "발동 +5% · 고점 -2%");
        assert_eq!(e.summary, "트레일링 발동 +5% · 고점 -2% · 손절 -3%");
        let k = exit_rules(&json!({"rule_type": "K", "position_side": "short",
            "params": {"short_take_profit_pct": 4, "short_stop_loss_pct": 2, "long_take_profit_pct": 9}, "risk": {}}));
        assert_eq!((k.tp.unwrap().pct, k.sl.unwrap().pct), (4.0, 2.0));
        let kl = exit_rules(&json!({"rule_type": "K", "position_side": "long",
            "params": {"long_take_profit_pct": 9}, "risk": {"stop_loss_pct": 3}}));
        assert_eq!((kl.tp.unwrap().pct, kl.sl.unwrap().pct), (9.0, 3.0));
    }

    #[test]
    fn trailing_without_activation_and_string_values() {
        let e = exit_rules(&json!({"rule_type": "E", "params": {"trail_percent": "1.5"}, "risk": {}}));
        assert_eq!(e.kind, "tp-only");
        assert_eq!(e.summary, "트레일링 고점 -1.5%");
        let a =
            exit_rules(&json!({"rule_type": "a", "params": {"take_profit_pct": "2.5"}, "risk": {"stop_loss_pct": 0}}));
        assert_eq!(a.summary, "익절 +2.5%", "소문자 유형·문자열 값·손절 0은 없음");
    }
}
