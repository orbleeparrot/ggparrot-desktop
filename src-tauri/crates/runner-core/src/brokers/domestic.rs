//! 업비트·빗썸 원화 현물 어댑터.
//!
//! 현물·롱·1배만 있음. 수수료는 원화에서 빠지므로 체결 수량이 곧 보유 수량임.
use async_trait::async_trait;
use base64::Engine;
use hmac::{Hmac, KeyInit, Mac};
use rust_decimal::prelude::*;
use serde_json::{Map, Value};
use sha2::{Digest, Sha256, Sha512};
use std::str::FromStr;
use std::sync::Mutex;
use std::time::Duration;

use super::{
    cool, cooling, fee_inclusive, get_pos, is_close, lookup_delay, network_error, positive_f64, positive_number,
    py_str, py_truthy, retry_after_secs, Broker, BrokerError, FeePaid, LogFn, Order, OrderRules, OrderSide,
    SubmitRequest, DOMESTIC_BID_FEE_FALLBACK, LOOKUP_ATTEMPTS, TERMINAL_STATUSES,
};
use crate::money::{grouped, plain_number, price_text, qty_text};
use crate::types::{Exchange, Market, Side};

/// 요청 하나의 제한. 이 안에 답이 없어도 주문이 들어갔을 수 있음 — 재주문이 아니라 조회로 확인함.
const DOMESTIC_TIMEOUT: Duration = Duration::from_secs(10);

pub const UPBIT_BASE: &str = "https://api.upbit.com";
pub const BITHUMB_BASE: &str = "https://api.bithumb.com";

/// 두 거래소는 읽기 경로(/v1/...)가 같지만 쓰기·인증은 다름. 그 차이를 거래소별 명세로 둠.
#[derive(Debug)]
pub struct DomesticSpec {
    pub alg: &'static str,
    pub timestamp: bool,
    pub order_path: &'static str,
    pub type_key: &'static str,
    pub id_key: &'static str,
    pub id_max: usize,
}

const UPBIT_SPEC: DomesticSpec = DomesticSpec {
    alg: "HS512",
    timestamp: false,
    order_path: "/v1/orders",
    type_key: "ord_type",
    id_key: "identifier",
    id_max: 64,
};
const BITHUMB_SPEC: DomesticSpec = DomesticSpec {
    alg: "HS256",
    timestamp: true,
    order_path: "/v2/orders",
    type_key: "order_type",
    id_key: "client_order_id",
    id_max: 36,
};

pub fn domestic_spec(ex: Exchange) -> Option<&'static DomesticSpec> {
    match ex {
        Exchange::Upbit => Some(&UPBIT_SPEC),
        Exchange::Bithumb => Some(&BITHUMB_SPEC),
        Exchange::Binance => None,
    }
}

pub fn domestic_base(ex: Exchange) -> Option<&'static str> {
    match ex {
        Exchange::Upbit => Some(UPBIT_BASE),
        Exchange::Bithumb => Some(BITHUMB_BASE),
        Exchange::Binance => None,
    }
}

// --- JWT ------------------------------------------------------------------

fn b64(raw: &[u8]) -> String {
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(raw)
}

/// 파이썬 `json.dumps`의 문자열 모양(ensure_ascii) — 아스키 밖 글자는 \uXXXX.
fn json_str(s: &str) -> String {
    let encoded = serde_json::to_string(s).unwrap_or_else(|_| "\"\"".into());
    let mut out = String::with_capacity(encoded.len());
    for ch in encoded.chars() {
        if (ch as u32) < 0x7f {
            out.push(ch);
        } else {
            let mut buf = [0u16; 2];
            for unit in ch.encode_utf16(&mut buf) {
                out.push_str(&format!("\\u{unit:04x}"));
            }
        }
    }
    out
}

/// 쿼리 스트링(정렬 없이 보낸 순서, URL 인코딩 없이).
fn plain_query(query: &[(String, String)]) -> String {
    query.iter().map(|(k, v)| format!("{k}={v}")).collect::<Vec<_>>().join("&")
}

