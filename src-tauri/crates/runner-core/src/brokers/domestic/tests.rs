use super::*;
use crate::brokers::test_keys::KEYS;
use serde_json::json;
use std::sync::Arc;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, Request, ResponseTemplate};

type Logs = Arc<Mutex<Vec<String>>>;

fn recorder() -> (LogFn, Logs) {
    let logs: Logs = Arc::new(Mutex::new(Vec::new()));
    let sink = logs.clone();
    (Arc::new(move |m: String| sink.lock().unwrap().push(m)), logs)
}

fn make(ex: Exchange, base: &str) -> (DomesticBroker, Logs) {
    let (log, logs) = recorder();
    let b = DomesticBroker::new(
        &KEYS.access,
        &KEYS.secret,
        ex,
        "KRW-BTC",
        log,
        reqwest::Client::new(),
        Some(base.to_string()),
    )
    .unwrap();
    (b, logs)
}

fn decode_part(raw: &str) -> Value {
    let bytes = base64::engine::general_purpose::URL_SAFE_NO_PAD.decode(raw).unwrap();
    serde_json::from_slice(&bytes).unwrap()
}

fn decode_jwt(token: &str) -> (Value, Value) {
    let parts: Vec<&str> = token.split('.').collect();
    assert_eq!(parts.len(), 3);
    (decode_part(parts[0]), decode_part(parts[1]))
}

fn bearer(req: &Request) -> String {
    req.headers.get("authorization").unwrap().to_str().unwrap().trim_start_matches("Bearer ").to_string()
}

fn q(pairs: &[(&str, &str)]) -> Vec<(String, String)> {
    pairs.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect()
}

fn filled() -> Value {
    json!({"state": "done", "executed_volume": "0.01",
           "trades": [{"price": "100000000", "volume": "0.01", "funds": "1000000"}]})
}

fn err(name: &str, message: &str, status: u16) -> ResponseTemplate {
    ResponseTemplate::new(status).set_body_json(json!({"error": {"name": name, "message": message}}))
}

