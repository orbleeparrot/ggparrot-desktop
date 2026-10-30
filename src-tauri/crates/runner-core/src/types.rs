//! 실행 모드·거래소·시장·자격증명 모양과 매크로 헬퍼.

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::collections::BTreeMap;

use crate::SERVER_LINK_LEVEL;

pub const DEFAULT_SERVER_BASE: &str = "https://gg-parrot.onrender.com";
pub const LOCAL_SERVER_BASE: &str = "http://127.0.0.1:8000";

/// 서버 주소. 개발 빌드는 환경변수 `GGP_SERVER_BASE`(실행 시) → 빌드 시 값 → 배포 서버.
/// 배포판은 실행 시 환경변수를 읽지 않음 — 같은 사용자 권한의 다른 프로그램이 환경변수로 서버를 바꿔
/// 회원 키를 가로채고 가짜 매매 명령을 내리지 못하게. 빌드 시 값도 https가 아니면 배포 서버를 씀.
pub fn server_base() -> String {
    let runtime = if cfg!(debug_assertions) { std::env::var("GGP_SERVER_BASE").ok() } else { None };
    let base = runtime
        .or_else(|| option_env!("GGP_SERVER_BASE").map(str::to_string))
        .unwrap_or_else(|| DEFAULT_SERVER_BASE.to_string());
    let base = base.trim().trim_end_matches('/').to_string();
    if !cfg!(debug_assertions) && !base.starts_with("https://") {
        return DEFAULT_SERVER_BASE.to_string();
    }
    base
}

/// 개발 빌드에서만 읽는 환경변수(배포판은 늘 None) — 회원 키·매크로 폴더처럼 바꾸면 위험한 값.
pub fn dev_env(name: &str) -> Option<String> {
    if cfg!(debug_assertions) { std::env::var(name).ok() } else { None }
}

/// 환경변수로 상한을 정하되 기본값보다 낮추는 쪽만 받음 — 같은 사용자 권한의 다른 프로그램이 환경변수로
/// 상한을 풀어 큰 실주문을 내게 하지 못하게.
fn env_cap(name: &str, default: f64) -> f64 {
    lowered_cap(std::env::var(name).ok().as_deref(), default)
}

/// 환경변수 글자 → 상한. 기본값보다 낮은 값만 받고, 없거나 숫자가 아니거나 0 이하·무한이면 기본값.
fn lowered_cap(raw: Option<&str>, default: f64) -> f64 {
    match raw.and_then(|v| v.trim().parse::<f64>().ok()) {
        Some(v) if v.is_finite() && v > 0.0 => v.min(default),
        _ => default,
    }
}

/// 1회 주문 상한(USDT). 기본 100(환경변수로는 낮추기만).
pub fn max_order_usdt() -> f64 {
    env_cap("MAX_ORDER_USDT", 100.0)
}
/// 1회 주문 상한(KRW). 기본 150,000(환경변수로는 낮추기만).
pub fn max_order_krw() -> f64 {
    env_cap("MAX_ORDER_KRW", 150_000.0)
}
/// 선물 상한 기준 — "notional" | "margin". margin(레버리지만큼 커짐)은 개발 빌드에서만 고를 수 있음.
pub fn order_cap_basis() -> String {
    if !cfg!(debug_assertions) {
        return "notional".into();
    }
    std::env::var("ORDER_CAP_BASIS").unwrap_or_else(|_| "notional".into()).to_lowercase()
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Exchange {
    Binance,
    Upbit,
    Bithumb,
}

impl Exchange {
    pub const ALL: [Exchange; 3] = [Exchange::Binance, Exchange::Upbit, Exchange::Bithumb];

    pub fn parse(s: &str) -> Option<Exchange> {
        match s.trim().to_lowercase().as_str() {
            "binance" => Some(Exchange::Binance),
            "upbit" => Some(Exchange::Upbit),
            "bithumb" => Some(Exchange::Bithumb),
            _ => None,
        }
    }
    pub fn as_str(self) -> &'static str {
        match self {
            Exchange::Binance => "binance",
            Exchange::Upbit => "upbit",
            Exchange::Bithumb => "bithumb",
        }
    }
    pub fn label(self) -> &'static str {
        match self {
            Exchange::Binance => "바이낸스",
            Exchange::Upbit => "업비트",
            Exchange::Bithumb => "빗썸",
        }
    }
    pub fn is_domestic(self) -> bool {
        matches!(self, Exchange::Upbit | Exchange::Bithumb)
    }
    pub fn has_testnet(self) -> bool {
        matches!(self, Exchange::Binance)
    }
    /// 거래소 발급 창에 적힌 이름 그대로.
    pub fn field_labels(self) -> (&'static str, &'static str) {
        match self {
            Exchange::Upbit => ("Access Key", "Secret Key"),
            _ => ("API Key", "Secret Key"),
        }
    }
}