/// HMAC JWT. alg와 timestamp는 거래소마다 다름(업비트 HS512, 빗썸 HS256 + timestamp).
pub fn jwt_token(access: &str, secret: &str, query: Option<&[(String, String)]>, alg: &str, timestamp: bool) -> String {
    let header = b64(format!("{{\"alg\":{},\"typ\":\"JWT\"}}", json_str(alg)).as_bytes());
    let mut fields = vec![
        format!("\"access_key\":{}", json_str(access)),
        format!("\"nonce\":{}", json_str(&uuid::Uuid::new_v4().to_string())),
    ];
    if timestamp {
        fields.push(format!("\"timestamp\":{}", crate::clock::unix_ms()));
    }
    if let Some(q) = query.filter(|q| !q.is_empty()) {
        let hash = hex::encode(Sha512::digest(plain_query(q).as_bytes()));
        fields.push(format!("\"query_hash\":{}", json_str(&hash)));
        fields.push("\"query_hash_alg\":\"SHA512\"".to_string());
    }
    let body = b64(format!("{{{}}}", fields.join(",")).as_bytes());
    let signing_input = format!("{header}.{body}");
    let signature = if alg == "HS256" {
        let mut mac = Hmac::<Sha256>::new_from_slice(secret.as_bytes()).expect("hmac accepts any key");
        mac.update(signing_input.as_bytes());
        mac.finalize().into_bytes().to_vec()
    } else {
        let mut mac = Hmac::<Sha512>::new_from_slice(secret.as_bytes()).expect("hmac accepts any key");
        mac.update(signing_input.as_bytes());
        mac.finalize().into_bytes().to_vec()
    };
    format!("{signing_input}.{}", b64(&signature))
}

// --- 오류 설명 ---------------------------------------------------------------

pub(crate) mod error_names;
use error_names::{EXPIRED_KEY_ERRORS, IP_FIRST_ERRORS, MARKET_ERRORS, NO_FUNDS_ERRORS, OTHER_ACCESS_ERRORS};

/// 거래소 오류를 사람이 다음에 할 일이 보이는 문장으로 옮김. 모르는 오류는 거래소 말을 그대로 둠.
pub fn explain_domestic_error(ex: Exchange, status: u16, name: &str, message: &str) -> String {
    let label = ex.label();
    if IP_FIRST_ERRORS.contains(&name) {
        return format!(
            "{label}에서 요청을 거절했어요({name}). 먼저 이 기기의 현재 IP가 {label}의 \
             허용 IP에 등록돼 있는지 확인하세요. 집 인터넷 IP는 바뀔 수 있어요. IP가 맞다면 \
             API 키(Access Key · Secret Key)를 다시 복사해 넣으세요."
        );
    }
    if name == "invalid_query_payload" {
        return format!(
            "{label}에서 요청 서명을 검증하지 못했어요({name}). API 키나 허용 IP 문제가 아니라 \
             이 앱이 만든 요청의 문제예요. 개발팀에 알려 주세요."
        );
    }
    if name == "jwt_verification" {
        return format!(
            "{label}에서 인증 토큰(JWT) 서명을 검증하지 못했어요({name}). \
             Access Key와 Secret Key가 같은 발급 건인지, Secret Key를 빠짐없이 붙여 넣었는지 확인한 뒤 \
             다시 연결을 확인하세요. 계속 실패하면 개발팀에 알려 주세요."
        );
    }
    if name == "out_of_scope" {
        return format!("이 API 키에 주문 권한이 없어요({name}). {label} API 관리에서 주문하기 권한을 켠 키를 쓰세요.");
    }
    if name == "expired_jwt" {
        return format!(
            "요청 인증 토큰(JWT)이 만료됐어요({name}). API 키는 그대로 쓸 수 있어요. \
             PC 시계를 맞춘 뒤 다시 연결을 확인하세요. 계속 실패하면 개발팀에 알려 주세요."
        );
    }
    if EXPIRED_KEY_ERRORS.contains(&name) {
        return format!("API 키가 만료됐어요({name}). {label} API 관리에서 만료된 키를 삭제하고 새 키를 발급하세요.");
    }
    if name.starts_with("under_min_total") {
        return format!(
            "최소 주문 금액보다 적어요({name}). 매크로 투자 금액을 {label} 최소 주문 금액 이상으로 올리세요."
        );
    }
    if NO_FUNDS_ERRORS.contains(&name) {
        return format!("주문할 원화 잔고가 모자라요({name}). 잔고를 채우거나 매크로 투자 금액을 줄이세요.");
    }
    if name == "over_krw_funds_bid" {
        return format!("거래소의 1회 최대 주문 금액을 넘었어요({name}). 매크로 투자 금액을 줄이세요.");
    }
    if MARKET_ERRORS.contains(&name) {
        return format!("이 마켓은 지금 주문할 수 없어요({name}). 마켓 이름이 맞는지, 거래소 점검 중인지 확인하세요.");
    }
    let detail = if !name.is_empty() && !message.is_empty() {
        format!("{name}: {message}")
    } else if !name.is_empty() {
        name.to_string()
    } else if !message.is_empty() {
        message.to_string()
    } else {
        format!("HTTP {status}")
    };
    format!("{label}에서 거절했어요({detail})")
}