async fn mount(server: &MockServer, verb: &str, p: &str, resp: ResponseTemplate) {
    Mock::given(method(verb)).and(path(p)).respond_with(resp).mount(server).await;
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

fn buy(notional: f64, id: &str) -> SubmitRequest {
    SubmitRequest {
        side: OrderSide::Buy,
        base_qty: None,
        notional: Some(notional),
        reduce_only: false,
        closing: None,
        client_id: id.into(),
    }
}

fn sell(qty: f64, id: &str) -> SubmitRequest {
    SubmitRequest {
        side: OrderSide::Sell,
        base_qty: Some(qty),
        notional: None,
        reduce_only: false,
        closing: None,
        client_id: id.into(),
    }
}

/// 보낸 본문(JSON 글자)의 키 순서 그대로 k=v를 이음.
fn ordered_pairs(body: &[u8]) -> Vec<(String, String)> {
    let text = std::str::from_utf8(body).unwrap();
    let inner = text.trim().trim_start_matches('{').trim_end_matches('}');
    inner
        .split(", ")
        .map(|kv| {
            let (k, v) = kv.split_once(": ").unwrap();
            (serde_json::from_str::<String>(k).unwrap(), serde_json::from_str::<String>(v).unwrap())
        })
        .collect()
}

// --- JWT ---------------------------------------------------------------

#[test]
fn jwt_has_three_parts_and_a_fresh_nonce() {
    let first = jwt_token(&KEYS.access, &KEYS.secret, None, "HS512", false);
    let second = jwt_token(&KEYS.access, &KEYS.secret, None, "HS512", false);
    assert_eq!(first.split('.').count(), 3);
    assert_ne!(first, second);
}

#[test]
fn query_hash_is_present_only_with_a_query() {
    let (_h, p) = decode_jwt(&jwt_token(&KEYS.access, &KEYS.secret, None, "HS512", false));
    assert!(p.get("query_hash").is_none());
    let (_h, p) = decode_jwt(&jwt_token(&KEYS.access, &KEYS.secret, Some(&[]), "HS512", false));
    assert!(p.get("query_hash").is_none());
    let (_h, p) =
        decode_jwt(&jwt_token(&KEYS.access, &KEYS.secret, Some(&q(&[("market", "KRW-BTC")])), "HS512", false));
    assert!(p.get("query_hash").is_some());
    assert_eq!(p["query_hash_alg"], "SHA512");
    assert_eq!(p["access_key"], KEYS.access.as_str());
    assert_eq!(p["nonce"].as_str().unwrap().len(), 36);
}

#[test]
fn payload_keys_are_in_the_original_order() {
    let token = jwt_token(&KEYS.access, &KEYS.secret, Some(&q(&[("market", "KRW-BTC")])), "HS256", true);
    let raw = token.split('.').nth(1).unwrap();
    let text = String::from_utf8(base64::engine::general_purpose::URL_SAFE_NO_PAD.decode(raw).unwrap()).unwrap();
    let idx = |k: &str| text.find(&format!("\"{k}\"")).unwrap();
    assert!(idx("access_key") < idx("nonce"));
    assert!(idx("nonce") < idx("timestamp"));
    assert!(idx("timestamp") < idx("query_hash"));
    assert!(idx("query_hash") < idx("query_hash_alg"));
    assert!(!text.contains(' '));
    let header = String::from_utf8(
        base64::engine::general_purpose::URL_SAFE_NO_PAD.decode(token.split('.').next().unwrap()).unwrap(),
    )
    .unwrap();
    assert_eq!(header, r#"{"alg":"HS256","typ":"JWT"}"#);
}

#[test]
fn signature_is_plain_hmac_over_header_and_payload() {
    for alg in ["HS512", "HS256"] {
        let token = jwt_token(&KEYS.access, &KEYS.secret, Some(&q(&[("market", "KRW-BTC")])), alg, false);
        let parts: Vec<&str> = token.split('.').collect();
        let input = format!("{}.{}", parts[0], parts[1]);
        let expected = if alg == "HS512" {
            let mut mac = Hmac::<Sha512>::new_from_slice(KEYS.secret.as_bytes()).unwrap();
            mac.update(input.as_bytes());
            mac.finalize().into_bytes().to_vec()
        } else {
            let mut mac = Hmac::<Sha256>::new_from_slice(KEYS.secret.as_bytes()).unwrap();
            mac.update(input.as_bytes());
            mac.finalize().into_bytes().to_vec()
        };
        assert_eq!(parts[2], b64(&expected), "{alg}");
        assert_eq!(decode_jwt(&token).0, json!({"alg": alg, "typ": "JWT"}));
    }
}

#[test]
fn query_hash_is_sha512_of_the_unencoded_query_string() {
    let (_h, p) = decode_jwt(&jwt_token(
        &KEYS.access,
        &KEYS.secret,
        Some(&q(&[("market", "KRW-BTC"), ("identifier", "ggp-1")])),
        "HS512",
        false,
    ));
    assert_eq!(p["query_hash"], hex::encode(Sha512::digest(b"market=KRW-BTC&identifier=ggp-1")));
}

#[test]
fn bithumb_token_is_hs256_with_a_millisecond_timestamp() {
    let (header, payload) = decode_jwt(&jwt_token(&KEYS.access, &KEYS.secret, None, "HS256", true));
    assert_eq!(header["alg"], "HS256");
    assert!(payload["timestamp"].as_u64().unwrap() > 1_700_000_000_000);
    let (_h, upbit) = decode_jwt(&jwt_token(&KEYS.access, &KEYS.secret, None, "HS512", false));
    assert!(upbit.get("timestamp").is_none());
}

#[tokio::test]
async fn broker_signs_with_its_exchanges_algorithm() {
    for (ex, alg) in [(Exchange::Upbit, "HS512"), (Exchange::Bithumb, "HS256")] {
        let server = MockServer::start().await;
        mount(
            &server,
            "GET",
            "/v1/accounts",
            ResponseTemplate::new(200).set_body_json(json!([{"currency": "KRW", "balance": "1"}])),
        )
        .await;
        let (b, _) = make(ex, &server.uri());
        b.check_account().await.unwrap();
        let reqs = requests(&server, "GET", "/v1/accounts").await;
        assert_eq!(decode_jwt(&bearer(&reqs[0])).0["alg"], alg);
    }
}

#[test]
fn non_domestic_exchange_is_refused() {
    let (log, _) = recorder();
    let e =
        DomesticBroker::new("a", "s", Exchange::Binance, "BTCUSDT", log, reqwest::Client::new(), None).err().unwrap();
    assert_eq!(e.to_string(), "지원하지 않는 국내 거래소예요: binance");
}

// --- 오류 설명 -----------------------------------------------------------

#[test]
fn ip_class_errors_name_the_allowed_ip_before_the_key() {
    for (ex, name) in [
        (Exchange::Upbit, "no_authorization_ip"),
        (Exchange::Upbit, "invalid_access_key"),
        (Exchange::Bithumb, "NotAllowIP"),
        (Exchange::Upbit, "no_authorization_token"),
    ] {
        let text = explain_domestic_error(ex, name, "허용되지 않은 IP");
        assert!(text.find("허용 IP").unwrap() < text.find("API 키").unwrap(), "{text}");
    }
    assert_eq!(
        explain_domestic_error(Exchange::Upbit, "no_authorization_ip", ""),
        "업비트에서 요청을 거절했어요. 먼저 이 기기의 현재 IP가 업비트의 \
         허용 IP에 등록돼 있는지 확인하세요. 집 인터넷 IP는 바뀔 수 있어요. IP가 맞다면 \
         API 키(Access Key · Secret Key)를 다시 복사해 넣으세요."
    );
}

/// 사람이 읽는 문장에는 오류 이름(코드)을 넣지 않음 — 코드는 `domestic_error_code`로 따로 감.
const ALL_NAMES: [&str; 20] = [
    "no_authorization_ip",
    "NotAllowIP",
    "invalid_access_key",
    "no_authorization_token",
    "invalid_query_payload",
    "jwt_verification",
    "out_of_scope",
    "expired_jwt",
    "expired_access_key",
    "under_min_total_bid",
    "under_min_total_market_bid",
    "insufficient_funds_bid",
    "insufficient_funds_ask",
    "insufficient_funds",
    "over_krw_funds_bid",
    "notfoundmarket",
    "invalid_market",
    "market_offline",
    "brand_new_error",
    "x",
];

#[test]
fn sentences_never_carry_the_error_code() {
    for ex in [Exchange::Upbit, Exchange::Bithumb] {
        for name in ALL_NAMES {
            let text = explain_domestic_error(ex, name, "");
            assert!(!text.contains(name), "{name}: {text}");
            assert!(!text.contains('(') || text.contains("(JWT)") || text.contains("(Access Key"), "{name}: {text}");
            assert_eq!(domestic_error_code(400, name), name, "코드는 오류 이름 그대로");
        }
    }
    assert_eq!(domestic_error_code(403, ""), "HTTP 403", "이름이 없으면 HTTP 상태가 코드");
}

#[test]
fn each_error_class_has_its_own_text() {
    let e = |name: &str| explain_domestic_error(Exchange::Upbit, name, "fixture");
    let sig = e("invalid_query_payload");
    assert!(sig.contains("서명") && !sig.contains("집 IP 는 바뀝니다") && !sig.contains("다시 복사"));
    let jwt = explain_domestic_error(Exchange::Bithumb, "jwt_verification", "");
    assert_eq!(
        jwt,
        "빗썸에서 인증 토큰(JWT) 서명을 검증하지 못했어요. Access Key와 Secret Key가 같은 발급 건인지, \
         Secret Key를 빠짐없이 붙여 넣었는지 확인한 뒤 다시 연결을 확인하세요. 계속 실패하면 개발팀에 알려 주세요."
    );
    assert!(!jwt.contains("허용 IP") && !jwt.contains("만료"));
    assert_eq!(
        e("out_of_scope"),
        "이 API 키에 주문 권한이 없어요. 업비트 API 관리에서 주문하기 권한을 켠 키를 쓰세요."
    );
    let expired_jwt = e("expired_jwt");
    assert!(expired_jwt.contains("JWT") && expired_jwt.contains("시계"));
    assert!(!expired_jwt.contains("API 키가 만료") && !expired_jwt.contains("재발급") && !expired_jwt.contains("연장"));
    for ex in [Exchange::Upbit, Exchange::Bithumb] {
        let t = explain_domestic_error(ex, "expired_access_key", "fixture");
        assert!(t.contains("API 키가 만료") && t.contains("삭제") && t.contains("발급") && !t.contains("연장"));
    }
    assert_eq!(
        e("under_min_total_market_bid"),
        "최소 주문 금액보다 적어요. 매크로 투자 금액을 업비트 최소 주문 금액 이상으로 올리세요."
    );
    for name in ["insufficient_funds_bid", "insufficient_funds_ask", "insufficient_funds"] {
        assert_eq!(e(name), "주문할 원화 잔고가 모자라요. 잔고를 채우거나 매크로 투자 금액을 줄이세요.");
        assert!(is_no_funds_error(name), "{name}");
    }
    assert!(!is_no_funds_error("under_min_total_bid") && !is_no_funds_error("over_krw_funds_bid"));
    assert_eq!(e("over_krw_funds_bid"), "거래소의 1회 최대 주문 금액을 넘었어요. 매크로 투자 금액을 줄이세요.");
    for name in ["notfoundmarket", "invalid_market", "market_offline"] {
        assert!(e(name).starts_with("이 마켓은 지금 주문할 수 없어요"));
    }
    // 모르는 오류는 거래소 설명을 그대로 두고, 이름(코드)은 문장에 넣지 않음.
    assert_eq!(e("brand_new_error"), "업비트에서 주문을 거절했어요: fixture");
    assert_eq!(explain_domestic_error(Exchange::Upbit, "x", ""), "업비트에서 주문을 거절했어요.");
    assert_eq!(explain_domestic_error(Exchange::Upbit, "", "말"), "업비트에서 주문을 거절했어요: 말");
    assert_eq!(explain_domestic_error(Exchange::Bithumb, "", ""), "빗썸에서 주문을 거절했어요.");
}

#[test]
fn access_classification() {
    assert!(is_access_error(400, "jwt_verification"));
    assert!(is_access_error(400, "no_authorization_ip"));
    assert!(is_access_error(401, ""));
    assert!(is_access_error(403, "whatever"));
    assert!(!is_access_error(404, "invalid_market"));
    assert!(!is_access_error(400, "under_min_total_bid"));
}

// --- 주문 본문·상태 --------------------------------------------------------

#[test]
fn order_body_keys_and_amounts() {
    let (up, _) = make(Exchange::Upbit, "http://127.0.0.1:9");
    assert_eq!(
        up.order_body(OrderSide::Buy, None, Some(5000.0), false, "ggp-9").unwrap(),
        q(&[("market", "KRW-BTC"), ("side", "bid"), ("ord_type", "price"), ("price", "5000"), ("identifier", "ggp-9")])
    );
    assert_eq!(
        up.order_body(OrderSide::Sell, Some(0.1 + 0.2), None, false, "ggp-8").unwrap(),
        q(&[
            ("market", "KRW-BTC"),
            ("side", "ask"),
            ("ord_type", "market"),
            ("volume", "0.3"),
            ("identifier", "ggp-8")
        ])
    );
    let body = up.order_body(OrderSide::Buy, None, Some(100_000.0 / 3.0), false, "ggp-won").unwrap();
    assert_eq!(body[3].1, "33333");
    let body = up.order_body(OrderSide::Sell, Some(0.123456789), None, false, "x").unwrap();
    assert_eq!(body[3].1, "0.12345678");
    let body = up.order_body(OrderSide::Buy, None, Some(1_000_000.0), false, "x").unwrap();
    assert_eq!(body[3].1, "1000000");

    let (bt, _) = make(Exchange::Bithumb, "http://127.0.0.1:9");
    assert_eq!(
        bt.order_body(OrderSide::Buy, None, Some(1.0), false, "ggp-7").unwrap(),
        q(&[
            ("market", "KRW-BTC"),
            ("side", "bid"),
            ("order_type", "price"),
            ("price", "1"),
            ("client_order_id", "ggp-7")
        ])
    );
    assert!(bt.order_body(OrderSide::Buy, None, Some(5000.0), false, &"x".repeat(36)).is_ok());
    assert!(bt.order_body(OrderSide::Buy, None, Some(5000.0), false, &"x".repeat(37)).is_err());
    assert!(up.order_body(OrderSide::Buy, None, Some(5000.0), false, &"x".repeat(64)).is_ok());
    assert!(up.order_body(OrderSide::Buy, None, Some(5000.0), false, &"x".repeat(65)).is_err());
}

#[test]
fn order_body_validation_errors() {
    let (up, _) = make(Exchange::Upbit, "http://127.0.0.1:9");
    let msg = |r: Result<Vec<(String, String)>, BrokerError>| r.unwrap_err().to_string();
    for n in [None, Some(0.0), Some(f64::NAN), Some(-5.0)] {
        assert_eq!(
            msg(up.order_body(OrderSide::Buy, Some(0.01), n, false, "x")),
            "시장가 매수에는 0보다 큰 원화 금액이 필요해요"
        );
    }
    for qty in [None, Some(0.0), Some(1e-12), Some(1e-30)] {
        assert_eq!(
            msg(up.order_body(OrderSide::Sell, qty, Some(1e6), false, "x")),
            "시장가 매도에는 0보다 큰 수량이 필요해요"
        );
    }
    assert_eq!(msg(up.order_body(OrderSide::Buy, None, Some(0.4), false, "x")), "시장가 매수 금액이 1원보다 적어요");
    assert_eq!(
        msg(up.order_body(OrderSide::Buy, None, Some(5000.0), true, "x")),
        "국내 현물은 롱만 있어 포지션 줄이기 매수를 쓸 수 없어요"
    );
    assert_eq!(msg(up.order_body(OrderSide::Buy, None, Some(5000.0), false, "")), "주문 식별자는 1~64자여야 해요");
    let (bt, _) = make(Exchange::Bithumb, "http://127.0.0.1:9");
    assert_eq!(
        msg(bt.order_body(OrderSide::Buy, None, Some(5000.0), false, &"x".repeat(37))),
        "주문 식별자는 1~36자여야 해요"
    );
}

#[test]
fn status_folding() {
    assert_eq!(fold_domestic_status(Some(&json!("done")), 0.0), "FILLED");
    assert_eq!(fold_domestic_status(Some(&json!("cancel")), 0.0), "CANCELED");
    assert_eq!(fold_domestic_status(Some(&json!("cancel")), 0.01), "FILLED");
    assert_eq!(fold_domestic_status(Some(&json!("wait")), 0.0), "OPEN");
    assert_eq!(fold_domestic_status(Some(&json!("watch")), 0.0), "OPEN");
    assert_eq!(fold_domestic_status(Some(&json!("")), 0.0), "UNKNOWN");
    assert_eq!(fold_domestic_status(None, 0.0), "UNKNOWN");
    assert_eq!(fold_domestic_status(Some(&Value::Null), 1.0), "UNKNOWN");
}

#[test]
fn normalize_reads_trades_and_holds_the_shape() {
    let o = normalize_domestic(&json!({"state": "done", "executed_volume": "0.02",
        "trades": [{"price": "100", "volume": "0.01", "funds": "1.0"}, {"price": "300", "volume": "0.01", "funds": "3.0"}]}));
    assert_eq!(o.avg_price, 200.0);
    assert!(o.fees_known);
    assert_eq!(o.acquired_qty, o.executed_qty);
    // funds가 없으면 price * volume
    let o = normalize_domestic(
        &json!({"state": "done", "executed_volume": "0.01", "trades": [{"price": "100", "volume": "0.01"}]}),
    );
    assert!((o.avg_price - 100.0).abs() < 1e-9);
    let o = normalize_domestic(
        &json!({"state": "done", "executed_volume": "0.02", "trades": [{"price": "100", "volume": "0.01", "funds": "1.0"}]}),
    );
    assert!(!o.fees_known);
    let o = normalize_domestic(&json!({"state": "done", "executed_volume": "0.01", "trades": ["0.01"]}));
    assert_eq!(o.status, "FILLED");
    assert!(!o.fees_known);
    for body in [json!([{"state": "done"}]), json!("maintenance"), Value::Null, json!(7)] {
        let o = normalize_domestic(&body);
        assert_eq!((o.status.as_str(), o.raw_status.as_str()), ("UNKNOWN", "unknown"));
    }
    let o = normalize_domestic(&json!({"state": "done", "uuid": "abc-123", "executed_volume": "0"}));
    assert_eq!(o.order_id.as_deref(), Some("abc-123"));
    let o = normalize_domestic(&json!({"state": "done", "order_id": "b-1"}));
    assert_eq!(o.order_id.as_deref(), Some("b-1"));
    let o = normalize_domestic(&json!({"state": "done"}));
    assert_eq!(o.order_id, None);
}

// --- 주문 --------------------------------------------------------------

#[tokio::test]
async fn market_buy_sends_krw_total_and_signs_the_body_in_order() {
    let server = MockServer::start().await;
    mount(
        &server,
        "POST",
        "/v1/orders",
        ResponseTemplate::new(201).set_body_json(json!({"uuid": "u1", "state": "wait"})),
    )
    .await;
    mount(&server, "GET", "/v1/order", ResponseTemplate::new(200).set_body_json(filled())).await;
    let (b, _) = make(Exchange::Upbit, &server.uri());
    let order = b.submit(buy(5000.0, "ggp-10")).await.unwrap();
    assert_eq!(order.status, "FILLED");
    let posts = requests(&server, "POST", "/v1/orders").await;
    assert_eq!(posts.len(), 1);
    let pairs = ordered_pairs(&posts[0].body);
    assert_eq!(
        pairs,
        q(&[
            ("market", "KRW-BTC"),
            ("side", "bid"),
            ("ord_type", "price"),
            ("price", "5000"),
            ("identifier", "ggp-10")
        ])
    );
    let (_h, payload) = decode_jwt(&bearer(&posts[0]));
    assert_eq!(payload["query_hash"], hex::encode(Sha512::digest(plain_query(&pairs).as_bytes())));
    let body: Value = serde_json::from_slice(&posts[0].body).unwrap();
    assert!(body.get("volume").is_none());
    let gets = requests(&server, "GET", "/v1/order").await;
    assert_eq!(gets[0].url.query(), Some("identifier=ggp-10"));
}

#[tokio::test]
async fn market_sell_sends_volume_not_price() {
    let server = MockServer::start().await;
    mount(&server, "POST", "/v1/orders", ResponseTemplate::new(201).set_body_json(json!({"uuid": "u2"}))).await;
    mount(&server, "GET", "/v1/order", ResponseTemplate::new(200).set_body_json(filled())).await;
    let (b, _) = make(Exchange::Upbit, &server.uri());
    let order = b.submit(sell(0.01, "ggp-2")).await.unwrap();
    assert_eq!((order.executed_qty, order.acquired_qty), (0.01, 0.01));
    let body: Value = serde_json::from_slice(&requests(&server, "POST", "/v1/orders").await[0].body).unwrap();
    assert_eq!(body["side"], "ask");
    assert_eq!(body["ord_type"], "market");
    assert_eq!(body["volume"], "0.01");
    assert!(body.get("price").is_none());
}

#[tokio::test]
async fn bithumb_uses_its_own_order_endpoint_and_keys() {
    let server = MockServer::start().await;
    mount(&server, "POST", "/v2/orders", ResponseTemplate::new(201).set_body_json(json!({"order_id": "u7"}))).await;
    mount(
        &server,
        "GET",
        "/v1/order",
        ResponseTemplate::new(200).set_body_json(json!({"state": "done", "executed_volume": "0.01",
        "trades": [{"price": "100", "volume": "0.01", "funds": "1.0"}]})),
    )
    .await;
    let (b, _) = make(Exchange::Bithumb, &server.uri());
    b.submit(buy(1.0, "ggp-7")).await.unwrap();
    let posts = requests(&server, "POST", "/v2/orders").await;
    let body: Value = serde_json::from_slice(&posts[0].body).unwrap();
    assert_eq!(body["order_type"], "price");
    assert_eq!(body["client_order_id"], "ggp-7");
    assert!(body.get("ord_type").is_none() && body.get("identifier").is_none());
    assert_eq!(requests(&server, "GET", "/v1/order").await[0].url.query(), Some("client_order_id=ggp-7"));
    assert!(requests(&server, "POST", "/v1/orders").await.is_empty());
}

#[tokio::test]
async fn invalid_arguments_raise_and_send_nothing() {
    let server = MockServer::start().await;
    let (b, _) = make(Exchange::Upbit, &server.uri());
    assert!(b.submit(buy(0.0, "ggp-x")).await.is_err());
    assert!(b.submit(sell(0.0, "ggp-x")).await.is_err());
    let mut r = buy(5000.0, "ggp-x");
    r.reduce_only = true;
    assert!(b.submit(r).await.is_err());
    assert!(b.submit(buy(5000.0, "")).await.is_err());
    assert!(server.received_requests().await.unwrap_or_default().is_empty());
}

#[tokio::test]
async fn server_error_on_post_is_reconciled_not_resent() {
    let server = MockServer::start().await;
    mount(&server, "POST", "/v1/orders", err("server_error", "", 500)).await;
    mount(&server, "GET", "/v1/order", ResponseTemplate::new(200).set_body_json(filled())).await;
    let (b, _) = make(Exchange::Upbit, &server.uri());
    let order = b.submit(buy(1_000_000.0, "ggp-13")).await.unwrap();
    assert_eq!(order.status, "FILLED");
    assert_eq!(requests(&server, "POST", "/v1/orders").await.len(), 1);
}

#[tokio::test]
async fn no_answer_at_all_is_unknown_and_never_resends() {
    // 닫힌 포트 — POST도 조회도 연결이 안 됨(타임아웃과 같은 길).
    let (b, logs) = make(Exchange::Upbit, "http://127.0.0.1:9");
    let order = b.submit(buy(5000.0, "ggp-12")).await.unwrap();
    // 조회 결과가 없으면 빈 응답으로 접음 — 상태는 UNKNOWN, 체결 0.
    assert_eq!((order.status.as_str(), order.raw_status.as_str()), ("UNKNOWN", "unknown"));
    assert_eq!((order.executed_qty, order.order_id.clone()), (0.0, None));
    assert!(logs.lock().unwrap().is_empty());
}

#[tokio::test]
async fn named_client_error_is_a_rejection_in_the_exchanges_words() {
    let server = MockServer::start().await;
    mount(&server, "POST", "/v1/orders", err("insufficient_funds_bid", "잔고 부족", 400)).await;
    mount(&server, "GET", "/v1/order", err("order_not_found", "", 404)).await;
    let (b, _) = make(Exchange::Upbit, &server.uri());
    let order = b.submit(buy(1_000_000.0, "ggp-14")).await.unwrap();
    assert_eq!((order.status.as_str(), order.raw_status.as_str()), ("REJECTED", "insufficient_funds_bid"));
    assert_eq!(order.executed_qty, 0.0);
    assert!(order.fees_known);
    assert_eq!(requests(&server, "POST", "/v1/orders").await.len(), 1);
    assert_eq!(requests(&server, "GET", "/v1/order").await.len(), 1);
}

#[tokio::test]
async fn unnamed_client_error_gets_the_full_reconcile() {
    let server = MockServer::start().await;
    mount(&server, "POST", "/v1/orders", ResponseTemplate::new(403).set_body_string("<html>Forbidden</html>")).await;
    Mock::given(method("GET"))
        .and(path("/v1/order"))
        .respond_with(err("order_not_found", "", 404))
        .up_to_n_times(1)
        .with_priority(1)
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/v1/order"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"state": "wait", "executed_volume": "0"})))
        .up_to_n_times(1)
        .with_priority(2)
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/v1/order"))
        .respond_with(ResponseTemplate::new(200).set_body_json(filled()))
        .with_priority(3)
        .mount(&server)
        .await;
    let (b, _) = make(Exchange::Upbit, &server.uri());
    let order = b.submit(buy(1_000_000.0, "ggp-17")).await.unwrap();
    assert_eq!(order.status, "FILLED");
    assert_eq!(requests(&server, "GET", "/v1/order").await.len(), 3);
    assert_eq!(requests(&server, "POST", "/v1/orders").await.len(), 1);
}

#[tokio::test]
async fn unnamed_client_error_with_nothing_found_is_unknown_not_rejected() {
    let server = MockServer::start().await;
    mount(&server, "POST", "/v1/orders", ResponseTemplate::new(403)).await;
    mount(&server, "GET", "/v1/order", err("order_not_found", "", 404)).await;
    let (b, _) = make(Exchange::Upbit, &server.uri());
    let order = b.submit(buy(1_000_000.0, "ggp-18")).await.unwrap();
    assert_eq!(order.status, "UNKNOWN");
    assert_eq!(requests(&server, "GET", "/v1/order").await.len(), crate::brokers::LOOKUP_ATTEMPTS);
}

#[tokio::test]
async fn client_error_but_order_found_trusts_the_lookup() {
    let server = MockServer::start().await;
    mount(&server, "POST", "/v1/orders", err("too_many_requests", "", 429)).await;
    mount(
        &server,
        "GET",
        "/v1/order",
        ResponseTemplate::new(200).set_body_json(json!({"uuid": "abc-123", "state": "done",
        "executed_volume": "0.01", "trades": [{"price": "100000000", "volume": "0.01", "funds": "1000000"}]})),
    )
    .await;
    let (b, _) = make(Exchange::Upbit, &server.uri());
    let order = b.submit(buy(1_000_000.0, "ggp-15")).await.unwrap();
    assert_eq!(order.status, "FILLED");
    assert_eq!(order.order_id.as_deref(), Some("abc-123"));
}

#[tokio::test]
async fn a_partial_fill_seen_once_is_kept_when_later_lookups_fail() {
    let server = MockServer::start().await;
    mount(&server, "POST", "/v1/orders", ResponseTemplate::new(201).set_body_json(json!({"uuid": "u"}))).await;
    Mock::given(method("GET"))
        .and(path("/v1/order"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"state": "wait", "executed_volume": "0.005",
            "trades": [{"price": "100", "volume": "0.005", "funds": "0.5"}]})))
        .up_to_n_times(1)
        .with_priority(1)
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/v1/order"))
        .respond_with(ResponseTemplate::new(500))
        .with_priority(2)
        .mount(&server)
        .await;
    let (b, _) = make(Exchange::Upbit, &server.uri());
    let order = b.submit(buy(5000.0, "ggp-p")).await.unwrap();
    assert_eq!((order.status.as_str(), order.raw_status.as_str()), ("OPEN", "wait"));
    assert_eq!(order.executed_qty, 0.005);
    assert!((order.avg_price - 100.0).abs() < 1e-9);
    assert_eq!(requests(&server, "GET", "/v1/order").await.len(), crate::brokers::LOOKUP_ATTEMPTS);
}

