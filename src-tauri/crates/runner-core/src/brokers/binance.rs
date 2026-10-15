//! 바이낸스 현물·USDT-M 선물 어댑터(REST 직접 서명).
//!
//! python-binance를 쓰지 않음. 서명이 필요한 호출은 쿼리 스트링에 `timestamp`·`recvWindow=5000`을 붙이고
//! 그 글자 그대로의 HMAC-SHA256(hex) 값을 `signature`로 붙임. POST도 같은 쿼리 스트링으로 보냄(본문 없음).
use async_trait::async_trait;
use serde_json::{Map, Value};
use std::collections::HashMap;
use std::sync::atomic::AtomicI64;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use super::{
    exc_text, lookup_delay, positive_f64, py_float_repr, py_str, Broker, BrokerError, LogFn, Order, OrderRules,
    SubmitRequest, LOOKUP_ATTEMPTS,
};
use crate::money::{plain_number, price_text, py_g, qty_text};
use crate::types::{split_symbol, Market, Side};

pub const SPOT_BASE: &str = "https://api.binance.com";
pub const FUTURES_BASE: &str = "https://fapi.binance.com";
pub const SPOT_TESTNET_BASE: &str = "https://testnet.binance.vision";
/// 선물 연습 주소 — 선물 연습 키는 demo.binance.com에서만 만들므로 공식 데모 주소로 보냄.
pub const FUTURES_DEMO_BASE: &str = "https://demo-fapi.binance.com";

/// 선물 거래소 정보(심볼별)를 함께 쓰는 시간.
const FUTURES_INFO_TTL: Duration = Duration::from_secs(300);

type FuturesInfo = Arc<HashMap<String, Value>>;

fn futures_info_cache() -> &'static Mutex<HashMap<String, (Instant, FuturesInfo)>> {
    static CACHE: std::sync::OnceLock<Mutex<HashMap<String, (Instant, FuturesInfo)>>> = std::sync::OnceLock::new();
    CACHE.get_or_init(Default::default)
}

#[derive(Clone, Debug)]
pub struct BinanceUrls {
    pub spot: String,
    pub futures: String,
}

mod http;
mod parse;
mod rehearsal;

use parse::{asset_row, futures_available, num_of, settled, spot_free};
pub use parse::{fold_status, parse_filters, spot_fills_fee};

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
    /// 마지막 리허설에서 거래소가 돌려준 오류 코드("-2015" 등, 지원용 — 문장에는 넣지 않음). 없으면 "".
    last_error_code: Mutex<String>,
    /// 거래소 시계 - 이 기기 시계(ms). 서명 요청의 timestamp에 더함(-1021 방지).
    time_offset: AtomicI64,
    /// 연습 환경(현물 테스트넷·선물 데모) 키인지. 연습 환경에는 IP 제한·키 권한 조회가 없음.
    testnet: bool,
    /// 마지막으로 읽은 잔고 목록(현물 balances·선물 잔고). 시작 전 잔고 확인과 잔고 한 줄에 씀.
    balances: Mutex<Option<Value>>,
}

/// 키 권한 조회(실거래 전용) — IP 제한·주문 권한을 읽음.
const API_RESTRICTIONS_PATH: &str = "/sapi/v1/account/apiRestrictions";
const REHEARSE_OK: &str = "바이낸스 계정과 주문 규격을 확인했어요. 주문 권한은 첫 주문 때 알 수 있어요.";

/// 주문이 거래소에 닿았는지 알 수 없는 오류 코드 — 거절의 증거가 아님.
/// -1000 알 수 없음·-1001 연결 끊김·-1006 예상 못 한 응답·-1007 처리 시간 초과·-1008 서버 과부하.
const BINANCE_AMBIGUOUS: [&str; 5] = ["-1000", "-1001", "-1006", "-1007", "-1008"];

