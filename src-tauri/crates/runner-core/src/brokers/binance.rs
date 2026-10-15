//! 바이낸스 현물·USDT-M 선물 어댑터(REST 직접 서명).
//!
//! python-binance를 쓰지 않음. 서명이 필요한 호출은 쿼리 스트링에 `timestamp`·`recvWindow=5000`을 붙이고
//! 그 글자 그대로의 HMAC-SHA256(hex) 값을 `signature`로 붙임. POST도 같은 쿼리 스트링으로 보냄(본문 없음).
use async_trait::async_trait;
use hmac::{Hmac, KeyInit, Mac};
use serde_json::{Map, Value};
use sha2::Sha256;
use std::collections::HashMap;
use std::sync::atomic::{AtomicI64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use super::{
    cool, cooling, exc_text, is_close, lookup_delay, network_error, positive_number, py_float_repr, py_str, py_truthy,
    retry_after_secs, Broker, BrokerError, LogFn, Order, OrderRules, SubmitRequest, LOOKUP_ATTEMPTS, TERMINAL_STATUSES,
};
use crate::types::{Market, Side};

pub const SPOT_BASE: &str = "https://api.binance.com";
pub const FUTURES_BASE: &str = "https://fapi.binance.com";
pub const SPOT_TESTNET_BASE: &str = "https://testnet.binance.vision";
pub const FUTURES_TESTNET_BASE: &str = "https://testnet.binancefuture.com";

/// 바이낸스가 주문 조회에 돌려주는 '더 조회해도 같은' 상태. TERMINAL_STATUSES보다 넓음.
/// 선물 거래소 정보(심볼별)를 함께 쓰는 시간.
const FUTURES_INFO_TTL: Duration = Duration::from_secs(300);

type FuturesInfo = Arc<HashMap<String, Value>>;

fn futures_info_cache() -> &'static Mutex<HashMap<String, (Instant, FuturesInfo)>> {
    static CACHE: std::sync::OnceLock<Mutex<HashMap<String, (Instant, FuturesInfo)>>> = std::sync::OnceLock::new();
    CACHE.get_or_init(Default::default)
}

/// 바이낸스 요청 하나의 제한 — 멈춘 연결이 손절·하트비트·정지를 붙잡지 않게.
const BINANCE_TIMEOUT: Duration = Duration::from_secs(10);
const BINANCE_SETTLED: [&str; 5] = ["FILLED", "CANCELED", "REJECTED", "EXPIRED", "EXPIRED_IN_MATCH"];

#[derive(Clone, Debug)]
pub struct BinanceUrls {
    pub spot: String,
    pub futures: String,
}

// --- 응답 읽기 도우미 ----------------------------------------------------------

/// 파이썬 `float(value)` — 문자열·숫자만. 읽지 못하면 None.
fn py_float(value: &Value) -> Option<f64> {
    match value {
        Value::Number(n) => n.as_f64(),
        Value::String(s) => s.trim().parse::<f64>().ok(),
        Value::Bool(b) => Some(if *b { 1.0 } else { 0.0 }),
        _ => None,
    }
}

/// 심볼 필터에서 (수량 단위, 최소 주문 금액)을 읽음.
pub fn parse_filters(info: &Value) -> (f64, f64) {
    let (mut step, mut min_notional) = (0.0, 0.0);
    let Some(filters) = info.get("filters").and_then(Value::as_array) else {
        return (step, min_notional);
    };
    for f in filters {
        let kind = f.get("filterType").and_then(Value::as_str).unwrap_or("");
        if (kind == "LOT_SIZE" || kind == "MARKET_LOT_SIZE") && step == 0.0 {
            step = f.get("stepSize").and_then(py_float).unwrap_or(0.0);
        } else if kind == "MIN_NOTIONAL" || kind == "NOTIONAL" {
            // f.get("minNotional", f.get("notional", 0)) or 0
            let raw = match f.get("minNotional") {
                Some(v) => v.clone(),
                None => f.get("notional").cloned().unwrap_or(Value::Null),
            };
            min_notional = if py_truthy(&raw) { py_float(&raw).unwrap_or(0.0) } else { 0.0 };
        }
    }
    (step, min_notional)
}