#[tokio::test]
async fn a_non_object_lookup_body_is_unknown_not_an_error() {
    for body in [json!([{"state": "done"}]), json!("maintenance"), json!(7), Value::Null] {
        let server = MockServer::start().await;
        mount(&server, "POST", "/v1/orders", ResponseTemplate::new(201).set_body_json(json!({"uuid": "u"}))).await;
        mount(&server, "GET", "/v1/order", ResponseTemplate::new(200).set_body_json(body)).await;
        let (b, _) = make(Exchange::Upbit, &server.uri());
        let order = b.submit(buy(5000.0, "ggp-shape")).await.unwrap();
        assert_eq!(order.status, "UNKNOWN");
        assert_eq!(requests(&server, "POST", "/v1/orders").await.len(), 1);
        assert_eq!(requests(&server, "GET", "/v1/order").await.len(), crate::brokers::LOOKUP_ATTEMPTS);
    }
}

#[tokio::test]
async fn state_is_folded_and_the_exchange_word_is_kept() {
    let cases = [
        (json!({"state": "done", "executed_volume": "0"}), "FILLED", "done"),
        (json!({"state": "cancel", "executed_volume": "0"}), "CANCELED", "cancel"),
        (json!({"state": "wait", "executed_volume": "0"}), "OPEN", "wait"),
        (
            json!({"state": "cancel", "executed_volume": "0.01", "trades": [{"price": "1", "volume": "0.01", "funds": "1"}]}),
            "FILLED",
            "cancel",
        ),
    ];
    for (payload, status, raw) in cases {
        let server = MockServer::start().await;
        mount(&server, "POST", "/v1/orders", ResponseTemplate::new(201).set_body_json(json!({"uuid": "u"}))).await;
        mount(&server, "GET", "/v1/order", ResponseTemplate::new(200).set_body_json(payload)).await;
        let (b, _) = make(Exchange::Upbit, &server.uri());
        let order = b.submit(buy(5000.0, "ggp-11")).await.unwrap();
        assert_eq!((order.status.as_str(), order.raw_status.as_str()), (status, raw));
    }
}