/// 거래소가 주문을 받지 않았다고 분명히 답한 오류인지 여부(4xx + 숫자 코드, 모호한 코드 제외).
fn definite_rejection(e: &BrokerError) -> Option<String> {
    match e {
        BrokerError::Api { status, name, message }
            if (400..500).contains(status) && is_code(name) && !BINANCE_AMBIGUOUS.contains(&name.as_str()) =>
        {
            Some(if message.is_empty() { name.clone() } else { format!("{name} {message}") })
        }
        _ => None,
    }
}

/// 숫자 오류 코드("-2015")인지 여부.
fn is_code(name: &str) -> bool {
    name.len() > 1 && name.starts_with('-') && name[1..].chars().all(|c| c.is_ascii_digit())
}

/// 바이낸스 오류를 (문장에 넣을 거래소 설명, 오류 코드)로 나눔. 코드는 문장에 넣지 않고 자세히 보기·원문 로그로 보냄.
/// 숫자 코드가 없는 오류(네트워크 등)는 (오류 글 그대로, "").
pub(crate) fn binance_error_parts(e: &BrokerError) -> (String, String) {
    match e {
        BrokerError::Api { status, name, message } if is_code(name) => {
            let text = if message.is_empty() { format!("HTTP {status}") } else { message.clone() };
            (text, name.clone())
        }
        other => (other.to_string(), String::new()),
    }
}

/// 거절된 주문의 거래소 상태 글([`definite_rejection`]의 "-2010 Account has …")을 (오류 코드, 설명)으로 나눔.
/// 코드 모양이 아니면 코드 없이 글 전체가 설명임.
pub fn binance_rejection_parts(raw: &str) -> (&str, &str) {
    let (head, rest) = raw.split_once(' ').unwrap_or((raw, ""));
    if is_code(head) {
        (head, rest.trim())
    } else {
        ("", raw.trim())
    }
}

/// 잔고(증거금)가 모자라 거절된 주문인지 여부 — 선물 -2019(Margin is insufficient), 현물 -2010 중 잔고 부족 설명.
pub fn is_binance_no_funds(code: &str, message: &str) -> bool {
    code == "-2019" || (code == "-2010" && message.to_ascii_lowercase().contains("insufficient balance"))
}

