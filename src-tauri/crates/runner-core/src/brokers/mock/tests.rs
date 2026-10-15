use super::*;
use crate::brokers::{BinanceBroker, BinanceUrls, DomesticBroker};
use crate::types::Exchange;
use serde_json::json;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

type Logs = Arc<Mutex<Vec<String>>>;

fn recorder() -> (LogFn, Logs) {
    let logs: Logs = Arc::new(Mutex::new(Vec::new()));
    let sink = logs.clone();
    (Arc::new(move |m: String| sink.lock().unwrap().push(m)), logs)
}

/// 시험용 안쪽 어댑터. 어떤 메서드가 불렸는지 이름을 남김.
struct Fake {
    price: f64,
    rules: Result<OrderRules, String>,
    bid_fee: Option<f64>,
    fee_from_base: bool,
    rehearse: (bool, String),
    load_market: Result<bool, String>,
    check_account: Result<(), String>,
    calls: Mutex<Vec<String>>,
}

impl Fake {
    fn new() -> Fake {
        Fake {
            price: 100.0,
            rules: Ok(OrderRules { step: 0.001, min_notional: 0.0 }),
            bid_fee: Some(0.0005),
            fee_from_base: true,
            rehearse: (true, String::new()),
            load_market: Ok(true),
            check_account: Ok(()),
            calls: Mutex::new(Vec::new()),
        }
    }
    fn called(&self, name: &str) -> usize {
        self.calls.lock().unwrap().iter().filter(|c| c.as_str() == name).count()
    }
    fn note(&self, name: &str) {
        self.calls.lock().unwrap().push(name.to_string());
    }
}

#[async_trait]
impl Broker for Fake {
    fn market(&self) -> Market {
        Market::Spot
    }
    fn side(&self) -> Side {
        Side::Long
    }
    fn symbol(&self) -> String {
        "BTCUSDT".into()
    }
    fn fee_from_base_asset(&self) -> bool {
        self.fee_from_base
    }
    fn bid_fee(&self) -> Option<f64> {
        self.bid_fee
    }
    fn last_error_name(&self) -> String {
        "inner_name".into()
    }
    fn set_log(&self, _log: LogFn) {}
    async fn check_account(&self) -> Result<(), BrokerError> {
        self.note("check_account");
        self.check_account.clone().map_err(BrokerError::Other)
    }
    async fn ensure_ready(&self) -> Result<bool, BrokerError> {
        self.note("ensure_ready");
        Ok(true)
    }
    async fn load_market(&self) -> Result<bool, BrokerError> {
        self.note("load_market");
        self.load_market.clone().map_err(BrokerError::Other)
    }
    async fn order_rules(&self) -> Result<OrderRules, BrokerError> {
        self.note("order_rules");
        self.rules.clone().map_err(BrokerError::Other)
    }
    async fn rehearse(&self, notional: f64, spend: Option<f64>) -> (bool, String) {
        self.note(&format!("rehearse({notional:?}, {spend:?})"));
        self.rehearse.clone()
    }
    async fn price(&self) -> Result<f64, BrokerError> {
        self.note("price");
        Ok(self.price)
    }
    async fn submit(&self, _req: SubmitRequest) -> Result<Order, BrokerError> {
        self.note("submit");
        Err(BrokerError::Other("inner submit must never be called".into()))
    }
}

fn wrap(fake: Fake) -> (MockBroker, Arc<Fake>, Logs) {
    let fake = Arc::new(fake);
    let (log, logs) = recorder();
    (MockBroker::new(fake.clone(), log), fake, logs)
}

fn req(side: OrderSide, base_qty: Option<f64>, notional: Option<f64>, id: &str) -> SubmitRequest {
    SubmitRequest { side, base_qty, notional, reduce_only: false, closing: None, client_id: id.into() }
}