// --- 준비·시세 -------------------------------------------------------------

#[tokio::test]
async fn order_rules_and_fees_come_from_chance() {
    let server = MockServer::start().await;
    mount(
        &server,
        "GET",
        "/v1/orders/chance",
        ResponseTemplate::new(200).set_body_json(json!({"bid_fee": "0.0005", "ask_fee": "0.0005",
        "bid": {"min_total": "5000"}, "ask": {"min_total": "5000"}})),
    )
    .await;
    let (b, _) = make(Exchange::Upbit, &server.uri());
    assert!(b.ensure_ready().await.unwrap());
    assert_eq!(b.order_rules().await.unwrap(), OrderRules { step: 0.0, min_notional: 5000.0 });
    assert_eq!(b.fees(), (0.0005, 0.0005));
    assert_eq!(b.bid_fee(), Some(0.0005));
    let reqs = requests(&server, "GET", "/v1/orders/chance").await;
    assert_eq!(reqs.len(), 1, "order_rules 는 읽어 둔 값을 쓴다");
    assert_eq!(reqs[0].url.query(), Some("market=KRW-BTC"));
    let (_h, payload) = decode_jwt(&bearer(&reqs[0]));
    assert_eq!(payload["query_hash"], hex::encode(Sha512::digest(b"market=KRW-BTC")));
}

