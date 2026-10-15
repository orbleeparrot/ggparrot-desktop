use super::*;
use crate::brokers::OrderSide;
use serde_json::json;
use std::sync::Arc;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, Request, ResponseTemplate};

type Logs = Arc<Mutex<Vec<String>>>;

fn make(server: &MockServer, market: Market, leverage: u32) -> (BinanceBroker, Logs) {
    make_in(server, market, leverage, true)
}

/// 실거래 키 — 키 권한 조회까지 함.
fn make_live(server: &MockServer, market: Market) -> BinanceBroker {
    make_in(server, market, 1, false).0
}

fn make_in(server: &MockServer, market: Market, leverage: u32, testnet: bool) -> (BinanceBroker, Logs) {
    let logs: Logs = Arc::new(Mutex::new(Vec::new()));
    let sink = logs.clone();
    let log: LogFn = Arc::new(move |m: String| sink.lock().unwrap().push(m));
    let urls = BinanceUrls { spot: server.uri(), futures: server.uri() };
    let b = BinanceBroker::new(
        "key",
        "secret",
        market,
        "BTCUSDT",
        Side::Long,
        testnet,
        leverage,
        log,
        reqwest::Client::new(),
        Some(urls),
    );
    (b, logs)
}

async fn mount(server: &MockServer, verb: &str, p: &str, resp: ResponseTemplate) {
    Mock::given(method(verb)).and(path(p)).respond_with(resp).mount(server).await;
}

fn ok(body: Value) -> ResponseTemplate {
    ResponseTemplate::new(200).set_body_json(body)
}

fn api_err(code: i64, msg: &str) -> ResponseTemplate {
    ResponseTemplate::new(400).set_body_json(json!({"code": code, "msg": msg}))
}

async fn requests(server: &MockServer, verb: &str, p: &str) -> Vec<Request> {
    server
        .received_requests()
        .await
        .unwrap_or_default()
        .into_iter()
        .filter(|r| r.method.as_str() == verb && r.url.path() == p)
        .collect()
}

fn param(req: &Request, key: &str) -> Option<String> {
    req.url.query_pairs().find(|(k, _)| k == key).map(|(_, v)| v.to_string())
}

fn spot_info() -> Value {
    json!({"symbols": [{"symbol": "BTCUSDT", "baseAsset": "BTC", "filters": [
        {"filterType": "LOT_SIZE", "stepSize": "0.001"}, {"filterType": "NOTIONAL", "minNotional": "5"}]}]})
}

fn futures_info() -> Value {
    json!({"symbols": [{"symbol": "BTCUSDT", "filters": [
        {"filterType": "LOT_SIZE", "stepSize": "0.001"}, {"filterType": "MIN_NOTIONAL", "notional": "5"}]}]})
}

fn order(side: OrderSide, qty: f64, id: &str) -> SubmitRequest {
    SubmitRequest { side, base_qty: Some(qty), notional: None, reduce_only: false, closing: None, client_id: id.into() }
}

// --- 순수 함수 -----------------------------------------------------------

#[test]
fn filters_fold_and_g_format() {
    assert_eq!(parse_filters(&spot_info()["symbols"][0]), (0.001, 5.0));
    assert_eq!(parse_filters(&futures_info()["symbols"][0]), (0.001, 5.0));
    let info = json!({"filters": [{"filterType": "MARKET_LOT_SIZE", "stepSize": "0.01"}, {"filterType": "LOT_SIZE", "stepSize": "0.001"},
                                  {"filterType": "MIN_NOTIONAL", "minNotional": "", "notional": "7"}]});
    assert_eq!(parse_filters(&info), (0.01, 0.0));
    assert_eq!(parse_filters(&Value::Null), (0.0, 0.0));
    assert_eq!(fold_status(Some(&json!("FILLED"))), "FILLED");
    assert_eq!(fold_status(Some(&json!("EXPIRED"))), "CANCELED");
    assert_eq!(fold_status(Some(&json!("EXPIRED_IN_MATCH"))), "CANCELED");
    assert_eq!(fold_status(Some(&json!("REJECTED"))), "REJECTED");
    assert_eq!(fold_status(Some(&json!("NEW"))), "OPEN");
    assert_eq!(fold_status(Some(&json!("PARTIALLY_FILLED"))), "OPEN");
    assert_eq!(fold_status(Some(&json!(""))), "UNKNOWN");
    assert_eq!(fold_status(None), "UNKNOWN");
    assert_eq!(py_g(5.0), "5");
    assert_eq!(py_g(1.0), "1");
    assert_eq!(py_g(10.5), "10.5");
    assert_eq!(py_g(0.1), "0.1");
    assert_eq!(py_g(1234567.0), "1.23457e+06");
    assert_eq!(py_g(0.00001), "1e-05");
    assert_eq!(py_g(100000.0), "100000");
    assert_eq!(py_g(0.0001), "0.0001");
    assert_eq!(fmt_qty(0.001), "0.001");
    assert_eq!(fmt_qty(2.0), "2");
    assert_eq!(fmt_qty(0.00001), "0.00001");
}