impl BinanceBroker {
    /// 심볼 끝의 견적 통화(USDT·USDC 등). 모르면 빈 문자열 — 금액 뒤에 붙여 단위를 보임.
    fn quote_asset(&self) -> &'static str {
        split_symbol(&self.symbol).1
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
                BinanceUrls { spot: SPOT_TESTNET_BASE.into(), futures: FUTURES_DEMO_BASE.into() }
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
            last_error_code: Mutex::new(String::new()),
            time_offset: AtomicI64::new(0),
            testnet,
            balances: Mutex::new(None),
        }
    }

    fn set_error_name(&self, name: &str) {
        *self.last_error_name.lock().unwrap() = name.into();
    }

    fn set_error_code(&self, code: &str) {
        *self.last_error_code.lock().unwrap() = code.into();
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
        let rows = self.get(true, "/fapi/v2/balance", &[], true).await?;
        *self.balances.lock().unwrap() = Some(rows.clone());
        Ok(rows)
    }

    /// 선물 USDT 사용 가능 증거금(주문에 쓸 수 있는 몫). 조회 실패는 None, USDT 줄이 없으면 0.
    async fn futures_usdt_balance(&self) -> Option<f64> {
        let rows = self.futures_balance_rows().await.ok()?;
        match asset_row(&rows, "USDT") {
            Some(row) if row.contains_key("availableBalance") || row.contains_key("balance") => futures_available(row),
            _ => Some(0.0),
        }
    }

    /// 견적 통화 이름 — 모르면 USDT.
    fn quote_or_usdt(&self) -> &str {
        match self.quote_asset() {
            "" => "USDT",
            q => q,
        }
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
                self.log(format!("⚠ 격리 마진 설정 실패({}).", exc_text(&e)));
                let (why, _) = binance_error_parts(&e);
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
            self.log(format!("⚠ 레버리지 {}배 설정 실패({}).", self.leverage, exc_text(&e)));
            let (why, _) = binance_error_parts(&e);
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
    fn last_error_code(&self) -> String {
        self.last_error_code.lock().unwrap().clone()
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
            *self.balances.lock().unwrap() = Some(balances.clone());
            let text = match asset_row(balances, "USDT") {
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
                            binance_error_parts(&e).0
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
    /// 실거래는 그 뒤 키 권한 조회로 허용 IP·주문 권한까지 봄(`check_live_restrictions`).
    async fn rehearse(&self, notional: f64, _spend: Option<f64>) -> (bool, String) {
        self.set_error_name("");
        self.set_error_code("");
        *self.balances.lock().unwrap() = None;
        let minimum = match self.rehearse_reads().await {
            Ok(Ok(minimum)) => minimum,
            Ok(Err(reason)) => return (false, reason),
            Err(e) => {
                let (why, code) = binance_error_parts(&e);
                self.set_error_code(&code);
                if let BrokerError::Api { name, .. } = &e {
                    // 키 형식(-2014)·서명 = 시크릿 짝(-1022)·없는 키(-2008) — 키를 다시 복사하면 됨.
                    if ["-2014", "-1022", "-2008"].contains(&name.as_str()) {
                        self.set_error_name("binance_key");
                        return (
                            false,
                            "바이낸스에서 키를 받지 않았어요. API Key · Secret Key를 다시 복사해 넣으세요.".into(),
                        );
                    }
                    // -2015는 '키·허용 IP·권한' 중 무엇인지 거래소가 가르지 않고 한 코드로 돌려줌.
                    // 테스트넷·데모에는 IP 제한이 없으니 키(또는 다른 환경에서 만든 키)임.
                    if name == "-2015" {
                        self.set_error_name(if self.testnet { "binance_access_test" } else { "binance_access" });
                        let hint = if self.testnet {
                            "이 사이트(테스트넷 · 데모)에서 만든 키인지 확인하세요."
                        } else {
                            "키 · 허용 IP · 권한 설정을 확인하세요."
                        };
                        return (false, format!("바이낸스에서 키를 거절했어요. {hint}"));
                    }
                }
                return (false, format!("바이낸스 확인에 실패했어요: {why}"));
            }
        };
        let amount = positive_f64(notional);
        if minimum != 0.0 && amount < minimum {
            self.set_error_name("under_min_total");
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
        if self.testnet {
            if let Some(short) = self.funds_shortfall(amount) {
                self.set_error_name("binance_funds_test");
                return (false, short);
            }
            self.set_error_name("binance_test_ok");
            return (true, REHEARSE_OK.into());
        }
        // 잔고는 키 권한을 확인한 뒤에 봄 — 모자라도 허용 IP·주문 권한 결과는 남기려고.
        let (ok, reason) = self.check_live_restrictions().await;
        if ok {
            if let Some(short) = self.funds_shortfall(amount) {
                let name = match self.last_error_name().as_str() {
                    "binance_verified" => "binance_funds",
                    "binance_verified_open" => "binance_funds_open",
                    _ => "insufficient_funds",
                };
                self.set_error_name(name);
                return (false, short);
            }
        }
        (ok, reason)
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
        num_of(raw).ok_or_else(|| BrokerError::Other(format!("could not convert string to float: '{}'", py_str(raw))))
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
            params.push(("quantity", plain_number(qty)));
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
            return Ok(Order::rejected(reason));
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

    /// 현물: 견적 통화 잔고와 보유 기초자산. 선물: 사용 가능 증거금.
    fn balance_note(&self) -> String {
        let Some((rows, available)) = self.quote_funds() else {
            return String::new();
        };
        let quote = self.quote_or_usdt();
        if self.futures() {
            return format!("사용 가능 증거금 {}", price_text(available, quote));
        }
        let mut text = format!("사용 가능 잔고 {}", price_text(available, quote));
        let base = match self.base_asset() {
            b if b.is_empty() => split_symbol(&self.symbol).0,
            b => b,
        };
        let held = asset_row(&rows, &base).and_then(spot_free).unwrap_or(0.0);
        if held > 0.0 {
            text.push_str(&format!(" · 보유 {base} {}", qty_text(held)));
        }
        text
    }
}

#[cfg(test)]
mod tests;