#[tokio::test]
async fn missing_market_is_false_but_access_errors_are_raised() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/v1/orders/chance"))
        .respond_with(err("invalid_market", "", 404))
        .up_to_n_times(2)
        .with_priority(1)
        .mount(&server)
        .await;
    mount(&server, "GET", "/v1/orders/chance", err("invalid_access_key", "", 401)).await;
    let (b, _) = make(Exchange::Upbit, &server.uri());
    assert!(!b.load_market().await.unwrap());
    assert!(!b.ensure_ready().await.unwrap());
    match b.ensure_ready().await {
        Err(BrokerError::Access { name, explanation }) => {
            assert_eq!(name, "invalid_access_key");
            assert!(explanation.contains("허용 IP"));
        }
        other => panic!("{other:?}"),
    }
}

#[tokio::test]
async fn an_ip_error_is_access_even_as_a_400() {
    for status in [400u16, 401, 403] {
        let server = MockServer::start().await;
        mount(&server, "GET", "/v1/orders/chance", err("no_authorization_ip", "", status)).await;
        let (b, _) = make(Exchange::Upbit, &server.uri());
        let e = b.load_market().await.unwrap_err();
        assert!(matches!(e, BrokerError::Access { .. }));
        assert!(e.to_string().contains("허용 IP") && !e.to_string().contains("심볼"));
    }
    for name in ["invalid_access_key", "expired_access_key", "out_of_scope", "jwt_verification"] {
        let server = MockServer::start().await;
        mount(&server, "GET", "/v1/orders/chance", err(name, "", 400)).await;
        let (b, _) = make(Exchange::Upbit, &server.uri());
        assert!(matches!(b.load_market().await, Err(BrokerError::Access { .. })), "{name}");
    }
}