#[tokio::test]
async fn normalize_average_price_fallbacks() {
    let server = MockServer::start().await;
    let (b, _) = make(&server, Market::Futures, 1);
    let o =
        b.normalize(&json!({"status": "FILLED", "executedQty": "2", "avgPrice": "0", "cumQuote": "210"}), false).await;
    assert_eq!(o.avg_price, 105.0);
    let o = b
        .normalize(
            &json!({"status": "FILLED", "executedQty": "2", "cummulativeQuoteQty": "220", "cumQuote": "210"}),
            false,
        )
        .await;
    assert_eq!(o.avg_price, 110.0);
    // "0.00000000"은 빈 문자열이 아니라 참(`py_truthy`)이므로 cumQuote로 넘어가지 않음 — fills 합으로 내려감.
    let o = b
        .normalize(
            &json!({"status": "FILLED", "executedQty": "2", "cummulativeQuoteQty": "0.00000000", "cumQuote": "210",
            "fills": [{"price": "100", "qty": "1"}, {"price": "120", "qty": "1"}]}),
            false,
        )
        .await;
    assert_eq!(o.avg_price, 110.0);
    let o = b.normalize(&json!({}), false).await;
    assert_eq!((o.status.as_str(), o.raw_status.as_str(), o.fees_known), ("UNKNOWN", "unknown", true));
    let o = b.normalize(&json!({"status": "EXPIRED", "executedQty": "1", "avgPrice": "5", "orderId": 77}), false).await;
    assert_eq!((o.status.as_str(), o.raw_status.as_str()), ("CANCELED", "EXPIRED"));
    assert_eq!(o.order_id.as_deref(), Some("77"));
    assert_eq!(o.acquired_qty, 1.0, "선물은 수수료를 수량에서 빼지 않는다");
}

#[tokio::test]
async fn spot_base_fee_is_deducted_from_acquired() {
    let server = MockServer::start().await;
    mount(
        &server,
        "POST",
        "/api/v3/order",
        ok(json!({"orderId": 7, "status": "FILLED", "executedQty": "2", "avgPrice": "100",
        "fills": [{"price": "100", "qty": "2", "commission": "0.002", "commissionAsset": "BTC"}]})),
    )
    .await;
    let (b, _) = make(&server, Market::Spot, 1);
    let o = b.submit(order(OrderSide::Buy, 2.0, "ggp-1")).await.unwrap();
    assert_eq!(o.executed_qty, 2.0);
    assert!((o.acquired_qty - 1.998).abs() < 1e-12);
    assert!(o.fees_known);
    assert_eq!(o.order_id.as_deref(), Some("7"));
    let post = &requests(&server, "POST", "/api/v3/order").await[0];
    assert_eq!(param(post, "type").as_deref(), Some("MARKET"));
    assert_eq!(param(post, "quantity").as_deref(), Some("2"));
    assert_eq!(param(post, "newClientOrderId").as_deref(), Some("ggp-1"));
    assert_eq!(param(post, "newOrderRespType").as_deref(), Some("FULL"));
    assert_eq!(param(post, "reduceOnly"), None);
    assert!(requests(&server, "GET", "/api/v3/order").await.is_empty(), "끝난 주문은 다시 조회하지 않는다");
}

#[tokio::test]
async fn closing_spot_sell_keeps_the_quantity_and_fee_in_other_assets_is_ignored() {
    let server = MockServer::start().await;
    mount(
        &server,
        "POST",
        "/api/v3/order",
        ok(json!({"orderId": 9, "status": "FILLED", "executedQty": "2", "avgPrice": "100",
        "fills": [{"price": "100", "qty": "2", "commission": "0.2", "commissionAsset": "USDT"}]})),
    )
    .await;
    let (b, _) = make(&server, Market::Spot, 1);
    let o = b.submit(order(OrderSide::Sell, 2.0, "ggp-s")).await.unwrap();
    assert_eq!(o.acquired_qty, 2.0);
    let o = b.submit(order(OrderSide::Buy, 2.0, "ggp-b")).await.unwrap();
    assert_eq!(o.acquired_qty, 2.0);
    assert!(o.fees_known);
}

#[tokio::test]
async fn missing_fee_confirmation_is_reported_not_guessed() {
    let server = MockServer::start().await;
    mount(
        &server,
        "POST",
        "/api/v3/order",
        ok(json!({"orderId": 8, "status": "FILLED", "executedQty": "2", "avgPrice": "100", "fills": []})),
    )
    .await;
    mount(&server, "GET", "/api/v3/myTrades", ok(json!([]))).await;
    let (b, _) = make(&server, Market::Spot, 1);
    let o = b.submit(order(OrderSide::Buy, 2.0, "ggp-3")).await.unwrap();
    assert!(!o.fees_known);
    assert_eq!(o.acquired_qty, 2.0);
    let trades = &requests(&server, "GET", "/api/v3/myTrades").await[0];
    assert_eq!(param(trades, "orderId").as_deref(), Some("8"));
    assert_eq!(param(trades, "limit").as_deref(), Some("1000"));
    assert!(param(trades, "signature").is_some());
}

#[tokio::test]
async fn trades_fill_in_missing_fills_and_must_cover_the_execution() {
    let server = MockServer::start().await;
    mount(
        &server,
        "POST",
        "/api/v3/order",
        ok(json!({"orderId": 8, "status": "FILLED", "executedQty": "2", "cummulativeQuoteQty": "200"})),
    )
    .await;
    mount(
        &server,
        "GET",
        "/api/v3/myTrades",
        ok(json!([
        {"orderId": 8, "qty": "1.5", "commission": "0.0015", "commissionAsset": "BTC"},
        {"orderId": 8, "qty": "0.5", "commission": "0.0005", "commissionAsset": "BTC"},
        {"orderId": 9, "qty": "5", "commission": "1", "commissionAsset": "BTC"}])),
    )
    .await;
    let (b, _) = make(&server, Market::Spot, 1);
    let o = b.submit(order(OrderSide::Buy, 2.0, "ggp-t")).await.unwrap();
    assert!(o.fees_known);
    assert!((o.acquired_qty - 1.998).abs() < 1e-12);
    assert_eq!(o.avg_price, 100.0);

    // 체결 내역이 체결 수량을 다 덮지 못하면 수수료를 확정하지 않음.
    let server = MockServer::start().await;
    mount(
        &server,
        "POST",
        "/api/v3/order",
        ok(json!({"orderId": 8, "status": "FILLED", "executedQty": "2", "avgPrice": "100",
        "fills": [{"price": "100", "qty": "1", "commission": "0.001", "commissionAsset": "BTC"}]})),
    )
    .await;
    let (b, _) = make(&server, Market::Spot, 1);
    let o = b.submit(order(OrderSide::Buy, 2.0, "ggp-u")).await.unwrap();
    assert!(!o.fees_known);
    assert_eq!(o.acquired_qty, 2.0);
    // commission 키가 없는 fill도 불확실임.
    let server = MockServer::start().await;
    mount(
        &server,
        "POST",
        "/api/v3/order",
        ok(json!({"orderId": 8, "status": "FILLED", "executedQty": "2", "avgPrice": "100",
        "fills": [{"price": "100", "qty": "2", "commissionAsset": "BTC"}]})),
    )
    .await;
    let (b, _) = make(&server, Market::Spot, 1);
    assert!(!b.submit(order(OrderSide::Buy, 2.0, "ggp-v")).await.unwrap().fees_known);
}