/// 설정의 키 칸. 바이낸스는 사이트마다 키가 달라 칸이 셋임 — 실거래·현물 테스트넷·선물 데모.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum KeySlot {
    Binance,
    BinanceTestnet,
    BinanceDemo,
    Upbit,
    Bithumb,
}

impl KeySlot {
    pub const ALL: [KeySlot; 5] =
        [KeySlot::Binance, KeySlot::BinanceTestnet, KeySlot::BinanceDemo, KeySlot::Upbit, KeySlot::Bithumb];

    pub fn parse(s: &str) -> Option<KeySlot> {
        match s.trim().to_lowercase().as_str() {
            "binance" => Some(KeySlot::Binance),
            "binance_testnet" => Some(KeySlot::BinanceTestnet),
            "binance_demo" => Some(KeySlot::BinanceDemo),
            "upbit" => Some(KeySlot::Upbit),
            "bithumb" => Some(KeySlot::Bithumb),
            _ => None,
        }
    }
    /// 키체인 저장 항목의 `exchanges` 키.
    pub fn as_str(self) -> &'static str {
        match self {
            KeySlot::Binance => "binance",
            KeySlot::BinanceTestnet => "binance_testnet",
            KeySlot::BinanceDemo => "binance_demo",
            KeySlot::Upbit => "upbit",
            KeySlot::Bithumb => "bithumb",
        }
    }
    pub fn exchange(self) -> Exchange {
        match self {
            KeySlot::Binance | KeySlot::BinanceTestnet | KeySlot::BinanceDemo => Exchange::Binance,
            KeySlot::Upbit => Exchange::Upbit,
            KeySlot::Bithumb => Exchange::Bithumb,
        }
    }
    pub fn label(self) -> &'static str {
        match self {
            KeySlot::BinanceTestnet => "바이낸스 테스트넷",
            KeySlot::BinanceDemo => "바이낸스 데모",
            other => other.exchange().label(),
        }
    }
    /// 설정 › 거래소 연결의 칸 제목 옆 바로가기가 여는 거래소 사이트.
    pub fn site_url(self) -> &'static str {
        match self {
            KeySlot::Binance => "https://www.binance.com/",
            KeySlot::BinanceTestnet => "https://testnet.binance.vision/",
            KeySlot::BinanceDemo => "https://demo.binance.com/",
            KeySlot::Upbit => "https://upbit.com/exchange",
            KeySlot::Bithumb => "https://www.bithumb.com/",
        }
    }
    /// 연습 사이트 칸(현물 테스트넷·선물 데모) 여부 — 허용 IP 없이 연습 주소로 확인함.
    pub fn is_practice(self) -> bool {
        matches!(self, KeySlot::BinanceTestnet | KeySlot::BinanceDemo)
    }
    /// 이 칸으로 하는 연결 확인의 모드 — 연습 칸은 테스트넷 모드(연습 주소), 나머지는 실거래.
    pub fn check_mode(self) -> RunMode {
        if self.is_practice() { RunMode::Testnet } else { RunMode::Live }
    }
    /// 연결 도우미가 안내하는 바이낸스 사이트(`connection::binance_env`). 국내는 "".
    pub fn binance_env(self) -> &'static str {
        match self {
            KeySlot::Binance => "live",
            KeySlot::BinanceTestnet => "testnet",
            KeySlot::BinanceDemo => "demo",
            KeySlot::Upbit | KeySlot::Bithumb => "",
        }
    }
    /// 칸이 정하는 확인 시장 — 연습 칸은 사이트가 곧 시장(테스트넷 현물·데모 선물), 국내는 현물.
    /// 바이낸스 실거래 칸만 None — 한 키로 현물·선물을 모두 쓰므로 쓰는 실행에 따라 고름.
    pub fn fixed_market(self) -> Option<Market> {
        match self {
            KeySlot::Binance => None,
            KeySlot::BinanceDemo => Some(Market::Futures),
            KeySlot::BinanceTestnet | KeySlot::Upbit | KeySlot::Bithumb => Some(Market::Spot),
        }
    }
    /// 이 거래소·모드·시장으로 도는 매크로가 쓰는 칸. 바이낸스 연습은 현물이면 테스트넷 칸, 선물이면 데모 칸.
    /// 모의는 키가 없어도 돌지만 있으면 실거래 칸을 씀(실거래 주소로 주문 전 확인만 함).
    pub fn for_run(ex: Exchange, mode: RunMode, market: Market) -> KeySlot {
        match (ex, mode, market) {
            (Exchange::Binance, RunMode::Testnet, Market::Spot) => KeySlot::BinanceTestnet,
            (Exchange::Binance, RunMode::Testnet, Market::Futures) => KeySlot::BinanceDemo,
            (Exchange::Binance, ..) => KeySlot::Binance,
            (Exchange::Upbit, ..) => KeySlot::Upbit,
            (Exchange::Bithumb, ..) => KeySlot::Bithumb,
        }
    }
    /// 설정 화면의 연결 확인에 쓰는 기본 매크로(주문 없이 키·허용 IP·권한만 봄).
    pub fn probe_macro(self) -> Value {
        self.probe_macro_in(Market::Spot)
    }
    /// 시장을 고른 확인 매크로 — 바이낸스 선물 키(데모·실거래)는 선물 주소로 확인해야 함. 국내는 늘 현물.
    /// 선물은 레버리지 2배·100 USDT로 둠 — 선물 최소 주문 금액(100 USDT)에 걸려 키 확인 전에 멈추지 않게.
    pub fn probe_macro_in(self, market: Market) -> Value {
        match (self.exchange(), market) {
            (Exchange::Binance, Market::Futures) => json!({"symbol": "BTCUSDT", "exchange": "binance",
                "position_side": "long", "leverage": 2, "params": {"initial_capital": 100}}),
            (Exchange::Binance, Market::Spot) => json!({"symbol": "BTCUSDT", "exchange": "binance",
                "position_side": "long", "leverage": 1, "params": {"initial_capital": 10}}),
            (ex, _) => json!({"symbol": "KRW-BTC", "exchange": ex.as_str(), "position_side": "long",
                              "leverage": 1, "params": {"initial_capital": 10000}}),
        }
    }
    /// 바이낸스 연결 도우미가 안내하는 사이트(`connection::binance_env`)의 키 칸 — testnet·demo·live 그대로.
    #[cfg(test)]
    pub fn for_binance_env(env: &str) -> KeySlot {
        match env {
            "testnet" => KeySlot::BinanceTestnet,
            "demo" => KeySlot::BinanceDemo,
            _ => KeySlot::Binance,
        }
    }
}