/// 거래소 상태를 봇이 아는 네 가지로 접음.
pub fn fold_status(raw: Option<&Value>) -> &'static str {
    let status = match raw {
        Some(v) if py_truthy(v) => py_str(v),
        _ => return "UNKNOWN",
    };
    if status == "EXPIRED" || status == "EXPIRED_IN_MATCH" {
        return "CANCELED";
    }
    TERMINAL_STATUSES.iter().find(|s| **s == status).copied().unwrap_or("OPEN")
}

/// 파이썬 `f"{x:g}"`.
pub fn py_g(x: f64) -> String {
    if x == 0.0 {
        return "0".into();
    }
    if !x.is_finite() {
        return py_float_repr(x);
    }
    let sci = format!("{x:.5e}");
    let (mantissa, exp) = sci.split_once('e').unwrap_or((&sci, "0"));
    let exp: i32 = exp.parse().unwrap_or(0);
    let strip = |s: &str| -> String {
        if s.contains('.') {
            s.trim_end_matches('0').trim_end_matches('.').to_string()
        } else {
            s.to_string()
        }
    };
    if !(-4..6).contains(&exp) {
        let sign = if exp < 0 { '-' } else { '+' };
        format!("{}e{sign}{:02}", strip(mantissa), exp.abs())
    } else {
        strip(&format!("{:.*}", (5 - exp) as usize, x))
    }
}

/// 주문 수량 글자 — 지수 없이, 끝의 0 없이.
fn fmt_qty(q: f64) -> String {
    format!("{q}")
}

fn settled(order: &Value) -> bool {
    order.get("status").and_then(Value::as_str).map(|s| BINANCE_SETTLED.contains(&s)).unwrap_or(false)
}

fn get_pos(obj: &Map<String, Value>, key: &str) -> f64 {
    obj.get(key).map(positive_number).unwrap_or(0.0)
}

fn now_ms() -> u128 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis()).unwrap_or(0)
}

// --- 어댑터 ------------------------------------------------------------------

pub struct BinanceBroker {
    api_key: String,
    api_secret: String,
    market: Market,
    symbol: String,
    side: Side,
    leverage: u32,
    log: Mutex<LogFn>,
    http: reqwest::Client,
    spot_base: String,
    futures_base: String,
    base_asset: Mutex<String>,
    rules: Mutex<OrderRules>,
    last_error_name: Mutex<String>,
    /// 거래소 시계 - 이 기기 시계(ms). 서명 요청의 timestamp에 더함(-1021 방지).
    time_offset: AtomicI64,
}

/// 주문이 거래소에 닿았는지 알 수 없는 오류 코드 — 거절의 증거가 아님.
/// -1000 알 수 없음·-1001 연결 끊김·-1006 예상 못 한 응답·-1007 처리 시간 초과·-1008 서버 과부하.
const BINANCE_AMBIGUOUS: [&str; 5] = ["-1000", "-1001", "-1006", "-1007", "-1008"];

/// 거래소가 주문을 받지 않았다고 분명히 답한 오류인지 여부(4xx + 숫자 코드, 모호한 코드 제외).
fn definite_rejection(e: &BrokerError) -> Option<String> {
    match e {
        BrokerError::Api { status, name, message }
            if (400..500).contains(status)
                && name.starts_with('-')
                && name[1..].chars().all(|c| c.is_ascii_digit())
                && !BINANCE_AMBIGUOUS.contains(&name.as_str()) =>
        {
            Some(if message.is_empty() { name.clone() } else { format!("{name} {message}") })
        }
        _ => None,
    }
}

impl BinanceBroker {
    /// 심볼 끝의 견적 통화(USDT·USDC 등). 모르면 빈 문자열 — 금액 뒤에 붙여 단위를 보임.
    fn quote_asset(&self) -> &str {
        ["FDUSD", "USDT", "USDC", "BUSD", "BTC", "ETH"]
            .into_iter()
            .find(|q| self.symbol.len() > q.len() && self.symbol.to_uppercase().ends_with(q))
            .unwrap_or("")
    }