#[tokio::test]
async fn a_definite_rejection_keeps_the_exchange_reason_and_is_not_uncertain() {
    let server = MockServer::start().await;
    mount(&server, "POST", "/fapi/v1/order", api_err(-2019, "Margin is insufficient.")).await;
    mount(&server, "GET", "/fapi/v1/order", api_err(-2013, "Order does not exist.")).await;
    let (b, logs) = make(&server, Market::Futures, 1);
    let o = b.submit(order(OrderSide::Buy, 2.0, "ggp-r")).await.unwrap();
    assert_eq!(o.status, "REJECTED");
    assert!(o.fees_known, "거절은 불확실하지 않다");
    assert_eq!(o.raw_status, "-2019 Margin is insufficient.");
    assert_eq!(requests(&server, "POST", "/fapi/v1/order").await.len(), 1, "다시 주문하지 않는다");
    assert_eq!(requests(&server, "GET", "/fapi/v1/order").await.len(), 1, "한 번만 확인한다");
    assert!(logs.lock().unwrap().iter().any(|l| l.contains("거절했어요(-2019")));
}

#[tokio::test]
async fn an_ambiguous_error_code_is_looked_up_not_treated_as_rejection() {
    let server = MockServer::start().await;
    mount(&server, "POST", "/fapi/v1/order", api_err(-1007, "Timeout waiting for response.")).await;
    mount(&server, "GET", "/fapi/v1/order", ok(json!({"status": "FILLED", "executedQty": "2", "avgPrice": "100"})))
        .await;
    let (b, _) = make(&server, Market::Futures, 1);
    let o = b.submit(order(OrderSide::Buy, 2.0, "ggp-a")).await.unwrap();
    assert_eq!(o.status, "FILLED");
}

#[tokio::test]
async fn clock_skew_resyncs_and_retries_the_signed_call_once() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/fapi/v1/order"))
        .respond_with(api_err(-1021, "Timestamp for this request is outside of the recvWindow."))
        .up_to_n_times(1)
        .with_priority(1)
        .mount(&server)
        .await;
    mount(&server, "GET", "/fapi/v1/order", ok(json!({"status": "FILLED", "executedQty": "1", "avgPrice": "1"}))).await;
    let far_future = now_ms() as i64 + 60_000;
    mount(&server, "GET", "/fapi/v1/time", ok(json!({"serverTime": far_future}))).await;
    let (b, logs) = make(&server, Market::Futures, 1);
    let lookup = [("symbol", "BTCUSDT".to_string()), ("origClientOrderId", "x".to_string())];
    assert!(b.get(true, "/fapi/v1/order", &lookup, true).await.is_ok());
    let gets = requests(&server, "GET", "/fapi/v1/order").await;
    assert_eq!(gets.len(), 2);
    let ts: i64 = param(&gets[1], "timestamp").unwrap().parse().unwrap();
    assert!(ts > now_ms() as i64 + 50_000, "두 번째 요청은 거래소 시계를 따른다");
    assert!(logs.lock().unwrap().iter().any(|l| l.contains("시계")));
}

#[tokio::test]
async fn rate_limit_pauses_requests_until_retry_after() {
    let server = MockServer::start().await;
    mount(
        &server,
        "GET",
        "/fapi/v1/ticker/price",
        ResponseTemplate::new(429)
            .insert_header("Retry-After", "60")
            .set_body_json(json!({"code": -1003, "msg": "Too many requests"})),
    )
    .await;
    let (b, logs) = make(&server, Market::Futures, 1);
    assert!(b.price().await.is_err());
    assert!(b.price().await.is_err());
    assert_eq!(requests(&server, "GET", "/fapi/v1/ticker/price").await.len(), 1, "쉬는 동안은 보내지 않는다");
    assert!(logs.lock().unwrap().iter().any(|l| l.contains("60초")));
    // 쉬는 동안 낸 주문은 보내지 않았으니 분명한 거절임(불확실하지 않음).
    let o = b.submit(order(OrderSide::Buy, 1.0, "ggp-rl")).await.unwrap();
    assert_eq!(o.status, "REJECTED");
    assert!(requests(&server, "POST", "/fapi/v1/order").await.is_empty());
}

#[tokio::test]
async fn timeout_is_reconciled_by_client_id_without_a_second_order() {
    let server = MockServer::start().await;
    mount(&server, "POST", "/fapi/v1/order", ResponseTemplate::new(504).set_body_string("gateway timeout")).await;
    mount(&server, "GET", "/fapi/v1/order", ok(json!({"status": "FILLED", "executedQty": "2", "avgPrice": "105"})))
        .await;
    let (b, _) = make(&server, Market::Futures, 1);
    let mut req = order(OrderSide::Sell, 2.0, "ggp-2");
    req.reduce_only = true;
    let o = b.submit(req).await.unwrap();
    assert_eq!(o.status, "FILLED");
    assert_eq!(o.avg_price, 105.0);
    let posts = requests(&server, "POST", "/fapi/v1/order").await;
    assert_eq!(posts.len(), 1);
    assert_eq!(param(&posts[0], "reduceOnly").as_deref(), Some("true"));
    assert_eq!(param(&posts[0], "newOrderRespType").as_deref(), Some("RESULT"));
    let gets = requests(&server, "GET", "/fapi/v1/order").await;
    assert_eq!(gets.len(), 1);
    assert_eq!(param(&gets[0], "origClientOrderId").as_deref(), Some("ggp-2"));
    assert_eq!(param(&gets[0], "symbol").as_deref(), Some("BTCUSDT"));
}

