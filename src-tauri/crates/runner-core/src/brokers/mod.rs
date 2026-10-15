//! 거래소 어댑터 공통 모양. Order·OrderRules와 세 어댑터가 함께 쓰는 메서드.

use async_trait::async_trait;
use std::fmt;
use std::sync::Arc;

use crate::types::{Market, Side};

pub mod binance;
pub mod domestic;
pub mod mock;

pub use binance::{BinanceBroker, BinanceUrls};
pub use domestic::{explain_domestic_error, jwt_token, DomesticBroker};
pub use mock::MockBroker;

pub type LogFn = Arc<dyn Fn(String) + Send + Sync>;

mod pacing;
pub(crate) use pacing::{cool, cool_key, cooling, retry_after_secs};
pub use pacing::{lookup_delay, LOOKUP_ATTEMPTS};

/// 아무것도 하지 않는 로그 — 연결 검사처럼 기록이 필요 없는 곳.
pub fn null_log() -> LogFn {
    Arc::new(|_msg: String| {})
}

pub const TERMINAL_STATUSES: [&str; 3] = ["FILLED", "CANCELED", "REJECTED"];

/// 업비트·빗썸 매수 수수료를 못 읽었을 때의 보수적인 값.
pub const DOMESTIC_BID_FEE_FALLBACK: f64 = 0.0025;
/// 국내 수량 소수 한계(8자리).
pub const VOLUME_DECIMALS: f64 = 0.000_000_01;

pub fn is_terminal(status: &str) -> bool {
    TERMINAL_STATUSES.contains(&status)
}

/// 수수료까지 합쳐 budget 안에 들도록 줄인 주문 금액 — 20,000원·0.04%면 19,992원.
pub fn fee_inclusive(budget: f64, rate: f64) -> f64 {
    if budget > 0.0 {
        budget / (1.0 + rate)
    } else {
        0.0
    }
}

/// 숫자로 읽히지 않거나 0 이하·NaN·무한이면 0.
pub fn positive_number(value: &serde_json::Value) -> f64 {
    positive_f64(match value {
        serde_json::Value::Number(n) => n.as_f64().unwrap_or(0.0),
        serde_json::Value::String(s) => s.trim().parse::<f64>().unwrap_or(0.0),
        _ => 0.0,
    })
}

/// 0보다 큰 유한한 값만 그대로, 나머지(0·음수·-0.0·NaN·무한)는 0.
pub fn positive_f64(x: f64) -> f64 {
    if x.is_finite() && x > 0.0 {
        x
    } else {
        0.0
    }
}