    #[allow(clippy::too_many_arguments)]
    pub fn new(
        api_key: &str,
        api_secret: &str,
        market: Market,
        symbol: &str,
        side: Side,
        testnet: bool,
        leverage: u32,
        log: LogFn,
        http: reqwest::Client,
        urls: Option<BinanceUrls>,
    ) -> Self {
        let urls = urls.unwrap_or_else(|| {
            if testnet {
                BinanceUrls { spot: SPOT_TESTNET_BASE.into(), futures: FUTURES_TESTNET_BASE.into() }
            } else {
                BinanceUrls { spot: SPOT_BASE.into(), futures: FUTURES_BASE.into() }
            }
        });
        BinanceBroker {
            api_key: api_key.to_string(),
            api_secret: api_secret.to_string(),
            market,
            symbol: symbol.to_string(),
            side,
            leverage,
            log: Mutex::new(log),
            http,
            spot_base: urls.spot.trim_end_matches('/').to_string(),
            futures_base: urls.futures.trim_end_matches('/').to_string(),
            base_asset: Mutex::new(String::new()),
            rules: Mutex::new(OrderRules::default()),
            last_error_name: Mutex::new(String::new()),
            time_offset: AtomicI64::new(0),
        }
    }

    /// 거래소 시계와의 차이를 맞춤. 실패하면 그대로 둠(다음 -1021에서 다시 시도).
    async fn sync_time(&self) {
        let path = if self.futures() { "/fapi/v1/time" } else { "/api/v3/time" };
        let before = now_ms() as i64;
        if let Ok(v) = self.call_once(reqwest::Method::GET, self.futures(), path, &[], false).await {
            if let Some(server) = v.get("serverTime").and_then(Value::as_i64) {
                let local = (before + now_ms() as i64) / 2;
                self.time_offset.store(server - local, Ordering::Relaxed);
            }
        }
    }

    pub fn base_asset(&self) -> String {
        self.base_asset.lock().unwrap().clone()
    }

    fn futures(&self) -> bool {
        self.market == Market::Futures
    }

    fn log(&self, msg: String) {
        let log = self.log.lock().unwrap().clone();
        log(msg);
    }

    fn sign(&self, payload: &str) -> String {
        let mut mac = Hmac::<Sha256>::new_from_slice(self.api_secret.as_bytes()).expect("hmac accepts any key");
        mac.update(payload.as_bytes());
        hex::encode(mac.finalize().into_bytes())
    }

    /// REST 호출. 서명 요청이 시계 차이(-1021)로 거절되면 시계를 맞추고 한 번만 다시 보냄
    /// (-1021은 거래소가 요청을 처리하지 않았다는 뜻이라 같은 client id로 다시 보내도 안전함).
    async fn call(
        &self,
        method: reqwest::Method,
        futures: bool,
        path: &str,
        params: &[(&str, String)],
        signed: bool,
    ) -> Result<Value, BrokerError> {
        match self.call_once(method.clone(), futures, path, params, signed).await {
            Err(BrokerError::Api { name, .. }) if signed && name == "-1021" => {
                self.log("바이낸스와 PC 시계가 맞지 않아 시계를 맞춘 뒤 다시 보냈어요.".into());
                self.sync_time().await;
                self.call_once(method, futures, path, params, signed).await
            }
            other => other,
        }
    }