#[tokio::test]
async fn an_open_order_is_looked_up_max_retries_times_and_keeps_the_last_answer() {
    let server = MockServer::start().await;
    mount(&server, "POST", "/fapi/v1/order", ok(json!({"status": "NEW", "executedQty": "0"}))).await;
    Mock::given(method("GET"))
        .and(path("/fapi/v1/order"))
        .respond_with(ok(json!({"status": "PARTIALLY_FILLED", "executedQty": "1", "avgPrice": "100"})))
        .up_to_n_times(1)
        .with_priority(1)
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/fapi/v1/order"))
        .respond_with(ResponseTemplate::new(500))
        .with_priority(2)
        .mount(&server)
        .await;
    let (b, _) = make(&server, Market::Futures, 1);
    let o = b.submit(order(OrderSide::Buy, 2.0, "ggp-o")).await.unwrap();
    assert_eq!((o.status.as_str(), o.raw_status.as_str()), ("OPEN", "PARTIALLY_FILLED"));
    assert_eq!(o.executed_qty, 1.0);
    assert_eq!(requests(&server, "GET", "/fapi/v1/order").await.len(), crate::brokers::LOOKUP_ATTEMPTS);
    assert_eq!(requests(&server, "POST", "/fapi/v1/order").await.len(), 1);
}

#[tokio::test]
async fn signed_requests_carry_the_key_header_and_a_valid_signature() {
    let server = MockServer::start().await;
    mount(&server, "GET", "/api/v3/account", ok(json!({"balances": [{"asset": "USDT", "free": "100"}]}))).await;
    let (b, logs) = make(&server, Market::Spot, 1);
    b.check_account().await.unwrap();
    assert_eq!(logs.lock().unwrap().last().unwrap(), "연결 성공 · 현물 USDT 잔고: 100");
    let req = &requests(&server, "GET", "/api/v3/account").await[0];
    assert_eq!(req.headers.get("x-mbx-apikey").unwrap().to_str().unwrap(), "key");
    let query = req.url.query().unwrap();
    let (payload, signature) = query.rsplit_once("&signature=").unwrap();
    assert!(payload.contains("timestamp=") && payload.contains("recvWindow=5000"));
    let mut mac = Hmac::<Sha256>::new_from_slice(b"secret").unwrap();
    mac.update(payload.as_bytes());
    assert_eq!(signature, hex::encode(mac.finalize().into_bytes()));
}

#[tokio::test]
async fn futures_check_account_reports_margin_or_lookup_failure() {
    let server = MockServer::start().await;
    mount(
        &server,
        "GET",
        "/fapi/v2/balance",
        ok(json!([{"asset": "BNB", "balance": "1"}, {"asset": "USDT", "balance": "100"}])),
    )
    .await;
    let (b, logs) = make(&server, Market::Futures, 1);
    b.check_account().await.unwrap();
    assert_eq!(logs.lock().unwrap().last().unwrap(), "연결 성공 · 선물 USDT 증거금: 100.0");
    let server = MockServer::start().await;
    mount(&server, "GET", "/fapi/v2/balance", api_err(-2015, "Invalid API-key, IP, or permissions for action")).await;
    let (b, logs) = make(&server, Market::Futures, 1);
    b.check_account().await.unwrap();
    assert_eq!(logs.lock().unwrap().last().unwrap(), "연결 성공 · 선물 USDT 증거금: 조회 실패");
}

// --- 선물 준비 --------------------------------------------------------------

#[tokio::test]
async fn futures_prepare_sets_isolated_margin_and_the_macro_leverage() {
    let server = MockServer::start().await;
    mount(&server, "GET", "/fapi/v1/exchangeInfo", ok(futures_info())).await;
    mount(&server, "POST", "/fapi/v1/marginType", ok(json!({"code": 200, "msg": "success"}))).await;
    mount(&server, "POST", "/fapi/v1/leverage", ok(json!({"leverage": 7}))).await;
    let (b, _) = make(&server, Market::Futures, 7);
    assert!(b.ensure_ready().await.unwrap());
    assert_eq!(b.order_rules().await.unwrap(), OrderRules { step: 0.001, min_notional: 5.0 });
    let margin = requests(&server, "POST", "/fapi/v1/marginType").await;
    assert_eq!(margin.len(), 1);
    assert_eq!(param(&margin[0], "marginType").as_deref(), Some("ISOLATED"));
    assert_eq!(param(&margin[0], "symbol").as_deref(), Some("BTCUSDT"));
    let lev = requests(&server, "POST", "/fapi/v1/leverage").await;
    assert_eq!(lev.len(), 1);
    assert_eq!(param(&lev[0], "leverage").as_deref(), Some("7"));
}

#[tokio::test]
async fn leverage_is_still_set_when_the_margin_type_is_refused() {
    let server = MockServer::start().await;
    mount(&server, "GET", "/fapi/v1/exchangeInfo", ok(futures_info())).await;
    mount(&server, "POST", "/fapi/v1/marginType", api_err(-4046, "No need to change margin type.")).await;
    mount(&server, "POST", "/fapi/v1/leverage", ok(json!({"leverage": 3}))).await;
    let (b, logs) = make(&server, Market::Futures, 3);
    assert!(b.ensure_ready().await.unwrap());
    assert_eq!(requests(&server, "POST", "/fapi/v1/leverage").await.len(), 1);
    assert!(logs.lock().unwrap().is_empty());
}