/// 키·허용 IP·권한 때문에 거절된 것인지 여부. 상태 코드만으로 가르지 않음(400짜리 허용 IP 오류가 있음).
pub fn is_access_error(status: u16, name: &str) -> bool {
    status == 401
        || status == 403
        || IP_FIRST_ERRORS.contains(&name)
        || EXPIRED_KEY_ERRORS.contains(&name)
        || OTHER_ACCESS_ERRORS.contains(&name)
}

// --- 숫자·상태 ---------------------------------------------------------------

/// `Decimal(repr(x)).quantize(10^-places, ROUND_DOWN)`를 지수 없이 쓴 글자. 0이면 None.
fn floor_plain(x: f64, places: u32) -> Option<String> {
    let text = format!("{x}");
    match Decimal::from_str(&text) {
        Ok(d) => {
            let q = d.round_dp_with_strategy(places, RoundingStrategy::ToZero);
            if q.is_zero() {
                None
            } else {
                Some(q.normalize().to_string())
            }
        }
        // Decimal 범위(28자리) 밖: 아주 작은 값은 0으로 내려가고, 아주 큰 값은 소수부가 없음.
        Err(_) => {
            if x.abs() < 1.0 {
                None
            } else {
                Some(text)
            }
        }
    }
}

/// 파이썬 `str(x or "")`.
fn py_word(value: Option<&Value>) -> String {
    match value {
        Some(v) if py_truthy(v) => py_str(v),
        _ => String::new(),
    }
}

/// 업비트·빗썸 state를 봇이 아는 상태로 접음. 답이 없으면 열렸는지조차 모르므로 UNKNOWN.
pub fn fold_domestic_status(state: Option<&Value>, executed_volume: f64) -> &'static str {
    let word = py_word(state);
    if word.is_empty() {
        return "UNKNOWN";
    }
    if word == "done" {
        return "FILLED";
    }
    if word == "cancel" {
        return if executed_volume > 0.0 { "FILLED" } else { "CANCELED" };
    }
    "OPEN"
}

/// 계좌 목록(/v1/accounts)의 원화 줄.
fn krw_row(accounts: &Value) -> Option<&Map<String, Value>> {
    accounts
        .as_array()?
        .iter()
        .filter_map(Value::as_object)
        .find(|a| a.get("currency").and_then(Value::as_str) == Some("KRW"))
}

/// 거래소 응답에서 체결 수량·평균가를 뽑음. 수수료는 원화에서 빠지므로 보유 수량은 체결 수량임.
pub fn normalize_domestic(order: &Value) -> Order {
    let empty = Map::new();
    let order = order.as_object().unwrap_or(&empty);
    let executed = get_pos(order, "executed_volume");
    let (mut funds, mut volume) = (0.0, 0.0);
    if let Some(Value::Array(trades)) = order.get("trades") {
        for trade in trades.iter().filter_map(Value::as_object) {
            let qty = get_pos(trade, "volume");
            volume += qty;
            let f = get_pos(trade, "funds");
            funds += if f != 0.0 { f } else { get_pos(trade, "price") * qty };
        }
    }
    let average = if volume != 0.0 { funds / volume } else { 0.0 };
    let fees_known = is_close(volume, executed);
    let order_id = match order.get("uuid") {
        Some(v) if py_truthy(v) => Some(py_str(v)),
        _ => order.get("order_id").map(py_str),
    };
    let raw = py_word(order.get("state"));
    // 낸 수수료(원화) — 주문 조회의 paid_fee. 칸이 없는 응답이면 모름.
    let fee =
        order.get("paid_fee").filter(|v| !v.is_null()).map(|v| FeePaid { quote: positive_number(v), other: vec![] });
    Order {
        status: fold_domestic_status(order.get("state"), executed).to_string(),
        executed_qty: executed,
        avg_price: average,
        acquired_qty: executed,
        fees_known,
        order_id,
        raw_status: if raw.is_empty() { "unknown".into() } else { raw },
        fee,
    }
}