    /// REST 호출 하나. 2xx가 아니면 `{"code","msg"}`를 Api 오류로(name = 코드 글자).
    async fn call_once(
        &self,
        method: reqwest::Method,
        futures: bool,
        path: &str,
        params: &[(&str, String)],
        signed: bool,
    ) -> Result<Value, BrokerError> {
        let base = if futures { &self.futures_base } else { &self.spot_base };
        // 요청 한도로 쉬는 중이면 보내지 않음(보내지 않았으니 주문은 분명히 '받지 않음').
        // 같은 주소로 나가는 다른 실행이 걸린 한도도 함께 지킴.
        if cooling(base).is_some() {
            return Err(BrokerError::Api {
                status: 429,
                name: "-1003".into(),
                message: "바이낸스 요청 한도로 잠시 쉬는 중이에요".into(),
            });
        }
        let mut query = {
            let mut ser = url::form_urlencoded::Serializer::new(String::new());
            for (k, v) in params {
                ser.append_pair(k, v);
            }
            if signed {
                let ts = now_ms() as i64 + self.time_offset.load(Ordering::Relaxed);
                ser.append_pair("timestamp", &ts.to_string());
                ser.append_pair("recvWindow", "5000");
            }
            ser.finish()
        };
        if signed {
            let signature = self.sign(&query);
            query.push_str("&signature=");
            query.push_str(&signature);
        }
        let url = if query.is_empty() { format!("{base}{path}") } else { format!("{base}{path}?{query}") };
        let resp = self
            .http
            .request(method, url)
            .header("X-MBX-APIKEY", &self.api_key)
            .timeout(BINANCE_TIMEOUT)
            .send()
            .await
            .map_err(network_error)?;
        let status = resp.status().as_u16();
        let retry_after = retry_after_secs(resp.headers());
        let text = resp.text().await.map_err(network_error)?;
        if status == 429 || status == 418 {
            // 주문 개수 한도(-1015)는 계정 단위임 — 이 주소로 나가는 모든 실행의 시세 조회까지 쉬게 하지 않음.
            let order_rate = serde_json::from_str::<Value>(&text)
                .ok()
                .and_then(|b| b.get("code").and_then(Value::as_i64))
                .is_some_and(|c| c == -1015);
            if !order_rate {
                // Retry-After(초)만큼 쉼. 없으면 429는 30초, 418(IP 차단)은 2분.
                let wait = retry_after.unwrap_or(if status == 418 { 120 } else { 30 }).clamp(1, 3600);
                cool(base, wait);
                self.log(format!("바이낸스 요청 한도에 걸려 {wait}초 동안 요청을 쉬어요(HTTP {status})."));
            }
        }
        if !(200..300).contains(&status) {
            let (code, message) = match serde_json::from_str::<Value>(&text) {
                Ok(Value::Object(body)) => (
                    body.get("code").map(py_str).unwrap_or_else(|| "0".into()),
                    body.get("msg").map(py_str).unwrap_or_default(),
                ),
                _ => ("0".into(), format!("Invalid JSON error message from Binance: {text}")),
            };
            return Err(BrokerError::Api { status, name: code, message });
        }
        serde_json::from_str(&text)
            .map_err(|_| BrokerError::Other(format!("BinanceRequestException: Invalid Response: {text}")))
    }

    async fn get(
        &self,
        futures: bool,
        path: &str,
        params: &[(&str, String)],
        signed: bool,
    ) -> Result<Value, BrokerError> {
        self.call(reqwest::Method::GET, futures, path, params, signed).await
    }

    async fn post(&self, futures: bool, path: &str, params: &[(&str, String)]) -> Result<Value, BrokerError> {
        self.call(reqwest::Method::POST, futures, path, params, true).await
    }

    /// python-binance `get_symbol_info` — 없는 심볼(-1121)은 None.
    async fn spot_symbol_info(&self) -> Result<Option<Value>, BrokerError> {
        let res = match self.get(false, "/api/v3/exchangeInfo", &[("symbol", self.symbol.clone())], false).await {
            Ok(v) => v,
            Err(BrokerError::Api { name, .. }) if name == "-1121" => return Ok(None),
            Err(e) => return Err(e),
        };
        let wanted = self.symbol.to_uppercase();
        Ok(res
            .get("symbols")
            .and_then(Value::as_array)
            .and_then(|rows| rows.iter().find(|s| s.get("symbol").and_then(Value::as_str) == Some(wanted.as_str())))
            .cloned())
    }

    async fn futures_exchange_symbol(&self) -> Result<Option<Value>, BrokerError> {
        // 선물 거래소 정보는 심볼을 골라 받을 수 없어 전체(수백 개)가 옴 — 같은 주소는 잠시 함께 씀
        // (실행마다 시작 확인·준비에서 여러 번 받지 않게).
        let key = super::cool_key(&self.futures_base);
        if let Some(rows) = futures_info_cache()
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .get(&key)
            .and_then(|(at, rows)| (at.elapsed() < FUTURES_INFO_TTL).then(|| rows.clone()))
        {
            return Ok(rows.get(&self.symbol).cloned());
        }
        let info = self.get(true, "/fapi/v1/exchangeInfo", &[], false).await?;
        let rows: HashMap<String, Value> = info
            .get("symbols")
            .and_then(Value::as_array)
            .map(|rows| rows.iter().filter_map(|s| Some((s.get("symbol")?.as_str()?.to_string(), s.clone()))).collect())
            .unwrap_or_default();
        let rows = Arc::new(rows);
        futures_info_cache().lock().unwrap_or_else(|e| e.into_inner()).insert(key, (Instant::now(), rows.clone()));
        Ok(rows.get(&self.symbol).cloned())
    }