#[tokio::test]
async fn a_failed_leverage_change_is_reported() {
    let server = MockServer::start().await;
    mount(&server, "GET", "/fapi/v1/exchangeInfo", ok(futures_info())).await;
    mount(&server, "POST", "/fapi/v1/marginType", ok(json!({}))).await;
    mount(&server, "POST", "/fapi/v1/leverage", api_err(-4028, "Leverage 5 is not valid")).await;
    let (b, logs) = make(&server, Market::Futures, 5);
    match b.ensure_ready().await {
        Err(BrokerError::Access { explanation, name }) => {
            assert_eq!(name, "leverage");
            assert!(explanation.contains("5배로 바꾸지 못했어요"), "{explanation}");
        }
        other => panic!("기본 레버리지로 진행하면 안 된다: {other:?}"),
    }
    assert_eq!(
        logs.lock().unwrap().last().unwrap(),
        "⚠ 레버리지 5배 설정 실패(APIError(code=-4028): Leverage 5 is not valid)."
    );
}

#[tokio::test]
async fn margin_type_already_isolated_is_fine_but_other_failures_stop() {
    let server = MockServer::start().await;
    mount(&server, "GET", "/fapi/v1/exchangeInfo", ok(futures_info())).await;
    mount(&server, "POST", "/fapi/v1/marginType", api_err(-4046, "No need to change margin type.")).await;
    mount(&server, "POST", "/fapi/v1/leverage", ok(json!({}))).await;
    let (b, _) = make(&server, Market::Futures, 5);
    assert!(b.ensure_ready().await.unwrap());

    let server = MockServer::start().await;
    mount(&server, "GET", "/fapi/v1/exchangeInfo", ok(futures_info())).await;
    mount(
        &server,
        "POST",
        "/fapi/v1/marginType",
        api_err(-4048, "Margin type cannot be changed if there exists position."),
    )
    .await;
    let (b, _) = make(&server, Market::Futures, 5);
    assert!(matches!(b.ensure_ready().await, Err(BrokerError::Access { .. })));
    assert!(requests(&server, "POST", "/fapi/v1/leverage").await.is_empty());
}

#[tokio::test]
async fn futures_symbol_lookup_failure_is_an_error_not_a_missing_symbol() {
    let server = MockServer::start().await;
    mount(&server, "GET", "/fapi/v1/exchangeInfo", ResponseTemplate::new(500).set_body_string("down")).await;
    let (b, logs) = make(&server, Market::Futures, 5);
    assert!(matches!(b.ensure_ready().await, Err(BrokerError::Access { .. })));
    assert_eq!(
        logs.lock().unwrap().last().unwrap(),
        "선물 심볼정보 조회 실패: APIError(code=0): Invalid JSON error message from Binance: down"
    );
    assert!(requests(&server, "POST", "/fapi/v1/leverage").await.is_empty());
}

#[tokio::test]
async fn spot_never_touches_margin_or_leverage_and_reads_the_base_asset() {
    let server = MockServer::start().await;
    mount(&server, "GET", "/api/v3/exchangeInfo", ok(spot_info())).await;
    let (b, _) = make(&server, Market::Spot, 5);
    assert!(b.ensure_ready().await.unwrap());
    assert_eq!(b.base_asset(), "BTC");
    assert_eq!(param(&requests(&server, "GET", "/api/v3/exchangeInfo").await[0], "symbol").as_deref(), Some("BTCUSDT"));
    assert!(requests(&server, "POST", "/fapi/v1/marginType").await.is_empty());
    assert!(requests(&server, "POST", "/fapi/v1/leverage").await.is_empty());
}

#[tokio::test]
async fn price_reads_the_ticker_of_the_market() {
    let server = MockServer::start().await;
    mount(&server, "GET", "/api/v3/ticker/price", ok(json!({"symbol": "BTCUSDT", "price": "100.5"}))).await;
    mount(&server, "GET", "/fapi/v1/ticker/price", ok(json!({"symbol": "BTCUSDT", "price": "101"}))).await;
    assert_eq!(make(&server, Market::Spot, 1).0.price().await.unwrap(), 100.5);
    assert_eq!(make(&server, Market::Futures, 1).0.price().await.unwrap(), 101.0);
}

// --- 리허설 -----------------------------------------------------------------

async fn spot_server(min_notional: &str) -> MockServer {
    let server = MockServer::start().await;
    mount(&server, "GET", "/api/v3/account", ok(json!({"balances": [{"asset": "USDT", "free": "100"}]}))).await;
    let mut info = spot_info();
    info["symbols"][0]["filters"][1]["minNotional"] = json!(min_notional);
    mount(&server, "GET", "/api/v3/exchangeInfo", ok(info)).await;
    server
}

#[tokio::test]
async fn spot_rehearsal_passes_on_account_and_rules_without_any_order() {
    let server = spot_server("5").await;
    let (b, _) = make(&server, Market::Spot, 1);
    let (ok_, reason) = b.rehearse(10.0, None).await;
    assert!(ok_, "{reason}");
    assert_eq!(reason, "바이낸스 계정과 주문 규격을 확인했어요. 주문 권한은 첫 주문 때 알 수 있어요.");
    let all = server.received_requests().await.unwrap();
    assert!(all.iter().all(|r| r.method.as_str() == "GET"));
    // 연습 환경에는 키 권한 조회가 없음.
    assert!(requests(&server, "GET", API_RESTRICTIONS_PATH).await.is_empty());
    assert_eq!(b.last_error_name(), "binance_test_ok");
}