#[tokio::test]
async fn mock_never_submits_and_fills_at_the_current_price() {
    let (m, fake, _) = wrap(Fake::new());
    let buy = m.submit(req(OrderSide::Buy, None, Some(1000.0), "ggp-3")).await.unwrap();
    m.submit(req(OrderSide::Sell, Some(1.0), None, "ggp-2")).await.unwrap();
    assert_eq!(fake.called("submit"), 0);
    assert_eq!((buy.status.as_str(), buy.avg_price, buy.executed_qty, buy.fees_known), ("FILLED", 100.0, 10.0, true));
    assert_eq!(buy.order_id.as_deref(), Some("mock-ggp-3"));
    assert_eq!(buy.raw_status, "MOCK_FILLED");
}

#[tokio::test]
async fn fee_rule_follows_the_inner_adapter() {
    let (m, _, _) = wrap(Fake::new());
    let o = m.submit(req(OrderSide::Buy, None, Some(1000.0), "ggp-4")).await.unwrap();
    assert!((o.acquired_qty - 10.0 * (1.0 - 0.0005)).abs() < 1e-12);
    let o = m.submit(req(OrderSide::Sell, Some(2.5), None, "ggp-5")).await.unwrap();
    assert_eq!((o.executed_qty, o.acquired_qty), (2.5, 2.5));
    let o = m.submit(req(OrderSide::Buy, Some(3.0), None, "ggp-6")).await.unwrap();
    assert!((o.acquired_qty - 3.0 * (1.0 - 0.0005)).abs() < 1e-12);
    let mut closing = req(OrderSide::Buy, Some(3.0), None, "ggp-8");
    closing.reduce_only = true;
    assert_eq!(m.submit(closing).await.unwrap().acquired_qty, 3.0);

    let mut fake = Fake::new();
    fake.fee_from_base = false;
    let (m, _, _) = wrap(fake);
    assert_eq!(m.submit(req(OrderSide::Buy, Some(3.0), None, "ggp-7")).await.unwrap().acquired_qty, 3.0);

    // 수수료율을 못 읽으면 거래소·시장 기본값 — 바이낸스 현물은 0.1%.
    for bid in [None, Some(0.0)] {
        let mut fake = Fake::new();
        fake.bid_fee = bid;
        let (m, _, _) = wrap(fake);
        let o = m.submit(req(OrderSide::Buy, None, Some(1000.0), "ggp-9")).await.unwrap();
        assert!((o.acquired_qty - 10.0 * (1.0 - 0.001)).abs() < 1e-12, "{}", o.acquired_qty);
        assert!((o.fee.unwrap().quote - 1.0).abs() < 1e-9, "1,000 USDT × 0.1%");
    }
    let mut fake = Fake::new();
    fake.bid_fee = Some(0.0025);
    let (m, _, _) = wrap(fake);
    let o = m.submit(req(OrderSide::Buy, None, Some(1000.0), "ggp-11")).await.unwrap();
    assert!((o.acquired_qty - 10.0 * (1.0 - 0.0025)).abs() < 1e-12);
    // 현물 모의 체결은 같은 수수료율로 낸 수수료(견적 통화)를 함께 돌려줌 — 화면의 수수료 반영 손익에 씀.
    let fee = o.fee.unwrap();
    assert!((fee.quote - 1000.0 * 0.0025).abs() < 1e-9 && fee.other.is_empty(), "{fee:?}");
}

#[tokio::test]
async fn orders_with_no_amount_are_refused() {
    let (m, _, _) = wrap(Fake::new());
    let e = m.submit(req(OrderSide::Buy, None, None, "ggp-12")).await.unwrap_err();
    assert_eq!(e.to_string(), "모의 주문에 쓸 금액이나 수량이 없어요");
    assert!(m.submit(req(OrderSide::Sell, None, Some(1000.0), "ggp-13")).await.is_err());
    let mut fake = Fake::new();
    fake.price = 0.0;
    let (m, _, _) = wrap(fake);
    assert_eq!(
        m.submit(req(OrderSide::Buy, None, Some(1000.0), "x")).await.unwrap_err().to_string(),
        "모의 체결에 쓸 현재가를 받지 못했어요"
    );
}