    async fn futures_balance_rows(&self) -> Result<Value, BrokerError> {
        self.get(true, "/fapi/v2/balance", &[], true).await
    }

    /// 선물 USDT 증거금. 조회 실패는 None, USDT 줄이 없으면 0.
    async fn futures_usdt_balance(&self) -> Option<f64> {
        let rows = self.futures_balance_rows().await.ok()?;
        for b in rows.as_array().map(Vec::as_slice).unwrap_or(&[]) {
            if b.get("asset").and_then(Value::as_str) == Some("USDT") {
                return match b.get("balance") {
                    None => Some(0.0),
                    Some(v) => py_float(v),
                };
            }
        }
        Some(0.0)
    }

    /// 격리 마진·레버리지를 맞춤. 맞추지 못하면 시작하지 않음 —
    /// 교차 마진이나 계정 기본 레버리지(예: 20배)로 주문이 나가면 손실 규모가 매크로와 달라짐.
    async fn configure_futures(&self) -> Result<(), BrokerError> {
        match self
            .post(true, "/fapi/v1/marginType", &[("symbol", self.symbol.clone()), ("marginType", "ISOLATED".into())])
            .await
        {
            Ok(_) => {}
            // -4046: 이미 격리 마진 — 원하는 상태임.
            Err(BrokerError::Api { name, .. }) if name == "-4046" => {}
            Err(e) => {
                let why = exc_text(&e);
                self.log(format!("⚠ 격리 마진 설정 실패({why})."));
                return Err(BrokerError::Access {
                    explanation: format!(
                        "바이낸스 선물을 격리 마진으로 바꾸지 못했어요({why}). 이 심볼에 열린 포지션이나 주문이 있으면 정리한 뒤 다시 시작하세요."
                    ),
                    name: "margin_type".into(),
                });
            }
        }
        if let Err(e) = self
            .post(
                true,
                "/fapi/v1/leverage",
                &[("symbol", self.symbol.clone()), ("leverage", self.leverage.to_string())],
            )
            .await
        {
            let why = exc_text(&e);
            self.log(format!("⚠ 레버리지 {}배 설정 실패({why}).", self.leverage));
            return Err(BrokerError::Access {
                explanation: format!(
                    "레버리지를 {}배로 바꾸지 못했어요({why}). 계정 기본 레버리지로 주문하지 않도록 시작을 멈췄어요.",
                    self.leverage
                ),
                name: "leverage".into(),
            });
        }
        Ok(())
    }

    /// 선물 리허설: 서명이 필요한 잔고 조회를 직접 불러 실패하면 그대로 나가게 함. 최소 주문 금액을 돌려줌.
    async fn rehearse_futures_reads(&self) -> Result<f64, BrokerError> {
        self.futures_balance_rows().await?;
        match self.futures_exchange_symbol().await? {
            Some(info) => Ok(parse_filters(&info).1),
            None => Err(BrokerError::Other(format!(
                "{} 심볼을 바이낸스에서 찾지 못했어요. 심볼 이름을 확인하세요.",
                self.symbol
            ))),
        }
    }

    async fn rehearse_reads(&self) -> Result<Result<f64, String>, BrokerError> {
        if self.futures() {
            return Ok(Ok(self.rehearse_futures_reads().await?));
        }
        self.check_account().await?;
        if !self.ensure_ready().await? {
            return Ok(Err(format!("{} 심볼을 바이낸스에서 찾지 못했어요. 심볼 이름을 확인하세요.", self.symbol)));
        }
        Ok(Ok(self.rules.lock().unwrap().min_notional))
    }