#[tokio::test]
async fn access_code_points_at_ip_on_live_and_at_the_key_on_practice() {
    let msg = "Invalid API-key, IP, or permissions for action";
    let server = MockServer::start().await;
    mount(&server, "GET", "/api/v3/account", api_err(-2015, msg)).await;
    let b = make_live(&server, Market::Spot);
    let (ok_, reason) = b.rehearse(10.0, None).await;
    assert!(!ok_);
    assert_eq!(
        reason,
        format!("바이낸스에서 키를 거절했어요(APIError(code=-2015): {msg}). 키 · 허용 IP · 권한 설정을 확인하세요.")
    );
    assert_eq!(b.last_error_name(), "binance_access");

    let (practice, _) = make(&server, Market::Spot, 1);
    let (ok_, reason) = practice.rehearse(10.0, None).await;
    assert!(!ok_ && reason.contains("테스트넷 · 데모"), "{reason}");
    assert_eq!(practice.last_error_name(), "binance_access_test");
}

#[tokio::test]
async fn format_signature_and_unknown_key_codes_are_key_errors() {
    for code in [-2014, -1022, -2008] {
        for testnet in [true, false] {
            let server = MockServer::start().await;
            mount(&server, "GET", "/api/v3/account", api_err(code, "bad key")).await;
            let (b, _) = make_in(&server, Market::Spot, 1, testnet);
            let (ok_, reason) = b.rehearse(10.0, None).await;
            assert!(!ok_);
            assert_eq!(
                reason,
                format!("바이낸스에서 키를 받지 않았어요(APIError(code={code}): bad key). API Key · Secret Key를 다시 복사해 넣으세요.")
            );
            assert_eq!(b.last_error_name(), "binance_key", "{code}");
        }
    }
}

async fn restrictions(server: &MockServer, body: Value) {
    mount(server, "GET", API_RESTRICTIONS_PATH, ok(body)).await;
}

#[tokio::test]
async fn live_spot_reads_the_key_permissions_after_the_account() {
    let cases = [
        (json!({"ipRestrict": true, "enableSpotAndMarginTrading": true}), true, "binance_verified"),
        (json!({"ipRestrict": false, "enableSpotAndMarginTrading": true}), true, "binance_verified_open"),
        (json!({"ipRestrict": false, "enableSpotAndMarginTrading": false}), false, "binance_ip_unrestricted"),
        (
            json!({"ipRestrict": true, "enableSpotAndMarginTrading": false, "enableFutures": true}),
            false,
            "binance_no_trade",
        ),
    ];
    for (body, passed, name) in cases {
        let server = spot_server("5").await;
        restrictions(&server, body).await;
        let b = make_live(&server, Market::Spot);
        let (ok_, reason) = b.rehearse(10.0, None).await;
        assert_eq!(ok_, passed, "{name}: {reason}");
        assert_eq!(b.last_error_name(), name);
        let lookups = requests(&server, "GET", API_RESTRICTIONS_PATH).await;
        assert_eq!(lookups.len(), 1);
        assert!(param(&lookups[0], "signature").is_some(), "서명한 조회");
    }
}

#[tokio::test]
async fn live_futures_checks_the_futures_switch() {
    let server = futures_server().await;
    restrictions(&server, json!({"ipRestrict": true, "enableSpotAndMarginTrading": true, "enableFutures": false}))
        .await;
    let b = make_live(&server, Market::Futures);
    let (ok_, reason) = b.rehearse(10.0, None).await;
    assert!(!ok_);
    assert!(reason.contains("Enable Futures") && reason.contains("새로 만드세요"), "{reason}");
    assert_eq!(b.last_error_name(), "binance_no_futures");

    let server = futures_server().await;
    restrictions(&server, json!({"ipRestrict": true, "enableSpotAndMarginTrading": false, "enableFutures": true}))
        .await;
    let b = make_live(&server, Market::Futures);
    assert!(b.rehearse(10.0, None).await.0);
    assert_eq!(b.last_error_name(), "binance_verified");
}

#[tokio::test]
async fn a_failed_permission_lookup_does_not_block() {
    let server = spot_server("5").await;
    mount(&server, "GET", API_RESTRICTIONS_PATH, api_err(-1000, "boom")).await;
    let b = make_live(&server, Market::Spot);
    let (ok_, reason) = b.rehearse(10.0, None).await;
    assert!(ok_);
    assert!(reason.contains("첫 주문"), "{reason}");
    assert_eq!(b.last_error_name(), "");
}

#[tokio::test]
async fn other_failures_and_unknown_symbols_fail() {
    let server = MockServer::start().await;
    mount(&server, "GET", "/api/v3/account", api_err(-1000, "boom")).await;
    let (b, _) = make(&server, Market::Spot, 1);
    assert_eq!(b.rehearse(10.0, None).await, (false, "바이낸스 확인에 실패했어요: APIError(code=-1000): boom".into()));
    assert_eq!(b.last_error_name(), "");

    let server = MockServer::start().await;
    mount(&server, "GET", "/api/v3/account", ok(json!({"balances": []}))).await;
    mount(&server, "GET", "/api/v3/exchangeInfo", api_err(-1121, "Invalid symbol.")).await;
    let (b, _) = make(&server, Market::Spot, 1);
    assert_eq!(
        b.rehearse(10.0, None).await,
        (false, "BTCUSDT 심볼을 바이낸스에서 찾지 못했어요. 심볼 이름을 확인하세요.".into())
    );
}

#[tokio::test]
async fn below_the_minimum_notional_fails() {
    let server = spot_server("5").await;
    let (b, _) = make(&server, Market::Spot, 1);
    assert_eq!(
        b.rehearse(1.0, None).await,
        (
            false,
            "최소 주문 금액보다 적어요(바이낸스 최소 5 USDT · 이번 주문 1 USDT). 매크로 투자 금액을 올리세요.".into()
        )
    );
    assert_eq!(b.last_error_name(), "under_min_total");
}

async fn futures_server() -> MockServer {
    let server = MockServer::start().await;
    mount(&server, "GET", "/fapi/v2/balance", ok(json!([{"asset": "USDT", "balance": "100"}]))).await;
    mount(&server, "GET", "/fapi/v1/exchangeInfo", ok(futures_info())).await;
    server
}