/// 바이낸스 심볼 끝에서 찾는 견적 통화. 앞에서부터 처음 맞는 것을 씀.
const BINANCE_QUOTES: [&str; 6] = ["FDUSD", "USDT", "USDC", "BUSD", "BTC", "ETH"];

/// 심볼을 (기초 자산, 견적 통화)로 나눔. 앞뒤 공백을 빼고 대문자로 맞춤.
/// - 국내 `KRW-BTC` → (BTC, KRW)
/// - 바이낸스 `BTCUSDT` → (BTC, USDT). 견적 통화는 끝에서 [`BINANCE_QUOTES`] 순서로 찾고, 기초 자산이 비는
///   경우(`USDT` 하나뿐)는 건너뜀. USDT·USDC 밖의 견적도 같은 규칙(`ETHBTC` → (ETH, BTC))
/// - 견적 통화를 모르는 심볼(`BTCKRW` 등) → (심볼 전체, "")
pub fn split_symbol(symbol: &str) -> (String, &'static str) {
    let s = symbol.trim().to_uppercase();
    if let Some(rest) = s.strip_prefix("KRW-") {
        return (rest.to_string(), "KRW");
    }
    for q in BINANCE_QUOTES {
        if let Some(b) = s.strip_suffix(q).filter(|b| !b.is_empty()) {
            return (b.to_string(), q);
        }
    }
    (s, "")
}