    /// 거래소 응답에서 체결 수량·평균가·실제 보유 수량을 뽑음. 보정 순서를 바꾸지 않음.
    pub async fn normalize(&self, order: &Value, closing: bool) -> Order {
        let empty = Map::new();
        let obj = order.as_object().unwrap_or(&empty);
        let executed = get_pos(obj, "executedQty");
        let mut average = get_pos(obj, "avgPrice");
        if average == 0.0 && executed != 0.0 {
            let quote = match obj.get("cummulativeQuoteQty") {
                Some(v) if py_truthy(v) => v.clone(),
                _ => obj.get("cumQuote").cloned().unwrap_or(Value::Null),
            };
            average = positive_number(&quote) / executed;
        }
        let response_fills: Vec<Value> = match obj.get("fills") {
            Some(Value::Array(rows)) => rows.clone(),
            _ => Vec::new(),
        };
        if average == 0.0 && executed != 0.0 {
            let quote: f64 = response_fills
                .iter()
                .filter_map(Value::as_object)
                .map(|f| get_pos(f, "price") * get_pos(f, "qty"))
                .sum();
            average = quote / executed;
        }

        let mut acquired = executed;
        let mut fees_known = true;
        if self.market == Market::Spot && !closing && executed != 0.0 {
            // ensure_ready() 전이라도 심볼에서 기초자산을 끌어냄 — 이게 없으면 수수료가 조용히 안 빠짐.
            let mut base_asset = self.base_asset();
            if base_asset.is_empty() && (self.symbol.ends_with("USDT") || self.symbol.ends_with("USDC")) {
                base_asset = self.symbol[..self.symbol.len() - 4].to_string();
            }
            let mut fills = response_fills;
            let order_id = obj.get("orderId").filter(|v| !v.is_null());
            if fills.is_empty() {
                if let Some(order_id) = order_id {
                    let id = py_str(order_id);
                    let params = [("symbol", self.symbol.clone()), ("orderId", id.clone()), ("limit", "1000".into())];
                    fills = match self.get(false, "/api/v3/myTrades", &params, true).await {
                        Ok(Value::Array(trades)) => trades
                            .into_iter()
                            .filter(|t| py_str(t.get("orderId").unwrap_or(&Value::Null)) == id)
                            .collect(),
                        _ => Vec::new(),
                    };
                }
            }
            fees_known = !base_asset.is_empty()
                && !fills.is_empty()
                && fills.iter().all(|f| {
                    f.as_object()
                        .map(|o| o.contains_key("commissionAsset") && o.contains_key("commission"))
                        .unwrap_or(false)
                });
            let filled: f64 = fills.iter().filter_map(Value::as_object).map(|f| get_pos(f, "qty")).sum();
            fees_known = fees_known && is_close(filled, executed);
            if fees_known {
                let base_fee: f64 = fills
                    .iter()
                    .filter_map(Value::as_object)
                    .filter(|f| f.get("commissionAsset").and_then(Value::as_str) == Some(base_asset.as_str()))
                    .map(|f| get_pos(f, "commission"))
                    .sum();
                acquired = (executed - base_fee).max(0.0);
            }
        }
        Order {
            status: fold_status(obj.get("status")).to_string(),
            executed_qty: executed,
            avg_price: average,
            acquired_qty: acquired,
            fees_known,
            order_id: obj.get("orderId").filter(|v| !v.is_null()).map(py_str),
            raw_status: obj.get("status").map(py_str).unwrap_or_else(|| "unknown".into()),
        }
    }
}

#[async_trait]
impl Broker for BinanceBroker {
    fn market(&self) -> Market {
        self.market
    }
    fn side(&self) -> Side {
        self.side
    }
    fn symbol(&self) -> String {
        self.symbol.clone()
    }
    /// 현물 매수 수수료는 받은 기초자산에서 빠짐. 선물은 증거금에서 빠져 수량이 그대로임.
    fn fee_from_base_asset(&self) -> bool {
        self.market == Market::Spot
    }
    fn bid_fee(&self) -> Option<f64> {
        None
    }
    fn last_error_name(&self) -> String {
        self.last_error_name.lock().unwrap().clone()
    }
    fn set_log(&self, log: LogFn) {
        *self.log.lock().unwrap() = log;
    }