/// 응답 객체의 한 칸을 [`positive_number`]로 읽음. 칸이 없으면 0.
pub(crate) fn get_pos(obj: &serde_json::Map<String, serde_json::Value>, key: &str) -> f64 {
    obj.get(key).map(positive_number).unwrap_or(0.0)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OrderSide {
    Buy,
    Sell,
}

impl OrderSide {
    pub fn as_str(self) -> &'static str {
        match self {
            OrderSide::Buy => "BUY",
            OrderSide::Sell => "SELL",
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct OrderRules {
    pub step: f64,
    pub min_notional: f64,
}

/// 주문 하나에서 낸 수수료. 화면 표시용(서버로 보내는 값에는 넣지 않음).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct FeePaid {
    /// 견적 통화(USDT·KRW)로 옮긴 수수료. 기초자산으로 낸 수수료는 체결가로 옮김.
    pub quote: f64,
    /// 견적 통화로 옮기지 못한 수수료(BNB 등) — (자산, 수량).
    pub other: Vec<(String, f64)>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Order {
    /// FILLED | CANCELED | REJECTED | OPEN | UNKNOWN
    pub status: String,
    pub executed_qty: f64,
    pub avg_price: f64,
    pub acquired_qty: f64,
    pub fees_known: bool,
    pub order_id: Option<String>,
    pub raw_status: String,
    /// 거래소가 알려 준 수수료. None이면 모름(선물·체결 목록이 없는 응답 등).
    pub fee: Option<FeePaid>,
}

impl Order {
    /// 거래소가 주문을 받지 않았다고 분명히 답한 상태. 체결 0·수수료 0, raw_status에 거절 이유.
    pub fn rejected(reason: String) -> Order {
        Order {
            status: "REJECTED".into(),
            executed_qty: 0.0,
            avg_price: 0.0,
            acquired_qty: 0.0,
            fees_known: true,
            order_id: None,
            raw_status: reason,
            fee: Some(FeePaid::default()),
        }
    }

    /// 주문을 보냈는데 그 뒤로 아무것도 확인하지 못한 상태.
    #[cfg(test)]
    pub fn unknown() -> Order {
        Order {
            status: "UNKNOWN".into(),
            executed_qty: 0.0,
            avg_price: 0.0,
            acquired_qty: 0.0,
            fees_known: false,
            order_id: None,
            raw_status: "unknown".into(),
            fee: None,
        }
    }
}

#[derive(Clone, Debug)]
pub struct SubmitRequest {
    pub side: OrderSide,
    pub base_qty: Option<f64>,
    pub notional: Option<f64>,
    pub reduce_only: bool,
    /// 보유를 줄이는 주문인지 여부. None이면 어댑터가 방향으로 판단함.
    pub closing: Option<bool>,
    pub client_id: String,
}

#[derive(Clone, Debug)]
pub enum BrokerError {
    /// 키·허용 IP·권한에 막힘. explanation이 사람이 읽을 문장.
    Access {
        explanation: String,
        name: String,
    },
    /// 거래소가 HTTP 오류로 답함. 바이낸스는 name에 오류 코드("-2015")를 넣음.
    Api {
        status: u16,
        name: String,
        message: String,
    },
    Network(String),
    Other(String),
}

impl BrokerError {
    pub fn name(&self) -> &str {
        match self {
            BrokerError::Access { name, .. } | BrokerError::Api { name, .. } => name,
            _ => "",
        }
    }
}

impl fmt::Display for BrokerError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            BrokerError::Access { explanation, .. } => write!(f, "{explanation}"),
            BrokerError::Api { status, name, message } => {
                if name.is_empty() {
                    write!(f, "HTTP {status}")
                } else {
                    write!(f, "HTTP {status} {name}: {message}")
                }
            }
            BrokerError::Network(m) | BrokerError::Other(m) => write!(f, "{m}"),
        }
    }
}

impl std::error::Error for BrokerError {}

#[async_trait]
pub trait Broker: Send + Sync {
    fn market(&self) -> Market;
    fn side(&self) -> Side;
    fn symbol(&self) -> String;
    /// 현물 매수 수수료를 받은 기초자산에서 떼는지 여부(바이낸스 현물만 참).
    fn fee_from_base_asset(&self) -> bool;
    /// 국내 어댑터가 읽은 매수 수수료율. 바이낸스는 None. 모의는 안쪽 값을 그대로.
    fn bid_fee(&self) -> Option<f64>;
    /// 마지막 리허설에서 거래소가 돌려준 오류 낱말.
    fn last_error_name(&self) -> String;
    fn set_log(&self, log: LogFn);

    async fn check_account(&self) -> Result<(), BrokerError>;
    async fn ensure_ready(&self) -> Result<bool, BrokerError>;
    async fn load_market(&self) -> Result<bool, BrokerError>;
    async fn order_rules(&self) -> Result<OrderRules, BrokerError>;
    async fn rehearse(&self, notional: f64, spend: Option<f64>) -> (bool, String);
    async fn price(&self) -> Result<f64, BrokerError>;
    async fn submit(&self, req: SubmitRequest) -> Result<Order, BrokerError>;

    /// 마지막 확인에서 읽은 잔고 한 줄("사용 가능 잔고 1,234.5 USDT · 보유 BTC 0.01"). 읽지 않았으면 "".
    fn balance_note(&self) -> String {
        String::new()
    }
    /// 마지막 시세 조회에 함께 온 전일 대비 등락률(0.012 = +1.2%). 모르면 None.
    fn day_change(&self) -> Option<f64> {
        None
    }
}

// --- 어댑터들이 함께 쓰는 서버 형식 도우미 ----------------------------------
// 서버가 파이썬이라 로그·오류 문장·숫자 표기를 서버와 같은 형식(파이썬 출력 모양)으로 맞춤.

/// 서버·활동 기록에 남기는 오류 문장(파이썬 `str(exc)` 모양). 바이낸스 오류(이름이 숫자 코드)는 python-binance의
/// `APIError(code=-2015): ...` 모양으로, 나머지는 Display 그대로.
pub fn exc_text(e: &BrokerError) -> String {
    match e {
        BrokerError::Api { name, message, .. } if !name.is_empty() && name.parse::<i64>().is_ok() => {
            format!("APIError(code={name}): {message}")
        }
        other => other.to_string(),
    }
}