#[tokio::test]
async fn futures_rehearsal_is_a_signed_read_and_touches_nothing_else() {
    let server = futures_server().await;
    let (b, _) = make(&server, Market::Futures, 5);
    let (ok_, reason) = b.rehearse(10.0, None).await;
    assert!(ok_, "{reason}");
    let all = server.received_requests().await.unwrap();
    assert!(all.iter().all(|r| r.method.as_str() == "GET"));
    assert!(param(&requests(&server, "GET", "/fapi/v2/balance").await[0], "signature").is_some());
    assert!(!b.rehearse(1.0, None).await.0);
    assert_eq!(b.last_error_name(), "under_min_total");
}

#[tokio::test]
async fn futures_bad_key_fails_even_though_check_account_would_swallow_it() {
    let server = MockServer::start().await;
    mount(&server, "GET", "/fapi/v2/balance", api_err(-2015, "Invalid API-key, IP, or permissions for action")).await;
    mount(&server, "GET", "/fapi/v1/exchangeInfo", ok(futures_info())).await;
    let (b, _) = make(&server, Market::Futures, 1);
    let (ok_, reason) = b.rehearse(10.0, None).await;
    assert!(!ok_ && reason.contains("키를 거절"), "{reason}");
    assert_eq!(b.last_error_name(), "binance_access_test");
}

#[tokio::test]
async fn futures_missing_symbol_fails() {
    let server = MockServer::start().await;
    mount(&server, "GET", "/fapi/v2/balance", ok(json!([]))).await;
    mount(&server, "GET", "/fapi/v1/exchangeInfo", ok(json!({"symbols": [{"symbol": "ETHUSDT", "filters": []}]})))
        .await;
    let (b, _) = make(&server, Market::Futures, 1);
    assert_eq!(
        b.rehearse(10.0, None).await,
        (
            false,
            "바이낸스 확인에 실패했어요: BTCUSDT 심볼을 바이낸스에서 찾지 못했어요. 심볼 이름을 확인하세요.".into()
        )
    );
}

#[test]
fn default_urls_follow_testnet() {
    let log: LogFn = Arc::new(|_m: String| {});
    let b = BinanceBroker::new(
        "k",
        "s",
        Market::Spot,
        "BTCUSDT",
        Side::Long,
        true,
        1,
        log.clone(),
        reqwest::Client::new(),
        None,
    );
    assert_eq!((b.spot_base.as_str(), b.futures_base.as_str()), (SPOT_TESTNET_BASE, FUTURES_DEMO_BASE));
    assert_eq!(b.futures_base, "https://demo-fapi.binance.com");
    let b =
        BinanceBroker::new("k", "s", Market::Spot, "BTCUSDT", Side::Long, false, 1, log, reqwest::Client::new(), None);
    assert_eq!((b.spot_base.as_str(), b.futures_base.as_str()), (SPOT_BASE, FUTURES_BASE));
    assert!(b.fee_from_base_asset());
    assert_eq!(b.bid_fee(), None);
}

#[tokio::test]
async fn an_ip_ban_seen_by_one_run_pauses_every_run_on_that_address() {
    let server = MockServer::start().await;
    mount(&server, "GET", "/api/v3/ticker/price", ResponseTemplate::new(418).insert_header("Retry-After", "120")).await;
    let (a, _) = make(&server, Market::Spot, 1);
    let (b, _) = make(&server, Market::Spot, 1);
    assert!(a.price().await.is_err());
    let before = server.received_requests().await.unwrap().len();
    // 다른 실행(b)은 같은 주소로 보내지 않음 — 차단 중에 두드리면 바이낸스가 차단을 늘림.
    let e = b.price().await.unwrap_err();
    assert!(exc_text(&e).contains("쉬는 중"), "{}", exc_text(&e));
    assert_eq!(server.received_requests().await.unwrap().len(), before);
}

// --- 시작 전 잔고 확인 --------------------------------------------------------

async fn spot_with_balances(balances: Value) -> MockServer {
    let server = MockServer::start().await;
    mount(&server, "GET", "/api/v3/account", ok(json!({ "balances": balances }))).await;
    mount(&server, "GET", "/api/v3/exchangeInfo", ok(spot_info())).await;
    server
}

#[tokio::test]
async fn spot_start_is_blocked_when_the_quote_balance_cannot_cover_the_first_order() {
    let server = spot_with_balances(json!([{"asset": "USDT", "free": "5.5"}, {"asset": "BTC", "free": "0"}])).await;
    let (b, _) = make(&server, Market::Spot, 1);
    let (ok_, reason) = b.rehearse(10.0, None).await;
    assert!(!ok_);
    assert_eq!(
        reason,
        "주문할 USDT 잔고가 모자라요. 첫 주문에 10 USDT가 필요한데 사용 가능한 USDT는 5.5 USDT예요. \
         바이낸스 현물 지갑에 USDT를 채우거나 매크로 투자 금액을 줄이세요."
    );
    assert_eq!(b.last_error_name(), "binance_funds_test");
    assert_eq!(requests(&server, "GET", "/api/v3/account").await.len(), 1, "계정 확인에서 읽은 잔고를 그대로 씀");

    // 딱 맞으면 통과 — 수량을 단위로 내려 주문하므로 이보다 더 쓰지 않음.
    let server = spot_with_balances(json!([{"asset": "USDT", "free": "10"}, {"asset": "BTC", "free": "0.01"}])).await;
    let (b, _) = make(&server, Market::Spot, 1);
    assert!(b.rehearse(10.0, None).await.0);
    assert_eq!(b.balance_note(), "사용 가능 잔고 10 USDT · 보유 BTC 0.01");
}