    /// 계정이 이 시장에서 쓸 수 있는 키인지 확인하고 잔고를 알림.
    async fn check_account(&self) -> Result<(), BrokerError> {
        if self.futures() {
            let bal = self.futures_usdt_balance().await;
            let text = bal.map(py_float_repr).unwrap_or_else(|| "조회 실패".into());
            self.log(format!("연결 성공 · 선물 USDT 증거금: {text}"));
        } else {
            let acc = self.get(false, "/api/v3/account", &[], true).await?;
            let Some(balances) = acc.get("balances") else {
                return Err(BrokerError::Other("'balances'".into()));
            };
            let usdt = balances
                .as_array()
                .and_then(|rows| rows.iter().find(|b| b.get("asset").and_then(Value::as_str) == Some("USDT")));
            let text = match usdt {
                Some(row) => match row.get("free") {
                    Some(v) => py_str(v),
                    None => return Err(BrokerError::Other("'free'".into())),
                },
                None => "조회 실패".into(),
            };
            self.log(format!("연결 성공 · 현물 USDT 잔고: {text}"));
        }
        Ok(())
    }

    /// 심볼 규격을 읽고 선물이면 마진·레버리지를 맞춤. 심볼이 없으면 거짓.
    async fn ensure_ready(&self) -> Result<bool, BrokerError> {
        self.sync_time().await;
        if !self.load_market().await? {
            return Ok(false);
        }
        if self.futures() {
            self.configure_futures().await?;
        }
        Ok(true)
    }

    /// 읽기만 함 — 심볼 규격과 기초자산. 계정 설정은 건드리지 않음.
    async fn load_market(&self) -> Result<bool, BrokerError> {
        let info = if self.futures() {
            match self.futures_exchange_symbol().await {
                Ok(Some(info)) => info,
                Ok(None) => return Ok(false),
                // 조회 실패를 '심볼이 없어요'로 잘못 알리지 않음.
                Err(e) => {
                    self.log(format!("선물 심볼정보 조회 실패: {}", exc_text(&e)));
                    return Err(BrokerError::Access {
                        explanation: format!(
                            "바이낸스 선물 심볼 정보를 받지 못했어요({}). 인터넷 연결을 확인한 뒤 다시 시작하세요.",
                            exc_text(&e)
                        ),
                        name: "symbol_info".into(),
                    });
                }
            }
        } else {
            let Some(info) = self.spot_symbol_info().await? else {
                return Ok(false);
            };
            *self.base_asset.lock().unwrap() = info.get("baseAsset").and_then(Value::as_str).unwrap_or("").to_string();
            info
        };
        let (step, min_notional) = parse_filters(&info);
        *self.rules.lock().unwrap() = OrderRules { step, min_notional };
        Ok(true)
    }

    async fn order_rules(&self) -> Result<OrderRules, BrokerError> {
        Ok(*self.rules.lock().unwrap())
    }

    /// 주문 없이 검증하는 호출이 없어 계정 확인과 주문 규격 읽기로 갈음함.
    async fn rehearse(&self, notional: f64, _spend: Option<f64>) -> (bool, String) {
        *self.last_error_name.lock().unwrap() = String::new();
        let minimum = match self.rehearse_reads().await {
            Ok(Ok(minimum)) => minimum,
            Ok(Err(reason)) => return (false, reason),
            Err(e) => {
                // -2015는 '키·허용 IP·권한' 중 무엇인지 거래소가 가르지 않고 한 코드로 돌려줌.
                if let BrokerError::Api { name, .. } = &e {
                    if name == "-2014" || name == "-2015" {
                        *self.last_error_name.lock().unwrap() = "binance_access".into();
                        return (
                            false,
                            format!(
                                "바이낸스에서 키를 거절했어요({}). 키 · 허용 IP · 권한 설정을 확인하세요.",
                                exc_text(&e)
                            ),
                        );
                    }
                }
                return (false, format!("바이낸스 확인에 실패했어요: {}", exc_text(&e)));
            }
        };
        let amount =
            serde_json::Number::from_f64(notional).map(Value::Number).map(|v| positive_number(&v)).unwrap_or(0.0);
        if minimum != 0.0 && amount < minimum {
            *self.last_error_name.lock().unwrap() = "under_min_total".into();
            return (
                false,
                format!(
                    "최소 주문 금액보다 적어요(바이낸스 최소 {}{q} · 이번 주문 {}{q}). 매크로 투자 금액을 올리세요.",
                    py_g(minimum),
                    py_g(amount),
                    q = match self.quote_asset() {
                        "" => String::new(),
                        q => format!(" {q}"),
                    }
                ),
            );
        }
        (true, "바이낸스 계정과 주문 규격을 확인했어요. 주문 권한은 첫 주문 때 알 수 있어요.".into())
    }