// --- 어댑터 ------------------------------------------------------------------

#[derive(Clone, Copy, Debug, Default, PartialEq)]
struct Fees {
    bid: f64,
    ask: f64,
}

/// 주문 가능 정보(/v1/orders/chance)에 함께 오는 잔고 — 원화와 이 마켓의 코인.
#[derive(Clone, Debug, Default, PartialEq)]
struct Holdings {
    krw: Option<f64>,
    coin: Option<(String, f64)>,
}

pub struct DomesticBroker {
    access: String,
    secret: String,
    exchange: Exchange,
    symbol: String,
    log: Mutex<LogFn>,
    http: reqwest::Client,
    base: String,
    spec: &'static DomesticSpec,
    fees: Mutex<Fees>,
    rules: Mutex<Option<OrderRules>>,
    last_error_name: Mutex<String>,
    holdings: Mutex<Option<Holdings>>,
    /// 마지막 시세의 전일 대비 등락률(signed_change_rate).
    change: Mutex<Option<f64>>,
}

impl DomesticBroker {
    /// 마지막으로 읽은 (매수, 매도) 수수료율. 못 읽었으면 0.
    #[cfg(test)]
    pub fn fees(&self) -> (f64, f64) {
        let f = *self.fees.lock().unwrap();
        (f.bid, f.ask)
    }

    pub fn new(
        access: &str,
        secret: &str,
        exchange: Exchange,
        symbol: &str,
        log: LogFn,
        http: reqwest::Client,
        base: Option<String>,
    ) -> Result<Self, BrokerError> {
        let (Some(spec), Some(default_base)) = (domestic_spec(exchange), domestic_base(exchange)) else {
            return Err(BrokerError::Other(format!("지원하지 않는 국내 거래소예요: {}", exchange.as_str())));
        };
        Ok(DomesticBroker {
            access: access.to_string(),
            secret: secret.to_string(),
            exchange,
            symbol: symbol.to_string(),
            log: Mutex::new(log),
            http,
            base: base.unwrap_or_else(|| default_base.to_string()).trim_end_matches('/').to_string(),
            spec,
            fees: Mutex::new(Fees::default()),
            rules: Mutex::new(None),
            last_error_name: Mutex::new(String::new()),
            holdings: Mutex::new(None),
            change: Mutex::new(None),
        })
    }

    fn log(&self, msg: String) {
        let log = self.log.lock().unwrap().clone();
        log(msg);
    }

    fn set_last_error(&self, name: &str) {
        *self.last_error_name.lock().unwrap() = name.to_string();
    }

