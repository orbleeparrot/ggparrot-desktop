//! 시작 전 연결 검사 — 서버 세션을 만들지 않고, 선물 설정을 바꾸지 않고, 주문을 내지 않음.

use serde_json::Value;
use std::time::Duration;

use crate::bot::{build_broker, rehearsal_amounts};
use crate::brokers::null_log;
use crate::connection::{preflight_fingerprint, safe_preflight_reason, PreflightResult};
use crate::types::{exchange_name_of, macro_leverage, macro_side, macro_symbol, Credentials, Exchange, RunMode, Side};

/// 연결 검사에 쓰는 HTTP 클라이언트 — 연결 3초·전체 8초.
pub fn check_http_client() -> reqwest::Client {
    crate::net::harden(reqwest::Client::builder())
        .connect_timeout(Duration::from_secs(3))
        .timeout(Duration::from_secs(8))
        .user_agent(crate::net::user_agent())
        .build()
        .unwrap_or_else(|_| crate::net::fallback(Duration::from_secs(8)))
}

fn validate(m: &Value, c: &Credentials, mode: RunMode) -> Result<Option<(Exchange, crate::types::KeyPair)>, String> {
    let name = exchange_name_of(m);
    let exchange = Exchange::parse(&name).ok_or_else(|| crate::types::unsupported_exchange_message(&name))?;
    if macro_symbol(m).is_empty() {
        return Err("매크로에 종목이 없어요. 웹에서 매크로를 다시 받아 주세요.".into());
    }
    let side = macro_side(m);
    let leverage = macro_leverage(m)
        .map_err(|_| "매크로의 레버리지 값이 올바르지 않아요. 웹에서 매크로를 다시 받아 주세요.".to_string())?;
    if exchange.is_domestic() && (side != Side::Long || leverage > 1) {
        return Err(crate::macro_info::DOMESTIC_SPOT_ONLY.into());
    }
    if mode == RunMode::Testnet && !exchange.has_testnet() {
        return Err("국내 거래소에는 테스트넷이 없어요. 모의를 선택하세요.".into());
    }
    if mode == RunMode::Mock {
        return Ok(None);
    }
    let pair = c.pair(exchange).ok_or_else(|| format!("{} API 키와 시크릿을 모두 입력하세요.", exchange.label()))?;
    Ok(Some((exchange, pair)))
}

pub async fn perform_connection_check(
    m: &Value,
    c: &Credentials,
    mode: RunMode,
    http: &reqwest::Client,
) -> PreflightResult {
    let fingerprint = preflight_fingerprint(m, mode, c);
    let (exchange, pair) = match validate(m, c, mode) {
        Err(reason) => return PreflightResult::new(false, safe_preflight_reason(&reason, c), fingerprint),
        Ok(None) => {
            return PreflightResult::new(
                true,
                "모의 준비 완료 · 키 없이 연습하고 실제 주문은 보내지 않아요. 실전 연결은 확인하지 않았어요.",
                fingerprint,
            )
        }
        Ok(Some(v)) => v,
    };
    let broker = match build_broker(exchange, mode, m, &pair, null_log(), http.clone()) {
        Ok(b) => b,
        Err(_) => {
            return PreflightResult::new(
                false,
                "연결 검사에 실패했어요. 인터넷 연결·거래소 키·허용 IP를 확인하고 다시 눌러 주세요.",
                fingerprint,
            )
        }
    };
    let (notional, spend) = rehearsal_amounts(m, exchange.as_str(), Some(&broker));
    let (ok, reason) = broker.rehearse(notional, spend).await;
    let mut result = PreflightResult::new(ok, safe_preflight_reason(&reason, c), fingerprint);
    result.error_name = broker.last_error_name();
    result.error_code = broker.last_error_code();
    result.balance = broker.balance_note();
    if ok {
        result.broker = Some(broker);
    }
    result
}

/// 모의 검사는 인증 요청이 없음 — 동기로 바로 냄.
pub fn mock_check(m: &Value, c: &Credentials) -> PreflightResult {
    let fingerprint = preflight_fingerprint(m, RunMode::Mock, c);
    match validate(m, c, RunMode::Mock) {
        Err(reason) => PreflightResult::new(false, safe_preflight_reason(&reason, c), fingerprint),
        Ok(_) => PreflightResult::new(
            true,
            "모의 준비 완료 · 키 없이 연습하고 실제 주문은 보내지 않아요. 실전 연결은 확인하지 않았어요.",
            fingerprint,
        ),
    }
}