/// `math.isclose(a, b, rel_tol=1e-9, abs_tol=1e-12)`.
pub fn is_close(a: f64, b: f64) -> bool {
    if a == b {
        return true;
    }
    if !a.is_finite() || !b.is_finite() {
        return false;
    }
    let diff = (a - b).abs();
    diff <= (1e-9 * b.abs()).max(1e-9 * a.abs()) || diff <= 1e-12
}

/// 파이썬 진릿값 — None·False·0·""·[]·{}는 거짓.
pub fn py_truthy(value: &serde_json::Value) -> bool {
    use serde_json::Value;
    match value {
        Value::Null => false,
        Value::Bool(b) => *b,
        Value::Number(n) => n.as_f64().map(|f| f != 0.0).unwrap_or(true),
        Value::String(s) => !s.is_empty(),
        Value::Array(a) => !a.is_empty(),
        Value::Object(o) => !o.is_empty(),
    }
}

/// 파이썬 `repr(float)` — 정수 값은 `100.0`, 아주 작거나 큰 값은 `1e-05`·`1e+16`.
pub fn py_float_repr(f: f64) -> String {
    if f.is_nan() {
        return "nan".into();
    }
    if f.is_infinite() {
        return if f > 0.0 { "inf".into() } else { "-inf".into() };
    }
    let abs = f.abs();
    if abs != 0.0 && !(1e-4..1e16).contains(&abs) {
        let text = format!("{f:e}");
        let (mantissa, exp) = text.split_once('e').unwrap_or((&text, "0"));
        let exp: i32 = exp.parse().unwrap_or(0);
        let sign = if exp < 0 { '-' } else { '+' };
        return format!("{mantissa}e{sign}{:02}", exp.abs());
    }
    if f.fract() == 0.0 {
        format!("{f:.1}")
    } else {
        format!("{f}")
    }
}

/// 파이썬 `str(value)` — JSON 문자열은 따옴표 없이, 정수는 정수로, 실수는 repr로.
pub fn py_str(value: &serde_json::Value) -> String {
    use serde_json::Value;
    match value {
        Value::Null => "None".into(),
        Value::Bool(true) => "True".into(),
        Value::Bool(false) => "False".into(),
        Value::String(s) => s.clone(),
        Value::Number(n) => {
            if n.is_f64() {
                py_float_repr(n.as_f64().unwrap_or(0.0))
            } else {
                n.to_string()
            }
        }
        other => other.to_string(),
    }
}

/// reqwest 오류를 네트워크 오류로. 주소(서명·쿼리 포함)는 빼고 원인 사슬만 남김.
pub fn network_error(e: reqwest::Error) -> BrokerError {
    let e = e.without_url();
    let mut text = e.to_string();
    let mut source = std::error::Error::source(&e);
    while let Some(cause) = source {
        text.push_str(&format!(": {cause}"));
        source = cause.source();
    }
    BrokerError::Network(text)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::Value;

    #[test]
    fn positive_f64_matches_reading_the_json_number() {
        // JSON 숫자로 옮겨 positive_number로 읽던 길과 같은 값 — NaN·무한은 JSON 숫자가 못 돼 0이었음.
        let via_json =
            |x: f64| positive_number(&serde_json::Number::from_f64(x).map(Value::Number).unwrap_or(Value::Null));
        for x in
            [0.0, -0.0, -1.5, 1e-300, 0.5, 2.0, 1e300, f64::NAN, f64::INFINITY, f64::NEG_INFINITY, f64::MIN_POSITIVE]
        {
            assert_eq!(positive_f64(x).to_bits(), via_json(x).to_bits(), "{x}");
        }
        assert_eq!(positive_f64(-0.0).to_bits(), 0.0f64.to_bits(), "-0.0는 +0으로");
    }

    #[test]
    fn rejected_order_has_no_fill_and_keeps_the_reason() {
        let o = Order::rejected("-2010 Account has insufficient balance".into());
        assert_eq!(o.status, "REJECTED");
        assert_eq!((o.executed_qty, o.avg_price, o.acquired_qty), (0.0, 0.0, 0.0));
        assert!(o.fees_known && o.order_id.is_none());
        assert_eq!(o.raw_status, "-2010 Account has insufficient balance");
        assert_eq!(o.fee, Some(FeePaid::default()));
    }
}