#[tokio::test]
async fn price_is_public_and_unsigned() {
    let server = MockServer::start().await;
    mount(
        &server,
        "GET",
        "/v1/ticker",
        ResponseTemplate::new(200).set_body_json(json!([{"market": "KRW-BTC", "trade_price": 101500000.0}])),
    )
    .await;
    let (b, _) = make(Exchange::Upbit, &server.uri());
    assert_eq!(b.price().await.unwrap(), 101500000.0);
    let reqs = requests(&server, "GET", "/v1/ticker").await;
    assert_eq!(reqs[0].url.query(), Some("markets=KRW-BTC"));
    assert!(reqs[0].headers.get("authorization").is_none());

    let server = MockServer::start().await;
    mount(&server, "GET", "/v1/ticker", ResponseTemplate::new(200).set_body_json(json!([]))).await;
    let (b, _) = make(Exchange::Upbit, &server.uri());
    assert_eq!(b.price().await.unwrap_err().to_string(), "KRW-BTC 현재가를 받지 못했어요");
}

#[tokio::test]
async fn check_account_reads_the_krw_balance() {
    let server = MockServer::start().await;
    mount(
        &server,
        "GET",
        "/v1/accounts",
        ResponseTemplate::new(200)
            .set_body_json(json!([{"currency": "BTC", "balance": "0.1"}, {"currency": "KRW", "balance": "250000"}])),
    )
    .await;
    let (b, logs) = make(Exchange::Upbit, &server.uri());
    b.check_account().await.unwrap();
    assert_eq!(logs.lock().unwrap().last().unwrap(), "연결 성공 · 원화 잔고: 250000");
    let reqs = requests(&server, "GET", "/v1/accounts").await;
    assert!(reqs[0].url.query().is_none());
    let (_h, payload) = decode_jwt(&bearer(&reqs[0]));
    assert!(payload.get("query_hash").is_none());
}

// --- 리허설: 업비트 ---------------------------------------------------------

fn chance() -> Value {
    json!({"bid_fee": "0.0005", "ask_fee": "0.0005", "bid": {"min_total": "5000"}, "ask": {"min_total": "5000"}})
}

#[tokio::test]
async fn upbit_uses_the_test_endpoint_with_the_real_body_shape() {
    let server = MockServer::start().await;
    mount(&server, "POST", "/v1/orders/test", ResponseTemplate::new(201).set_body_json(json!({"uuid": "t1"}))).await;
    mount(&server, "POST", "/v1/orders", ResponseTemplate::new(201).set_body_json(json!({"uuid": "u"}))).await;
    mount(&server, "GET", "/v1/order", ResponseTemplate::new(200).set_body_json(filled())).await;
    let (b, _) = make(Exchange::Upbit, &server.uri());
    let (ok, reason) = b.rehearse(10_000.0, None).await;
    assert!(ok, "{reason}");
    assert_eq!(reason, "업비트 주문 검증을 통과했어요. 키 · 허용 IP · 주문 권한 · 최소 주문 금액 모두 문제없어요.");
    assert!(requests(&server, "POST", "/v1/orders").await.is_empty());
    let test_req = &requests(&server, "POST", "/v1/orders/test").await[0];
    let rehearsal = ordered_pairs(&test_req.body);
    let (_h, payload) = decode_jwt(&bearer(test_req));
    assert_eq!(payload["query_hash"], hex::encode(Sha512::digest(plain_query(&rehearsal).as_bytes())));
    assert!(rehearsal[4].1.starts_with("ggp-rehearsal-") && rehearsal[4].1.len() <= 64);
    b.submit(buy(10_000.0, "ggp-1")).await.unwrap();
    let real = ordered_pairs(&requests(&server, "POST", "/v1/orders").await[0].body);
    let keys = |p: &[(String, String)]| p.iter().map(|(k, _)| k.clone()).collect::<Vec<_>>();
    assert_eq!(keys(&rehearsal), keys(&real));
    assert_eq!(rehearsal[..4], real[..4]);
}

#[tokio::test]
async fn upbit_test_order_uses_the_real_amount_when_given_the_spend() {
    let server = MockServer::start().await;
    mount(&server, "GET", "/v1/orders/chance", ResponseTemplate::new(200).set_body_json(chance())).await;
    mount(&server, "POST", "/v1/orders/test", ResponseTemplate::new(201).set_body_json(json!({"uuid": "t1"}))).await;
    let (b, _) = make(Exchange::Upbit, &server.uri());
    let (ok, reason) = b.rehearse(fee_inclusive(20_000.0, DOMESTIC_BID_FEE_FALLBACK), Some(20_000.0)).await;
    assert!(ok, "{reason}");
    let body: Value = serde_json::from_slice(&requests(&server, "POST", "/v1/orders/test").await[0].body).unwrap();
    assert_eq!(body["price"], "19990");
}