    /// 로그·오류 문장에 쓰는 거래소 이름(업비트·빗썸).
    fn label(&self) -> &'static str {
        self.exchange.label()
    }

    fn auth(&self, query: Option<&[(String, String)]>) -> String {
        format!("Bearer {}", jwt_token(&self.access, &self.secret, query, self.spec.alg, self.spec.timestamp))
    }

    fn url(&self, path: &str, params: &[(String, String)]) -> String {
        if params.is_empty() {
            return format!("{}{}", self.base, path);
        }
        let mut ser = url::form_urlencoded::Serializer::new(String::new());
        for (k, v) in params {
            ser.append_pair(k, v);
        }
        format!("{}{}?{}", self.base, path, ser.finish())
    }

    async fn send(&self, req: reqwest::RequestBuilder) -> Result<(u16, String), BrokerError> {
        let resp = req.timeout(DOMESTIC_TIMEOUT).send().await.map_err(network_error)?;
        let status = resp.status().as_u16();
        if status == 429 || status == 418 {
            // 업비트·빗썸은 초 단위 한도라 짧게 — Retry-After가 있으면 그만큼.
            let wait = retry_after_secs(resp.headers()).unwrap_or(if status == 418 { 60 } else { 2 }).clamp(1, 600);
            cool(&self.base, wait);
            self.log(format!("{} 요청 한도에 걸려 {wait}초 동안 요청을 쉬어요(HTTP {status}).", self.label()));
        }
        let text = resp.text().await.map_err(network_error)?;
        Ok((status, text))
    }

    /// 400 이상이면 거래소 오류 낱말을 들고 Api 오류.
    fn raise_for_error(status: u16, text: &str) -> Result<(), BrokerError> {
        if status < 400 {
            return Ok(());
        }
        let parsed: Value = serde_json::from_str(text).unwrap_or(Value::Null);
        let error = parsed.get("error").and_then(Value::as_object);
        let field = |key: &str| error.and_then(|e| e.get(key)).map(py_str).unwrap_or_default();
        Err(BrokerError::Api { status, name: field("name"), message: field("message") })
    }

    fn parse_json(text: &str) -> Result<Value, BrokerError> {
        serde_json::from_str(text).map_err(|e| BrokerError::Other(format!("거래소 응답을 읽지 못했어요: {e}")))
    }

    async fn get(&self, path: &str, params: &[(String, String)], private: bool) -> Result<Value, BrokerError> {
        // 요청 한도로 쉬는 중이면 공개 요청(시세)은 보내지 않음 — 같은 거래소로 나가는 모든 실행이 함께 쉼.
        // 주문·주문 조회는 막지 않음: 결과를 모르는 주문은 조회로 확인해야 함.
        if !private {
            if let Some(left) = cooling(&self.base) {
                return Err(BrokerError::Api {
                    status: 429,
                    name: "too_many_requests".into(),
                    message: format!("{} 요청 한도로 {left}초 쉬는 중이에요", self.label()),
                });
            }
        }
        let mut req = self.http.get(self.url(path, params));
        if private {
            req = req.header("Authorization", self.auth(Some(params)));
        }
        let (status, text) = self.send(req).await?;
        Self::raise_for_error(status, &text)?;
        Self::parse_json(&text)
    }

    async fn post(&self, path: &str, body: &[(String, String)]) -> Result<(u16, String), BrokerError> {
        let json = format!(
            "{{{}}}",
            body.iter().map(|(k, v)| format!("{}: {}", json_str(k), json_str(v))).collect::<Vec<_>>().join(", ")
        );
        let req = self
            .http
            .post(format!("{}{}", self.base, path))
            .header("Authorization", self.auth(Some(body)))
            .header("Content-Type", "application/json")
            .body(json);
        self.send(req).await
    }

    async fn load_chance(&self) -> Result<OrderRules, BrokerError> {
        let chance = self.get("/v1/orders/chance", &[("market".into(), self.symbol.clone())], true).await?;
        let empty = Map::new();
        let chance = chance.as_object().unwrap_or(&empty);
        *self.fees.lock().unwrap() = Fees { bid: get_pos(chance, "bid_fee"), ask: get_pos(chance, "ask_fee") };
        // step이 0인 이유: 시장가 매수는 원화 금액으로 내므로 수량 단위가 필요 없음.
        let min_total =
            chance.get("bid").and_then(Value::as_object).map(|bid| get_pos(bid, "min_total")).unwrap_or(0.0);
        let rules = OrderRules { step: 0.0, min_notional: min_total };
        *self.rules.lock().unwrap() = Some(rules);
        let account = |key: &str| chance.get(key).and_then(Value::as_object);
        let balance = |a: &Map<String, Value>| a.get("balance").and_then(|v| py_str(v).trim().parse::<f64>().ok());
        *self.holdings.lock().unwrap() = Some(Holdings {
            krw: account("bid_account").and_then(balance),
            coin: account("ask_account").and_then(|a| {
                let currency = a.get("currency").and_then(Value::as_str)?;
                Some((currency.to_string(), balance(a)?))
            }),
        });
        Ok(rules)
    }

    fn bid_rate(&self) -> f64 {
        let bid = self.fees.lock().unwrap().bid;
        if bid != 0.0 {
            bid
        } else {
            DOMESTIC_BID_FEE_FALLBACK
        }
    }

    /// 시장가 주문 본문(보낸 순서 그대로). submit과 리허설이 같은 함수로 만듦.
    pub fn order_body(
        &self,
        side: OrderSide,
        base_qty: Option<f64>,
        notional: Option<f64>,
        reduce_only: bool,
        client_id: &str,
    ) -> Result<Vec<(String, String)>, BrokerError> {
        let spec = self.spec;
        if reduce_only && side == OrderSide::Buy {
            return Err(BrokerError::Other("국내 현물은 롱만 있어 포지션 줄이기 매수를 쓸 수 없어요".into()));
        }
        let len = client_id.chars().count();
        if len == 0 || len > spec.id_max {
            return Err(BrokerError::Other(format!("주문 식별자는 1~{}자여야 해요", spec.id_max)));
        }
        let mut body = vec![("market".to_string(), self.symbol.clone())];
        match side {
            OrderSide::Buy => {
                let total = notional.map(positive_f64).unwrap_or(0.0);
                if total == 0.0 {
                    return Err(BrokerError::Other("시장가 매수에는 0보다 큰 원화 금액이 필요해요".into()));
                }
                // 1원 아래로 내림 — 올리면 주문 상한을 넘음.
                let Some(won) = floor_plain(total, 0) else {
                    return Err(BrokerError::Other("시장가 매수 금액이 1원보다 적어요".into()));
                };
                body.push(("side".into(), "bid".into()));
                body.push((spec.type_key.into(), "price".into()));
                body.push(("price".into(), won));
            }
            OrderSide::Sell => {
                let qty = base_qty.map(positive_f64).unwrap_or(0.0);
                // 8자리 아래로 내림 — 올리면 가진 것보다 많이 팔려는 주문이 됨.
                let Some(volume) = floor_plain(qty, 8) else {
                    return Err(BrokerError::Other("시장가 매도에는 0보다 큰 수량이 필요해요".into()));
                };
                body.push(("side".into(), "ask".into()));
                body.push((spec.type_key.into(), "market".into()));
                body.push(("volume".into(), volume));
            }
        }
        body.push((spec.id_key.into(), client_id.to_string()));
        Ok(body)
    }

    /// client_id로 주문 하나를 조회함. 실패나 객체가 아닌 답은 '답이 없음'(빈 객체)으로 봄.
    async fn lookup_order(&self, client_id: &str) -> Map<String, Value> {
        match self.get("/v1/order", &[(self.spec.id_key.into(), client_id.to_string())], true).await {
            Ok(Value::Object(found)) => found,
            _ => Map::new(),
        }
    }

    async fn rehearse_inner(&self, notional: f64, spend: Option<f64>) -> Result<(bool, String), BrokerError> {
        let spend_truthy = matches!(spend, Some(s) if s != 0.0);
        if self.exchange == Exchange::Upbit {
            let mut notional = notional;
            if spend_truthy {
                // 수수료율 — 검증 주문도 실제 주문 금액으로 냄(업비트 검증은 잔고까지 봄).
                self.load_chance().await?;
                notional = fee_inclusive(positive_f64(spend.unwrap_or(0.0)), self.bid_rate());
            }
            return self.rehearse_by_test_order(notional).await;
        }
        self.rehearse_by_reads(notional, if spend_truthy { spend } else { None }).await
    }

    async fn rehearse_by_test_order(&self, notional: f64) -> Result<(bool, String), BrokerError> {
        // 진짜 주문 경로를 쓰지 않음 — 리허설이 주문을 넣으면 안 됨.
        let hex12: String = uuid::Uuid::new_v4().simple().to_string().chars().take(12).collect();
        let body = self.order_body(OrderSide::Buy, None, Some(notional), false, &format!("ggp-rehearsal-{hex12}"))?;
        let (status, text) = self.post(&format!("{}/test", self.spec.order_path), &body).await?;
        Self::raise_for_error(status, &text)?;
        if status != 201 {
            return Ok((false, format!("업비트 주문 검증이 예상 밖 응답을 줬어요(HTTP {status})")));
        }
        Ok((true, "업비트 주문 검증을 통과했어요. 키 · 허용 IP · 주문 권한 · 최소 주문 금액 모두 문제없어요.".into()))
    }

    async fn rehearse_by_reads(&self, notional: f64, spend: Option<f64>) -> Result<(bool, String), BrokerError> {
        let accounts = self.get("/v1/accounts", &[], true).await?;
        let rules = self.load_chance().await?;
        let rate = self.bid_rate();
        let mut notional = notional;
        if let Some(spend) = spend {
            notional = fee_inclusive(positive_f64(spend), rate); // 실제 주문과 같은 금액
        }
        let amount = positive_f64(notional);
        let minimum = rules.min_notional;
        if minimum != 0.0 && amount < minimum {
            self.set_last_error("under_min_total");
            return Ok((
                false,
                format!(
                    "최소 주문 금액보다 적어요. 빗썸 최소 주문 금액은 {}원인데 이번 주문은 {}원이에요. 매크로 투자 금액을 올리세요.",
                    plain_number(minimum),
                    plain_number(amount)
                ),
            ));
        }
        // 빗썸에는 잔고까지 보는 검증 주문이 없음 — 주문 금액 + 수수료가 주문 가능 원화 안에 드는지 직접 봄.
        let krw = krw_row(&accounts);
        let available = krw.map(|k| get_pos(k, "balance")).unwrap_or(0.0);
        // 주문 가능 정보에 원화 잔고가 없으면 방금 읽은 계좌 목록으로 채움(잔고 한 줄에 씀).
        if let Some(h) = self.holdings.lock().unwrap().as_mut().filter(|h| h.krw.is_none()) {
            h.krw = krw.map(|_| available);
        }
        let need = amount * (1.0 + rate);
        if need > available {
            self.set_last_error("insufficient_funds");
            return Ok((
                false,
                format!(
                    "주문할 원화 잔고가 모자라요. 첫 주문에 수수료까지 {}원이 필요한데 \
                     주문 가능한 원화는 {}원이에요. 빗썸에 원화를 채우거나 매크로 투자 금액을 줄이세요.",
                    grouped(need.ceil(), 0),
                    grouped(available.floor(), 0)
                ),
            ));
        }
        Ok((
            true,
            "빗썸 키 · 허용 IP · 최소 주문 금액 · 원화 잔고를 확인했어요. \
             주문 권한은 빗썸에서 미리 확인할 수 없어 첫 주문 때 알 수 있어요."
                .into(),
        ))
    }
}