#[tokio::test]
async fn reads_the_market_without_the_inner_account_writes() {
    let (m, fake, _) = wrap(Fake::new());
    assert!(m.ensure_ready().await.unwrap());
    m.check_account().await.unwrap();
    assert_eq!(m.price().await.unwrap(), 100.0);
    assert_eq!(fake.called("load_market"), 1);
    assert_eq!(fake.called("check_account"), 1);
    assert_eq!(fake.called("price"), 1);
    assert_eq!(fake.called("ensure_ready"), 0);
    let mut fake = Fake::new();
    fake.load_market = Ok(false);
    assert!(!wrap(fake).0.ensure_ready().await.unwrap());
    assert_eq!(m.last_error_name(), "inner_name");
    assert_eq!(m.bid_fee(), Some(0.0005));
    assert_eq!(m.symbol(), "BTCUSDT");
}

#[tokio::test]
async fn failures_are_swallowed_and_noted_once() {
    let mut fake = Fake::new();
    fake.load_market = Err("HTTP 401 no_authorization_ip".into());
    fake.check_account = Err("HTTP 401 invalid_access_key".into());
    fake.rules = Err("401 키가 없어요".into());
    let (m, _, logs) = wrap(fake);
    assert!(m.ensure_ready().await.unwrap());
    assert!(m.ensure_ready().await.unwrap());
    m.check_account().await.unwrap();
    m.check_account().await.unwrap();
    for _ in 0..3 {
        assert_eq!(m.order_rules().await.unwrap(), OrderRules { step: 0.0, min_notional: 0.0 });
    }
    assert_eq!(
        *logs.lock().unwrap(),
        vec![
            "모의: 거래소 규격을 읽지 못했지만 계속해요(HTTP 401 no_authorization_ip)".to_string(),
            "모의: 계정 확인에 실패했지만 계속해요. 실전에서는 막힐 수 있어요(HTTP 401 invalid_access_key)".to_string(),
            "모의: 주문 규격을 읽지 못해 제한 없이 진행해요(401 키가 없어요)".to_string(),
        ]
    );
}

#[tokio::test]
async fn prefers_the_inner_order_rules_when_readable() {
    let (m, _, logs) = wrap(Fake::new());
    assert_eq!(m.order_rules().await.unwrap(), OrderRules { step: 0.001, min_notional: 0.0 });
    assert!(logs.lock().unwrap().is_empty());
}

#[tokio::test]
async fn a_failed_rehearsal_is_passed_through_with_its_reason() {
    let mut fake = Fake::new();
    fake.rehearse = (false, "허용 IP 가 등록되지 않았어요".into());
    let (m, fake, logs) = wrap(fake);
    let (ok, reason) = m.rehearse(1000.0, None).await;
    assert!(ok);
    assert_eq!(reason, "모의라서 그대로 진행해요. 실전이라면 막힐 이유: 허용 IP 가 등록되지 않았어요");
    assert_eq!(
        *logs.lock().unwrap(),
        vec!["모의 모드: 실전이라면 여기서 막힙니다 — 허용 IP 가 등록되지 않았어요".to_string()]
    );
    assert_eq!(fake.called("rehearse(1000.0, None)"), 1);
    // spend는 참일 때만 넘김.
    m.rehearse(1000.0, Some(0.0)).await;
    m.rehearse(1000.0, Some(20_000.0)).await;
    assert_eq!(fake.called("rehearse(1000.0, None)"), 2);
    assert_eq!(fake.called("rehearse(1000.0, Some(20000.0))"), 1);
}

#[tokio::test]
async fn a_successful_rehearsal_passes_through_quietly() {
    let mut fake = Fake::new();
    fake.rehearse = (true, "검증 통과".into());
    let (m, _, logs) = wrap(fake);
    assert_eq!(m.rehearse(1000.0, None).await, (true, "검증 통과".to_string()));
    assert!(logs.lock().unwrap().is_empty());
}

// --- 실제 어댑터를 감싼 경우 -------------------------------------------------