/// 기초 자산 이름 — `BTCUSDT` → BTC, `KRW-BTC` → BTC. 견적 통화를 모르면 심볼 전체([`split_symbol`]).
pub fn base_asset(symbol: &str) -> String {
    split_symbol(symbol).0
}

/// 이름으로 라벨. 모르는 이름은 그대로 돌려줌.
pub fn exchange_label(name: &str) -> String {
    Exchange::parse(name).map(|e| e.label().to_string()).unwrap_or_else(|| name.to_string())
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RunMode {
    Mock,
    Testnet,
    Live,
}

impl RunMode {
    /// 모르는 값은 모의로 접음 — 설정이 깨졌을 때 실전으로 떨어지면 안 됨.
    pub fn parse_or_mock(s: &str) -> RunMode {
        match s.trim().to_lowercase().as_str() {
            "testnet" => RunMode::Testnet,
            "live" => RunMode::Live,
            _ => RunMode::Mock,
        }
    }
    pub fn as_str(self) -> &'static str {
        match self {
            RunMode::Mock => "mock",
            RunMode::Testnet => "testnet",
            RunMode::Live => "live",
        }
    }
    pub fn label(self) -> &'static str {
        match self {
            RunMode::Mock => "모의",
            RunMode::Testnet => "테스트넷",
            RunMode::Live => "실전",
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Market {
    #[default]
    Spot,
    Futures,
}

impl Market {
    pub fn as_str(self) -> &'static str {
        match self {
            Market::Spot => "spot",
            Market::Futures => "futures",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Side {
    Long,
    Short,
}

impl Side {
    pub fn parse(s: &str) -> Side {
        if s.trim().eq_ignore_ascii_case("short") { Side::Short } else { Side::Long }
    }
    pub fn as_str(self) -> &'static str {
        match self {
            Side::Long => "long",
            Side::Short => "short",
        }
    }
}

#[derive(Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct KeyPair {
    #[serde(default)]
    pub api_key: String,
    #[serde(default)]
    pub api_secret: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct IpObservation {
    pub public_ipv4: String,
    pub observed_at: f64,
}

/// 자격증명 v2 모양: {"version":2,"member_key","exchanges":{거래소:{api_key,api_secret}},"connection_history"}.
#[derive(Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Credentials {
    #[serde(default)]
    pub member_key: String,
    #[serde(default)]
    pub exchanges: BTreeMap<String, KeyPair>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub connection_history: BTreeMap<String, IpObservation>,
}

/// 디버그 출력·패닉 메시지에 키가 찍히지 않게 값은 감춤(채워졌는지만).
fn redacted(v: &str) -> &'static str {
    if v.trim().is_empty() { "<empty>" } else { "<redacted>" }
}

impl std::fmt::Debug for KeyPair {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("KeyPair")
            .field("api_key", &redacted(&self.api_key))
            .field("api_secret", &redacted(&self.api_secret))
            .finish()
    }
}

impl std::fmt::Debug for Credentials {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Credentials")
            .field("member_key", &redacted(&self.member_key))
            .field("exchanges", &self.exchanges)
            .field("connection_history", &self.connection_history.len())
            .finish()
    }
}

impl Credentials {
    /// 이 거래소의 키 한 쌍. 둘 중 하나라도 비면 None — '아직 안 넣었다'와 같음.
    pub fn pair(&self, ex: Exchange) -> Option<KeyPair> {
        let pair = self.exchanges.get(ex.as_str())?;
        let (k, s) = (pair.api_key.trim(), pair.api_secret.trim());
        if k.is_empty() || s.is_empty() {
            None
        } else {
            Some(KeyPair { api_key: k.to_string(), api_secret: s.to_string() })
        }
    }
}

/// 매크로가 고른 거래소 이름(소문자). 없으면 binance.
pub fn exchange_name_of(m: &Value) -> String {
    match m.get("exchange") {
        Some(Value::String(s)) if !s.trim().is_empty() => s.trim().to_lowercase(),
        _ => "binance".into(),
    }
}

pub fn macro_symbol(m: &Value) -> String {
    match m.get("symbol") {
        Some(Value::String(s)) => s.trim().to_uppercase(),
        Some(v) if !v.is_null() => v.to_string().trim_matches('"').to_uppercase(),
        _ => String::new(),
    }
}