#[async_trait]
impl Broker for DomesticBroker {
    fn market(&self) -> Market {
        Market::Spot
    }
    fn side(&self) -> Side {
        Side::Long
    }
    fn symbol(&self) -> String {
        self.symbol.clone()
    }
    fn fee_from_base_asset(&self) -> bool {
        false
    }
    fn bid_fee(&self) -> Option<f64> {
        Some(self.fees.lock().unwrap().bid)
    }
    fn ask_fee(&self) -> Option<f64> {
        Some(self.fees.lock().unwrap().ask)
    }
    fn default_fee_rate(&self) -> f64 {
        crate::pnl::default_fee_rate(Some(self.exchange), Market::Spot)
    }
    fn last_error_name(&self) -> String {
        self.last_error_name.lock().unwrap().clone()
    }
    fn set_log(&self, log: LogFn) {
        *self.log.lock().unwrap() = log;
    }

    /// 키가 유효한지 원화 잔고 조회로 확인함. 실패하면 오류 — 호출자가 키 안내를 붙임.
    async fn check_account(&self) -> Result<(), BrokerError> {
        let accounts = self.get("/v1/accounts", &[], true).await?;
        let balance = match krw_row(&accounts) {
            Some(row) => match row.get("balance") {
                Some(v) => py_str(v),
                None => return Err(BrokerError::Other("'balance'".into())),
            },
            None => "조회 실패".into(),
        };
        self.log(format!("연결 성공 · 원화 잔고: {balance}"));
        Ok(())
    }