#[tokio::test]
async fn the_rehearsal_order_uses_the_same_integer_amount() {
    let server = MockServer::start().await;
    mount(&server, "POST", "/v1/orders/test", ResponseTemplate::new(201).set_body_json(json!({"uuid": "t"}))).await;
    let (b, _) = make(Exchange::Upbit, &server.uri());
    assert!(b.rehearse(100_000.0 / 3.0, None).await.0);
    let body: Value = serde_json::from_slice(&requests(&server, "POST", "/v1/orders/test").await[0].body).unwrap();
    assert_eq!(body["price"], "33333");
}

#[tokio::test]
async fn upbit_status_other_than_201_is_not_a_pass() {
    let server = MockServer::start().await;
    mount(&server, "POST", "/v1/orders/test", ResponseTemplate::new(200).set_body_json(json!({"ok": true}))).await;
    let (b, _) = make(Exchange::Upbit, &server.uri());
    assert_eq!(
        b.rehearse(10_000.0, None).await,
        (false, "업비트 주문 검증이 예상 밖 응답을 줬어요(HTTP 200)".to_string())
    );
}

#[tokio::test]
async fn rehearsal_errors_are_explained_and_named() {
    let cases: [(Exchange, &str, u16, &str); 6] = [
        (Exchange::Upbit, "no_authorization_ip", 401, "허용 IP"),
        (Exchange::Bithumb, "NotAllowIP", 401, "허용 IP"),
        (Exchange::Upbit, "jwt_verification", 400, "JWT"),
        (Exchange::Upbit, "under_min_total_bid", 400, "최소 주문 금액"),
        (Exchange::Upbit, "out_of_scope", 403, "주문 권한"),
        (Exchange::Bithumb, "expired_jwt", 401, "PC 시계"),
    ];
    for (ex, name, status, needle) in cases {
        let server = MockServer::start().await;
        mount(&server, "POST", "/v1/orders/test", err(name, "fixture", status)).await;
        mount(&server, "GET", "/v1/accounts", err(name, "fixture", status)).await;
        let (b, _) = make(ex, &server.uri());
        let (ok, reason) = b.rehearse(10_000.0, None).await;
        assert!(!ok);
        assert!(reason.contains(needle), "{name}: {reason}");
        assert_eq!(reason, explain_domestic_error(ex, name, "fixture"));
        assert!(!reason.contains(name), "문장에는 코드가 없음: {reason}");
        assert_eq!(b.last_error_name(), name);
        assert_eq!(b.last_error_code(), name, "코드는 따로 들고 있음(자세히 보기용)");
        if ex == Exchange::Bithumb {
            assert!(requests(&server, "POST", "/v1/orders/test").await.is_empty());
        }
    }
}

#[tokio::test]
async fn unrecognised_and_unnamed_errors_keep_the_exchanges_words_or_status() {
    let server = MockServer::start().await;
    mount(&server, "POST", "/v1/orders/test", err("brand_new_error", "거래소가 한 말", 400)).await;
    let (b, _) = make(Exchange::Upbit, &server.uri());
    let (ok, reason) = b.rehearse(10_000.0, None).await;
    assert_eq!((ok, reason.as_str()), (false, "업비트에서 주문을 거절했어요: 거래소가 한 말"));
    assert_eq!(b.last_error_code(), "brand_new_error", "코드는 문장 밖에서 따로 들고 있음");

    let server = MockServer::start().await;
    mount(&server, "POST", "/v1/orders/test", ResponseTemplate::new(403).set_body_string("<html>blocked</html>")).await;
    let (b, _) = make(Exchange::Upbit, &server.uri());
    let (ok, reason) = b.rehearse(10_000.0, None).await;
    assert!(!ok);
    assert_eq!(reason, "업비트에서 주문을 거절했어요.");
    assert_eq!(b.last_error_name(), "");
    assert_eq!(b.last_error_code(), "HTTP 403");
}

#[tokio::test]
async fn network_failure_is_a_failure_not_blamed_on_the_key() {
    let (b, _) = make(Exchange::Upbit, "http://127.0.0.1:9");
    let (ok, reason) = b.rehearse(10_000.0, None).await;
    assert!(!ok);
    assert!(reason.starts_with("업비트에 연결하지 못했어요: "), "{reason}");
    assert!(!reason.contains("허용 IP"));
    assert_eq!(b.last_error_name(), "network");
}

#[tokio::test]
async fn bad_notional_fails_without_sending_anything() {
    let server = MockServer::start().await;
    let (b, _) = make(Exchange::Upbit, &server.uri());
    let (ok, reason) = b.rehearse(0.0, None).await;
    assert!(!ok);
    assert_eq!(
        reason,
        "주문 검증 요청을 만들지 못했어요: 시장가 매수에는 0보다 큰 원화 금액이 필요해요. 매크로 내용을 확인하세요."
    );
    assert!(server.received_requests().await.unwrap_or_default().is_empty());
}

// --- 리허설: 빗썸 -----------------------------------------------------------

async fn bithumb(accounts: Value, chance: Value) -> (MockServer, DomesticBroker) {
    let server = MockServer::start().await;
    mount(&server, "GET", "/v1/accounts", ResponseTemplate::new(200).set_body_json(accounts)).await;
    mount(&server, "GET", "/v1/orders/chance", ResponseTemplate::new(200).set_body_json(chance)).await;
    let (b, _) = make(Exchange::Bithumb, &server.uri());
    (server, b)
}

#[tokio::test]
async fn bithumb_never_posts_and_does_not_claim_the_order_permission() {
    let (server, b) = bithumb(json!([{"currency": "KRW", "balance": "250000"}]), chance()).await;
    let (ok, reason) = b.rehearse(10_000.0, None).await;
    assert!(ok, "{reason}");
    assert!(reason.contains("주문 권한") && reason.contains("첫 주문"));
    let all = server.received_requests().await.unwrap();
    assert!(all.iter().all(|r| r.method.as_str() == "GET"));
    let paths: Vec<_> = all.iter().map(|r| r.url.path().to_string()).collect();
    assert_eq!(paths, ["/v1/accounts", "/v1/orders/chance"]);
}

#[tokio::test]
async fn bithumb_checks_the_minimum_amount_itself() {
    let (_s, b) = bithumb(json!([{"currency": "KRW", "balance": "250000"}]), chance()).await;
    let (ok, reason) = b.rehearse(1_000.0, None).await;
    assert!(!ok);
    assert_eq!(reason, "최소 주문 금액보다 적어요. 빗썸 최소 주문 금액은 5000원인데 이번 주문은 1000원이에요. 매크로 투자 금액을 올리세요.");
    assert_eq!(b.last_error_name(), "under_min_total");
}