/// 묶음(포트폴리오) 매크로의 레그 종목들. 묶음이 아니면 빈 목록.
///
/// **여기서 자금을 쪼개지 않는다.** 비중 셈은 서버가 하고 실행기는 시작 응답의
/// `leg_macro` 를 그대로 쓴다 — 양쪽이 따로 반올림하면 레그 금액의 합이 묶음 금액과
/// 어긋난다. 이 함수는 "몇 개로 나뉘고 각 줄의 이름이 무엇인가" 만 읽는다.
pub fn macro_leg_symbols(m: &Value) -> Vec<String> {
    if let Some(legs) = m.get("legs").and_then(Value::as_array) {
        let out: Vec<String> = legs
            .iter()
            .filter_map(|leg| leg.get("symbol").and_then(Value::as_str))
            .map(|s| s.trim().to_uppercase())
            .filter(|s| !s.is_empty())
            .collect();
        if out.len() > 1 {
            return out;
        }
        return vec![];
    }
    let out: Vec<String> = m
        .get("symbols")
        .and_then(Value::as_array)
        .map(|rows| {
            rows.iter()
                .filter_map(Value::as_str)
                .map(|s| s.trim().to_uppercase())
                .filter(|s| !s.is_empty())
                .collect()
        })
        .unwrap_or_default();
    if out.len() > 1 { out } else { vec![] }
}

/// 묶음 한도가 걸렸는가 — 레그를 갈라 돌릴 수 없는 묶음이다(서버도 같은 이유로 거절함).
pub fn macro_has_bundle_risk(m: &Value) -> bool {
    m.get("bundle_risk").is_some_and(|v| v.is_object())
}

pub fn macro_side(m: &Value) -> Side {
    m.get("position_side").and_then(Value::as_str).map(Side::parse).unwrap_or(Side::Long)
}

/// `max(1, int(leverage or 1))`. 숫자로 못 읽으면 오류.
pub fn macro_leverage(m: &Value) -> Result<u32, String> {
    let lev = match m.get("leverage") {
        None | Some(Value::Null) => 1.0,
        Some(Value::Bool(_)) => 1.0, // int(True)=1, int(False)=0 → 둘 다 max(1, ·)=1
        Some(Value::Number(n)) => n.as_f64().unwrap_or(1.0),
        Some(Value::String(s)) if s.trim().is_empty() => 1.0,
        Some(Value::String(s)) => s.trim().parse::<i64>().map_err(|_| "invalid macro leverage".to_string())? as f64,
        _ => return Err("invalid macro leverage".into()),
    };
    if !lev.is_finite() {
        return Err("invalid macro leverage".into());
    }
    let lev = lev.trunc();
    Ok(if lev < 1.0 { 1 } else { lev.min(1000.0) as u32 })
}

/// 지원하지 않는 거래소의 매크로 — 어디서 막히든 같은 문장으로 알림.
pub fn unsupported_exchange_message(name: &str) -> String {
    let name = if name.trim().is_empty() { "알 수 없음" } else { name.trim() };
    format!("이 앱이 지원하지 않는 거래소({name})예요. 바이낸스 · 업비트 · 빗썸 매크로만 실행할 수 있어요.")
}

/// 거래소 키가 없어 시작하지 못할 때 — 어디서 막히든 같은 문장으로 알림.
pub fn exchange_key_needed_message(label: &str) -> String {
    format!("{label} 거래소 키가 필요해요. 설정 › 거래소 연결에서 넣어 주세요.")
}

/// 호가 통화 — 국내는 `KRW-BTC`, 바이낸스는 `BTCUSDT`.
pub fn quote_of(symbol: &str) -> &'static str {
    if symbol.trim().to_uppercase().starts_with("KRW-") { "KRW" } else { "USDT" }
}

/// 1회 주문 상한. 상한은 늘 통화와 함께 다님.
pub fn order_cap(quote: &str) -> f64 {
    if quote.eq_ignore_ascii_case("KRW") { max_order_krw() } else { max_order_usdt() }
}

/// 거래소까지 본 시장. 국내는 원화 현물뿐.
pub fn market_of(exchange: &str, side: Side, leverage: u32) -> Market {
    if !exchange.eq_ignore_ascii_case("binance") {
        return Market::Spot;
    }
    if side == Side::Short || leverage > 1 { Market::Futures } else { Market::Spot }
}