    async fn ensure_ready(&self) -> Result<bool, BrokerError> {
        self.load_market().await
    }

    /// 읽기뿐인 준비. 키·허용 IP 실패를 '마켓 없음'(거짓)으로 접지 않음.
    async fn load_market(&self) -> Result<bool, BrokerError> {
        match self.load_chance().await {
            Ok(_) => Ok(true),
            Err(BrokerError::Api { status, name, message }) => {
                if is_access_error(status, &name) {
                    let explanation = explain_domestic_error(self.exchange, status, &name, &message);
                    Err(BrokerError::Access { explanation, name })
                } else if status == 400 || status == 404 {
                    Ok(false)
                } else {
                    Err(BrokerError::Api { status, name, message })
                }
            }
            Err(other) => Err(other),
        }
    }

    async fn order_rules(&self) -> Result<OrderRules, BrokerError> {
        let cached = *self.rules.lock().unwrap();
        match cached {
            Some(rules) => Ok(rules),
            None => self.load_chance().await,
        }
    }

    /// 돈을 쓰지 않고 키·허용 IP·주문 권한·최소 주문 금액이 통하는지 봄.
    async fn rehearse(&self, notional: f64, spend: Option<f64>) -> (bool, String) {
        self.set_last_error("");
        match self.rehearse_inner(notional, spend).await {
            Ok(result) => result,
            Err(BrokerError::Api { status, name, message }) => {
                self.set_last_error(&name);
                (false, explain_domestic_error(self.exchange, status, &name, &message))
            }
            Err(BrokerError::Access { explanation, name }) => {
                self.set_last_error(&name);
                (false, explanation)
            }
            Err(BrokerError::Other(m)) => {
                (false, format!("주문 검증 요청을 만들지 못했어요: {m}. 매크로 내용을 확인하세요."))
            }
            Err(BrokerError::Network(m)) => {
                // 네트워크 끊김·시간 초과. 거래소 응답이 아니므로 키 탓으로 읽히지 않게 따로 말함.
                self.set_last_error("network");
                (false, format!("{}에 연결하지 못했어요: {m}", self.label()))
            }
        }
    }