    async fn price(&self) -> Result<f64, BrokerError> {
        let res = if self.futures() {
            self.get(true, "/fapi/v1/ticker/price", &[("symbol", self.symbol.clone())], false).await?
        } else {
            self.get(false, "/api/v3/ticker/price", &[("symbol", self.symbol.clone())], false).await?
        };
        let Some(raw) = res.get("price") else {
            return Err(BrokerError::Other("'price'".into()));
        };
        py_float(raw).ok_or_else(|| BrokerError::Other(format!("could not convert string to float: '{}'", py_str(raw))))
    }

    /// 시장가 주문 하나를 넣고 끝난 상태까지 확인해서 돌려줌. 타임아웃은 거절의 증거가 아님.
    async fn submit(&self, req: SubmitRequest) -> Result<Order, BrokerError> {
        let opening_word = if self.side == Side::Long { "BUY" } else { "SELL" };
        let closing = req.closing.unwrap_or(req.reduce_only || req.side.as_str() != opening_word);
        let futures = self.futures();
        // 바이낸스는 수량으로 주문함. notional은 국내 시장가 매수용 — 여기서는 안 씀.
        let mut params: Vec<(&str, String)> =
            vec![("symbol", self.symbol.clone()), ("side", req.side.as_str().into()), ("type", "MARKET".into())];
        if let Some(qty) = req.base_qty {
            params.push(("quantity", fmt_qty(qty)));
        }
        params.push(("newClientOrderId", req.client_id.clone()));
        params.push(("newOrderRespType", if futures { "RESULT" } else { "FULL" }.into()));
        if req.reduce_only {
            params.push(("reduceOnly", "true".into()));
        }
        let path = if futures { "/fapi/v1/order" } else { "/api/v3/order" };
        // 타임아웃·5xx·모호한 코드는 거절의 증거가 아님. 같은 client_id로 조회할 뿐, 두 번째 시장가 주문은 없음.
        // 거래소가 숫자 코드로 분명히 거절한 4xx(잔고 부족·최소 금액·키 권한 등)는 그 이유를 그대로 남김.
        let mut rejection: Option<String> = None;
        let mut order = match self.post(futures, path, &params).await {
            Ok(v) => v,
            Err(e) => {
                rejection = definite_rejection(&e);
                Value::Object(Map::new())
            }
        };
        let lookup = [("symbol", self.symbol.clone()), ("origClientOrderId", req.client_id.clone())];
        if let Some(reason) = rejection {
            // 거절이면 한 번만 조회해 정말 없는지 확인함(있으면 그 상태를 따름).
            if let Ok(found) = self.get(futures, path, &lookup, true).await {
                if found.as_object().is_some_and(|o| !o.is_empty()) {
                    return Ok(self.normalize(&found, closing).await);
                }
            }
            self.log(format!("바이낸스에서 주문을 거절했어요({reason})."));
            return Ok(Order {
                status: "REJECTED".into(),
                executed_qty: 0.0,
                avg_price: 0.0,
                acquired_qty: 0.0,
                fees_known: true,
                order_id: None,
                raw_status: reason,
            });
        }
        for attempt in 0..LOOKUP_ATTEMPTS {
            if settled(&order) {
                break;
            }
            if let Ok(found) = self.get(futures, path, &lookup, true).await {
                order = found; // 실패하면 직전 응답에서 확인된 부분 체결을 유지함
            }
            if !settled(&order) && attempt + 1 < LOOKUP_ATTEMPTS {
                tokio::time::sleep(lookup_delay(attempt)).await;
            }
        }
        Ok(self.normalize(&order, closing).await)
    }
}

#[cfg(test)]
mod tests;