/// 서버에 올리는 세션 시작 payload. 거래소 키·시크릿은 들어가지 않음.
pub fn session_payload(m: &Value, testnet: bool, mode: RunMode) -> Value {
    let side = macro_side(m);
    let lev = macro_leverage(m).unwrap_or(1);
    let exchange = exchange_name_of(m);
    json!({
        "symbol": macro_symbol(m),
        "exchange": exchange,
        "position_side": side.as_str(),
        "leverage": lev,
        "market": market_of(&exchange, side, lev).as_str(),
        "testnet": testnet,
        "mode": mode.as_str(),
        "human_summary": m.get("human_summary").cloned().unwrap_or(Value::String(String::new())),
        "runner_version": SERVER_LINK_LEVEL,
        // 이 앱에 묶여 있는 판단 엔진의 세대. 서버가 매크로마다 필요한 세대와 견줌.
        "engine_version": crate::engine::ENGINE_VERSION.to_string(),
        // 실행기는 스스로 판단할 줄 안다고 말함. 실제로 켤지는 **서버가** 정함
        // (응답의 "decides") — 양쪽이 동시에 판단하면 같은 신호로 두 번 사게 됨.
        "decides": "local",
        "macro": m,
    })
}

#[cfg(test)]
mod tests;

#[cfg(test)]
mod redact_tests;

#[cfg(test)]
mod cap_tests {
    use super::*;

    #[test]
    fn caps_from_the_environment_can_only_be_lowered() {
        // 환경변수를 바꾸지 않고 글자로 시험함 — set_var는 다른 시험 스레드의 읽기와 겹치면 안전하지 않음.
        assert_eq!(env_cap("GGP_TEST_CAP_UNSET", 100.0), 100.0);
        assert_eq!(lowered_cap(None, 100.0), 100.0);
        assert_eq!(lowered_cap(Some("1e9"), 100.0), 100.0, "높이는 값은 무시");
        assert_eq!(lowered_cap(Some(" 20 "), 100.0), 20.0);
        for bad in ["inf", "NaN", "0", "-5", "abc", ""] {
            assert_eq!(lowered_cap(Some(bad), 100.0), 100.0, "{bad:?}");
        }
    }
}

#[cfg(test)]
mod bundle_tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn weighted_legs_are_read_in_order() {
        let m = json!({"symbol": "BTCUSDT", "legs": [
            {"symbol": "btcusdt", "weight": 60}, {"symbol": "ETHUSDT", "weight": 40}]});
        assert_eq!(macro_leg_symbols(&m), vec!["BTCUSDT", "ETHUSDT"]);
    }

    #[test]
    fn an_equal_weight_bundle_written_as_symbols_is_also_a_bundle() {
        let m = json!({"symbol": "BTCUSDT", "symbols": ["BTCUSDT", "ETHUSDT", "SOLUSDT"]});
        assert_eq!(macro_leg_symbols(&m).len(), 3);
    }

    #[test]
    fn a_single_symbol_macro_has_no_legs() {
        assert!(macro_leg_symbols(&json!({"symbol": "BTCUSDT"})).is_empty());
        // 한 종목짜리 symbols·legs 는 묶음이 아니다 — 지금까지대로 한 줄로 돈다.
        assert!(macro_leg_symbols(&json!({"symbol": "BTCUSDT", "symbols": ["BTCUSDT"]})).is_empty());
        assert!(
            macro_leg_symbols(&json!({"symbol": "BTCUSDT", "legs": [{"symbol": "BTCUSDT", "weight": 100}]}))
                .is_empty()
        );
    }

    #[test]
    fn legs_win_over_symbols_like_the_server() {
        let m = json!({"symbol": "BTCUSDT",
                       "legs": [{"symbol": "AAAUSDT", "weight": 50}, {"symbol": "BBBUSDT", "weight": 50}],
                       "symbols": ["CCCUSDT", "DDDUSDT"]});
        assert_eq!(macro_leg_symbols(&m), vec!["AAAUSDT", "BBBUSDT"]);
    }

    #[test]
    fn a_bundle_limit_is_spotted() {
        assert!(macro_has_bundle_risk(&json!({"bundle_risk": {"max_positions": 1}})));
        assert!(!macro_has_bundle_risk(&json!({"bundle_risk": null})));
        assert!(!macro_has_bundle_risk(&json!({"symbol": "BTCUSDT"})));
    }
}