    async fn price(&self) -> Result<f64, BrokerError> {
        let rows = self.get("/v1/ticker", &[("markets".into(), self.symbol.clone())], false).await?;
        let row = rows.as_array().and_then(|r| r.first());
        let value = row.and_then(|row| row.get("trade_price")).map(positive_number).unwrap_or(0.0);
        // 같은 응답의 전일 대비 등락률 — 화면에만 씀.
        *self.change.lock().unwrap() = row
            .and_then(|row| row.get("signed_change_rate"))
            .and_then(|v| py_str(v).trim().parse::<f64>().ok())
            .filter(|c| c.is_finite());
        if value == 0.0 {
            return Err(BrokerError::Other(format!("{} 현재가를 받지 못했어요", self.symbol)));
        }
        Ok(value)
    }

    /// 시장가 주문 하나를 넣고 끝난 상태까지 확인해서 돌려줌. POST 뒤에는 어떤 일이 있어도 오류를 내지 않음.
    async fn submit(&self, req: SubmitRequest) -> Result<Order, BrokerError> {
        let body = self.order_body(req.side, req.base_qty, req.notional, req.reduce_only, &req.client_id)?;

        let mut rejection = String::new();
        if let Ok((status, text)) = self.post(self.spec.order_path, &body).await {
            if (400..500).contains(&status) {
                // 거래소가 오류 이름을 달아 돌려준 4xx만 '받지 않음'으로 봄.
                if let Ok(parsed) = serde_json::from_str::<Value>(&text) {
                    rejection = py_word(parsed.get("error").and_then(|e| e.get("name")));
                }
            }
        }
        // 타임아웃·5xx·이름 없는 4xx는 거절의 증거가 아님. 같은 client_id로 조회할 뿐 재주문은 없음.

        let attempts = if rejection.is_empty() { LOOKUP_ATTEMPTS } else { 1 };
        let mut order = Map::new();
        for attempt in 0..attempts {
            let found = self.lookup_order(&req.client_id).await;
            if !found.is_empty() {
                order = found; // 빈 답은 덮지 않음 — 직전 응답에서 확인된 부분 체결을 유지함
            }
            let executed = get_pos(&order, "executed_volume");
            if TERMINAL_STATUSES.contains(&fold_domestic_status(order.get("state"), executed)) {
                break;
            }
            if attempt + 1 < attempts {
                tokio::time::sleep(lookup_delay(attempt)).await;
            }
        }
        if !rejection.is_empty() && order.is_empty() {
            return Ok(Order::rejected(rejection));
        }
        Ok(normalize_domestic(&Value::Object(order)))
    }

    /// 주문 가능 정보에서 읽은 원화 잔고와 이 코인 보유량.
    fn balance_note(&self) -> String {
        let Some(h) = self.holdings.lock().unwrap().clone() else {
            return String::new();
        };
        let Some(krw) = h.krw else {
            return String::new();
        };
        let mut text = format!("사용 가능 잔고 {}", price_text(krw.max(0.0).floor(), "KRW"));
        if let Some((coin, qty)) = h.coin.filter(|(_, q)| *q > 0.0) {
            text.push_str(&format!(" · 보유 {coin} {}", qty_text(qty)));
        }
        text
    }

    fn day_change(&self) -> Option<f64> {
        *self.change.lock().unwrap()
    }
}

#[cfg(test)]
mod tests;