#[tokio::test]
async fn live_spot_shortfall_keeps_what_the_permission_lookup_found() {
    let cases = [
        (json!({"ipRestrict": true, "enableSpotAndMarginTrading": true}), "binance_funds"),
        (json!({"ipRestrict": false, "enableSpotAndMarginTrading": true}), "binance_funds_open"),
    ];
    for (perms, name) in cases {
        let server = spot_with_balances(json!([{"asset": "USDT", "free": "1"}])).await;
        restrictions(&server, perms).await;
        let b = make_live(&server, Market::Spot);
        let (ok_, reason) = b.rehearse(10.0, None).await;
        assert!(!ok_ && reason.contains("잔고가 모자라요"), "{reason}");
        assert_eq!(b.last_error_name(), name);
    }
    // 주문 권한이 꺼져 있으면 그 이유가 먼저임(잔고는 보지 않음).
    let server = spot_with_balances(json!([{"asset": "USDT", "free": "1"}])).await;
    restrictions(&server, json!({"ipRestrict": true, "enableSpotAndMarginTrading": false})).await;
    let b = make_live(&server, Market::Spot);
    assert!(!b.rehearse(10.0, None).await.0);
    assert_eq!(b.last_error_name(), "binance_no_trade");
}

#[tokio::test]
async fn futures_start_checks_the_available_margin_for_the_leverage() {
    // 증거금 잔액(balance)은 넉넉해도 주문에 쓸 수 있는 몫(availableBalance)이 모자람.
    let server = MockServer::start().await;
    mount(
        &server,
        "GET",
        "/fapi/v2/balance",
        ok(json!([{"asset": "USDT", "balance": "100", "availableBalance": "1.5"}])),
    )
    .await;
    mount(&server, "GET", "/fapi/v1/exchangeInfo", ok(futures_info())).await;
    let (b, logs) = make(&server, Market::Futures, 5);
    let (ok_, reason) = b.rehearse(10.0, None).await;
    assert!(!ok_);
    assert_eq!(
        reason,
        "선물 증거금이 모자라요. 첫 주문(5배)에 수수료 여유까지 증거금 2.01 USDT가 필요한데 \
         사용 가능한 증거금은 1.5 USDT예요. 바이낸스 선물 지갑으로 USDT를 옮기거나 매크로 투자 금액을 줄이세요."
    );
    assert_eq!(b.last_error_name(), "binance_funds_test");
    assert_eq!(requests(&server, "GET", "/fapi/v2/balance").await.len(), 1);
    assert_eq!(b.balance_note(), "사용 가능 증거금 1.5 USDT");
    // 계정 확인 로그도 주문에 쓸 수 있는 몫을 알림.
    b.check_account().await.unwrap();
    assert_eq!(logs.lock().unwrap().last().unwrap(), "연결 성공 · 선물 USDT 증거금: 1.5");

    let server = MockServer::start().await;
    mount(&server, "GET", "/fapi/v2/balance", ok(json!([{"asset": "USDT", "availableBalance": "2.01"}]))).await;
    mount(&server, "GET", "/fapi/v1/exchangeInfo", ok(futures_info())).await;
    let (b, _) = make(&server, Market::Futures, 5);
    assert!(b.rehearse(10.0, None).await.0);
}

// --- 수수료 ---------------------------------------------------------------

#[test]
fn spot_fill_fees_are_converted_to_the_quote_where_possible() {
    let fills = [
        json!({"price": "100", "qty": "1", "commission": "0.001", "commissionAsset": "BTC"}),
        json!({"price": "110", "qty": "1", "commission": "0.11", "commissionAsset": "USDT"}),
        json!({"price": "110", "qty": "0.5", "commission": "0.0001", "commissionAsset": "BNB"}),
        json!({"price": "110", "qty": "0.5", "commission": "0.0002", "commissionAsset": "BNB"}),
    ];
    let fee = spot_fills_fee(&fills, 3.0, "BTC", "USDT").unwrap();
    assert!((fee.quote - 0.21).abs() < 1e-12, "{fee:?}");
    assert_eq!(fee.other.len(), 1);
    assert_eq!(fee.other[0].0, "BNB");
    assert!((fee.other[0].1 - 0.0003).abs() < 1e-12);
    // 체결 목록이 없거나, 수량을 다 덮지 못하거나, 수수료 칸이 빠지면 모름.
    assert_eq!(spot_fills_fee(&[], 1.0, "BTC", "USDT"), None);
    assert_eq!(spot_fills_fee(&fills[..1], 3.0, "BTC", "USDT"), None);
    assert_eq!(
        spot_fills_fee(&[json!({"price": "100", "qty": "1", "commissionAsset": "BTC"})], 1.0, "BTC", "USDT"),
        None
    );
}

#[tokio::test]
async fn orders_carry_the_fee_from_the_fills_already_received() {
    let server = MockServer::start().await;
    mount(
        &server,
        "POST",
        "/api/v3/order",
        ok(json!({"orderId": 9, "status": "FILLED", "executedQty": "2", "avgPrice": "100",
        "fills": [{"price": "100", "qty": "2", "commission": "0.2", "commissionAsset": "USDT"}]})),
    )
    .await;
    let (b, _) = make(&server, Market::Spot, 1);
    let o = b.submit(order(OrderSide::Sell, 2.0, "ggp-s")).await.unwrap();
    assert_eq!(o.fee, Some(FeePaid { quote: 0.2, other: vec![] }));

    // 선물은 체결 응답에 수수료가 없음 — 따로 조회하지 않고 모름으로 둠.
    let server = MockServer::start().await;
    mount(
        &server,
        "POST",
        "/fapi/v1/order",
        ok(json!({"orderId": 3, "status": "FILLED", "executedQty": "1", "avgPrice": "100"})),
    )
    .await;
    let (b, _) = make(&server, Market::Futures, 1);
    let o = b.submit(order(OrderSide::Buy, 1.0, "ggp-f")).await.unwrap();
    assert_eq!(o.fee, None);
    assert_eq!(server.received_requests().await.unwrap().len(), 1);
}