#[tokio::test]
async fn bithumb_checks_the_krw_balance_including_the_fee() {
    let (_s, b) = bithumb(json!([{"currency": "KRW", "balance": "10004", "locked": "50000"}]), chance()).await;
    let (ok, reason) = b.rehearse(10_000.0, None).await;
    assert!(!ok);
    assert_eq!(b.last_error_name(), "insufficient_funds");
    assert_eq!(
        reason,
        "주문할 원화 잔고가 모자라요. 첫 주문에 수수료까지 10,005원이 필요한데 주문 가능한 원화는 10,004원이에요. \
         빗썸에 원화를 채우거나 매크로 투자 금액을 줄이세요."
    );
    let (_s, b) = bithumb(json!([{"currency": "KRW", "balance": "10005"}]), chance()).await;
    let (ok, reason) = b.rehearse(10_000.0, None).await;
    assert!(ok, "{reason}");
    assert!(reason.contains("원화 잔고"));
}

#[tokio::test]
async fn bithumb_balance_check_uses_the_real_fee_when_given_the_spend() {
    let c = json!({"bid_fee": "0.0004", "ask_fee": "0.0005", "bid": {"min_total": "5000"}});
    let guessed = fee_inclusive(20_000.0, DOMESTIC_BID_FEE_FALLBACK);
    let (_s, b) = bithumb(json!([{"currency": "KRW", "balance": "19970"}]), c.clone()).await;
    let (ok, reason) = b.rehearse(guessed, Some(20_000.0)).await;
    assert!(!ok);
    assert!(reason.contains("20,000원"), "{reason}");
    let (_s, b) = bithumb(json!([{"currency": "KRW", "balance": "20000"}]), c).await;
    assert!(b.rehearse(guessed, Some(20_000.0)).await.0);
}

#[tokio::test]
async fn bithumb_without_a_krw_row_or_minimum() {
    let (_s, b) = bithumb(json!([{"currency": "BTC", "balance": "0.1"}]), chance()).await;
    assert!(!b.rehearse(10_000.0, None).await.0);
    assert_eq!(b.last_error_name(), "insufficient_funds");
    let (_s, b) = bithumb(json!([{"currency": "KRW", "balance": "100"}]), json!({"bid": {}})).await;
    assert!(b.rehearse(1.0, None).await.0);
}

#[tokio::test]
async fn bithumb_missing_market_is_reported() {
    let server = MockServer::start().await;
    mount(
        &server,
        "GET",
        "/v1/accounts",
        ResponseTemplate::new(200).set_body_json(json!([{"currency": "KRW", "balance": "250000"}])),
    )
    .await;
    mount(&server, "GET", "/v1/orders/chance", err("invalid_market", "", 404)).await;
    let (b, _) = make(Exchange::Bithumb, &server.uri());
    let (ok, reason) = b.rehearse(10_000.0, None).await;
    assert!(!ok && reason.contains("마켓"), "{reason}");
}

#[test]
fn thousands_and_plain() {
    assert_eq!(grouped(10005.0, 0), "10,005");
    assert_eq!(grouped(999.0, 0), "999");
    assert_eq!(grouped(1234567.0, 0), "1,234,567");
    assert_eq!(plain_number(5000.0), "5000");
    assert_eq!(plain_number(19992.0), "19992");
    assert_eq!(plain_number(0.00001), "0.00001");
}

#[tokio::test]
async fn a_rate_limit_pauses_public_requests_but_not_order_lookups() {
    let server = MockServer::start().await;
    mount(&server, "GET", "/v1/ticker", ResponseTemplate::new(429)).await;
    let (a, _) = make(Exchange::Upbit, &server.uri());
    let (b, _) = make(Exchange::Upbit, &server.uri());
    assert!(a.price().await.is_err());
    let before = server.received_requests().await.unwrap().len();
    assert!(b.price().await.is_err());
    assert_eq!(server.received_requests().await.unwrap().len(), before, "다른 실행의 시세 요청도 쉰다");
}

// --- 화면 정보: 수수료·잔고·전일 대비 -----------------------------------------

#[test]
fn the_paid_fee_of_an_order_is_kept_in_krw_or_left_unknown() {
    let o = normalize_domestic(&json!({"state": "done", "executed_volume": "0.01", "paid_fee": "500.25",
        "trades": [{"price": "100000000", "volume": "0.01", "funds": "1000000"}]}));
    assert_eq!(o.fee, Some(FeePaid { quote: 500.25, other: vec![] }));
    // 칸이 없는 응답이면 모름(0원으로 보이지 않게).
    assert_eq!(normalize_domestic(&filled()).fee, None);
    assert_eq!(normalize_domestic(&json!({"state": "done", "paid_fee": null})).fee, None);
}

#[tokio::test]
async fn chance_balances_become_the_balance_line() {
    let server = MockServer::start().await;
    let mut body = chance();
    body["bid_account"] = json!({"currency": "KRW", "balance": "1234567.8", "locked": "0"});
    body["ask_account"] = json!({"currency": "BTC", "balance": "0.01", "locked": "0"});
    mount(&server, "GET", "/v1/orders/chance", ResponseTemplate::new(200).set_body_json(body)).await;
    let (b, _) = make(Exchange::Upbit, &server.uri());
    assert_eq!(b.balance_note(), "", "읽기 전에는 비어 있음");
    assert!(b.ensure_ready().await.unwrap());
    assert_eq!(b.balance_note(), "사용 가능 잔고 1,234,567 KRW · 보유 BTC 0.01");
    assert_eq!(requests(&server, "GET", "/v1/orders/chance").await.len(), 1, "잔고 때문에 더 묻지 않음");

    // 코인이 없으면 원화만.
    let server = MockServer::start().await;
    let mut body = chance();
    body["bid_account"] = json!({"currency": "KRW", "balance": "5000"});
    body["ask_account"] = json!({"currency": "BTC", "balance": "0"});
    mount(&server, "GET", "/v1/orders/chance", ResponseTemplate::new(200).set_body_json(body)).await;
    let (b, _) = make(Exchange::Upbit, &server.uri());
    b.ensure_ready().await.unwrap();
    assert_eq!(b.balance_note(), "사용 가능 잔고 5,000 KRW");
}

#[tokio::test]
async fn bithumb_falls_back_to_the_accounts_it_already_read() {
    let (_s, b) = bithumb(json!([{"currency": "KRW", "balance": "250000"}]), chance()).await;
    assert!(b.rehearse(10_000.0, None).await.0);
    assert_eq!(b.balance_note(), "사용 가능 잔고 250,000 KRW");
}

#[tokio::test]
async fn the_ticker_change_rate_rides_along_with_the_price() {
    let server = MockServer::start().await;
    mount(
        &server,
        "GET",
        "/v1/ticker",
        ResponseTemplate::new(200)
            .set_body_json(json!([{"market": "KRW-BTC", "trade_price": 101500000.0, "signed_change_rate": -0.0123}])),
    )
    .await;
    let (b, _) = make(Exchange::Bithumb, &server.uri());
    assert_eq!(b.day_change(), None);
    b.price().await.unwrap();
    assert_eq!(b.day_change(), Some(-0.0123));
    assert_eq!(requests(&server, "GET", "/v1/ticker").await.len(), 1);
}