#[tokio::test]
async fn keyless_domestic_still_practises_and_never_posts_an_order() {
    for ex in [Exchange::Upbit, Exchange::Bithumb] {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/v1/ticker"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!([{"trade_price": 100.0}])))
            .with_priority(1)
            .mount(&server)
            .await;
        let denied =
            ResponseTemplate::new(401).set_body_json(json!({"error": {"name": "no_authorization_ip", "message": "x"}}));
        Mock::given(method("GET")).respond_with(denied.clone()).with_priority(5).mount(&server).await;
        Mock::given(method("POST")).respond_with(denied).with_priority(5).mount(&server).await;
        let (log, _) = recorder();
        let inner =
            DomesticBroker::new("acc", "sec", ex, "KRW-BTC", log.clone(), reqwest::Client::new(), Some(server.uri()))
                .unwrap();
        assert!(inner.check_account().await.is_err());
        assert!(inner.ensure_ready().await.is_err());
        let m = MockBroker::new(Arc::new(inner), log);
        m.check_account().await.unwrap();
        assert!(m.ensure_ready().await.unwrap());
        let (ok, reason) = m.rehearse(1000.0, None).await;
        assert!(ok && !reason.is_empty());
        assert_eq!(m.order_rules().await.unwrap(), OrderRules { step: 0.0, min_notional: 0.0 });
        assert_eq!(m.price().await.unwrap(), 100.0);
        let o = m.submit(req(OrderSide::Buy, None, Some(1000.0), "ggp-k1")).await.unwrap();
        assert_eq!((o.status.as_str(), o.executed_qty, o.acquired_qty), ("FILLED", 10.0, 10.0));
        let posted: Vec<String> = server
            .received_requests()
            .await
            .unwrap()
            .into_iter()
            .filter(|r| r.method.as_str() == "POST")
            .map(|r| r.url.path().to_string())
            .collect();
        assert!(!posted.iter().any(|p| p == "/v1/orders" || p == "/v2/orders"), "{posted:?}");
        if ex == Exchange::Bithumb {
            assert!(posted.is_empty(), "빗썸은 리허설도 읽기뿐이다");
        }
    }
}

#[tokio::test]
async fn wrapped_binance_futures_never_changes_margin_or_leverage_or_orders() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/fapi/v1/exchangeInfo"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"symbols": [{"symbol": "BTCUSDT", "filters": [
            {"filterType": "LOT_SIZE", "stepSize": "0.001"}, {"filterType": "MIN_NOTIONAL", "notional": "5"}]}]})))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/fapi/v1/ticker/price"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"price": "100"})))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/fapi/v2/balance"))
        .respond_with(ResponseTemplate::new(401).set_body_json(json!({"code": -2015, "msg": "Invalid API-key"})))
        .mount(&server)
        .await;
    let (log, _) = recorder();
    let urls = BinanceUrls { spot: server.uri(), futures: server.uri() };
    let inner = BinanceBroker::new(
        "k",
        "s",
        Market::Futures,
        "BTCUSDT",
        Side::Long,
        false,
        5,
        log.clone(),
        reqwest::Client::new(),
        Some(urls),
    );
    let m = MockBroker::new(Arc::new(inner), log);
    m.check_account().await.unwrap();
    assert!(m.ensure_ready().await.unwrap());
    assert_eq!(m.order_rules().await.unwrap().step, 0.001);
    let (ok, reason) = m.rehearse(1000.0, None).await;
    assert!(ok && reason.contains("APIError(code=-2015)"), "{reason}");
    assert_eq!(m.last_error_name(), "binance_access");
    let o = m.submit(req(OrderSide::Buy, Some(3.0), None, "ggp-f2")).await.unwrap();
    assert_eq!((o.status.as_str(), o.acquired_qty), ("FILLED", 3.0));
    // 모의 선물도 시장가(테이커) 기본 수수료 0.05%를 알려 줌 — 3개 × 100 USDT × 0.05% = 0.15 USDT.
    let fee = o.fee.unwrap();
    assert!((fee.quote - 0.15).abs() < 1e-9 && fee.other.is_empty(), "{fee:?}");
    let posted = server.received_requests().await.unwrap().into_iter().filter(|r| r.method.as_str() == "POST").count();
    assert_eq!(posted, 0, "모의는 레버리지 · 마진 · 주문 어느 것도 쓰지 않는다");
}
