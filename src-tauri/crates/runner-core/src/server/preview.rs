//! 시작 전 진입 대기 안내 — 조건을 기다리는 매크로인지, 조건 문장과 최근 조건이 맞은 횟수·간격을 서버에 물음.

use std::time::Duration;

use serde_json::{Value, json};

use crate::types::{exchange_name_of, macro_leverage, macro_side, market_of};

pub(crate) const PROTOCOL_ENTRY_PREVIEW_PATH: &str = "/api/runner/entry-preview";

/// `POST /api/runner/entry-preview` — 실패하면 None(시작을 막지 않고 안내만 건너뜀).
pub async fn entry_preview(http: &reqwest::Client, base: &str, runner_key: &str, macro_: &Value) -> Option<Value> {
    let side = macro_side(macro_);
    let market = market_of(&exchange_name_of(macro_), side, macro_leverage(macro_).unwrap_or(1));
    let r = http
        .post(format!("{}{}", base.trim_end_matches('/'), PROTOCOL_ENTRY_PREVIEW_PATH))
        .header("X-Runner-Key", runner_key)
        .json(&json!({"macro": macro_, "market": market.as_str()}))
        .timeout(Duration::from_secs(10))
        .send()
        .await
        .ok()?;
    if !r.status().is_success() {
        return None;
    }
    let v: Value = r.json().await.ok()?;
    v.is_object().then_some(v)
}

/// 서버에 묻기 전에 걸러냄 — 조건을 기다리는 규칙(B·E 눌림·F·G·I·J)만.
pub fn is_waiting_macro(macro_: &Value) -> bool {
    let rule = macro_.get("rule_type").and_then(Value::as_str).unwrap_or_default();
    match rule {
        "B" | "F" | "G" | "I" | "J" => true,
        "E" => macro_.get("params").and_then(|p| p.get("entry_mode")).and_then(Value::as_str) == Some("dip"),
        _ => false,
    }
}
