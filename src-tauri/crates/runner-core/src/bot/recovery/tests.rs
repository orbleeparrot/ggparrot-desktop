use super::*;
use crate::bot::tests::{FakeBroker, bot_with};
use crate::recovery::RecoveryStore;

fn attach(bot: &mut Bot, path: std::path::PathBuf) -> RecoveryContext {
    let context = RecoveryContext { store: RecoveryStore::new(path), recovery_id: uuid::Uuid::new_v4().to_string() };
    let checkpoint = Checkpoint {
        recovery_id: context.recovery_id.clone(),
        session_id: 31,
        generation: 0,
        macro_: bot.macro_.clone(),
        macro_digest: "digest".into(),
        mode: bot.mode,
        server_base: "http://localhost".into(),
        completed: false,
        runtime: json!({}),
        engine_version: crate::engine::ENGINE_VERSION,
        decides_locally: false,
    };
    bot.recovery_state = Some(RecoveryRuntime { context: context.clone(), checkpoint });
    context
}

#[tokio::test]
async fn storage_failure_prevents_the_first_order_request() {
    let dir = tempfile::tempdir().unwrap();
    let blocker = dir.path().join("not-a-folder");
    std::fs::write(&blocker, "x").unwrap();
    let broker = FakeBroker::new(100.0, Market::Spot, Side::Long);
    let (mut bot, _rx) =
        bot_with(json!({"symbol":"BTCUSDT","exchange":"binance","rule_type":"A"}), broker.clone(), RunMode::Mock);
    attach(&mut bot, blocker.join("recovery.sqlite"));
    assert!(bot.place(OrderSide::Buy, 0.1, false, 0.0).await.is_err());
    assert!(broker.submits().is_empty());
    assert!(!bot.in_position);
}

#[tokio::test]
async fn order_intent_stays_dirty_until_position_engine_and_risk_complete() {
    let dir = tempfile::tempdir().unwrap();
    let broker = FakeBroker::new(100.0, Market::Spot, Side::Long);
    let (mut bot, _rx) =
        bot_with(json!({"symbol":"BTCUSDT","exchange":"binance","rule_type":"A"}), broker.clone(), RunMode::Mock);
    let context = attach(&mut bot, dir.path().join("recovery.sqlite"));
    assert!(bot.place(OrderSide::Buy, 0.1, false, 0.0).await.unwrap());
    let dirty = context.store.load(&context.recovery_id).unwrap().unwrap();
    assert_eq!(dirty.snapshot()["position_qty"], 0.1);
    assert_eq!(dirty.runtime["pending_orders"].as_array().unwrap().len(), 1);
    assert_eq!(dirty.runtime["pending_orders"][0]["client_id"], broker.submits()[0].client_id);
    let mut guard = bot.risk_guard();
    guard.on_entry();
    bot.sync_recovery(&guard, None, true).unwrap();
    let clean = context.store.load(&context.recovery_id).unwrap().unwrap();
    assert!(clean.runtime["pending_orders"].as_array().unwrap().is_empty());
    let state: BotState = serde_json::from_value(clean.runtime["bot"].clone()).unwrap();
    let (mut restored, _) = bot_with(bot.macro_.clone(), broker, RunMode::Mock);
    state.apply(&mut restored);
    assert!(restored.in_position);
    assert_eq!(restored.held_qty, 0.1);
    let mut risk = restored.risk_guard();
    risk.restore_checkpoint(&clean.runtime["risk"]).unwrap();
    assert!(risk.holding());
}

fn local_checkpoint(bot: &Bot) -> Value {
    let mut engine = crate::engine::runner::build(&bot.macro_).unwrap();
    engine.set_clock(now_ms());
    assert!(engine.tick(100.0).is_some());
    json!({"engine":serde_json::to_value(engine).unwrap(),"last_t":1_700_000_000_000i64,
        "capital":100.0,"seq":9,"pending_fills":[],"last_bar":null,"recent":[]})
}

#[tokio::test]
async fn normalized_local_checkpoint_resumes_same_session_and_retries_a_lost_response() {
    use std::sync::atomic::{AtomicUsize, Ordering};
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, Request, ResponseTemplate};
    let exchange = MockServer::start().await;
    let dir = tempfile::tempdir().unwrap();
    let broker = FakeBroker::new(100.0, Market::Spot, Side::Long);
    let macro_ = json!({"symbol":"BTCUSDT","exchange":"binance","rule_type":"A","position_side":"long","leverage":1,
        "params":{"initial_capital":100.0,"take_profit_pct":5.0},"risk":{"cooldown_minutes":10},"candle_interval":"1h"});
    let (mut bot, _) = bot_with(macro_.clone(), broker.clone(), RunMode::Mock);
    let context = attach(&mut bot, dir.path().join("recovery.sqlite"));
    bot.place(OrderSide::Buy, 1.0, false, 0.0).await.unwrap();
    let mut guard = bot.risk_guard();
    guard.on_entry();
    bot.sync_recovery(&guard, None, true).unwrap();
    let mut checkpoint = context.store.load(&context.recovery_id).unwrap().unwrap();
    checkpoint.server_base = exchange.uri();
    checkpoint.decides_locally = true;
    checkpoint.runtime["local"] = local_checkpoint(&bot);
    context.store.save(&checkpoint).unwrap();
    let previews = Arc::new(AtomicUsize::new(0));
    let p = previews.clone();
    let m = macro_.clone();
    Mock::given(method("POST")).and(path("/api/runner/recovery")).respond_with(move |_:&Request| {
        let later=p.fetch_add(1,Ordering::SeqCst)>0;
        ResponseTemplate::new(200).set_body_json(json!({"session_id":31,"generation":if later{1}else{0},"macro":m,"macro_digest":"digest","mode":"mock","decides":"local","recoverable":!later,"reason":"online"}))
    }).mount(&exchange).await;
    let resumes = Arc::new(AtomicUsize::new(0));
    let r = resumes.clone();
    let m = macro_.clone();
    Mock::given(method("POST")).and(path("/api/runner/resume")).respond_with(move |_:&Request| {
        if r.fetch_add(1,Ordering::SeqCst)==0 {ResponseTemplate::new(503).set_body_json(json!({"detail":"response lost"}))}
        else {ResponseTemplate::new(200).set_body_json(json!({"session_id":31,"generation":1,"macro":m,"macro_digest":"digest","mode":"mock","decides":"local"}))}
    }).mount(&exchange).await;
    let outbox = Arc::new(crate::outbox::EventOutbox::open(Some(dir.path().join("events.sqlite"))));
    let client = || ServerClient::new("fixture-member", &exchange.uri(), outbox.clone(), reqwest::Client::new());
    assert!(
        prepare_resume(checkpoint, crate::bot::tests::creds(), reqwest::Client::new(), client(), context.clone())
            .await
            .is_err()
    );
    let retry = context.store.load(&context.recovery_id).unwrap().unwrap();
    let nonce = retry.runtime["resume_attempt"]["resume_id"].as_str().unwrap().to_string();
    assert!(uuid::Uuid::parse_str(&nonce).is_ok());
    let prepared = prepare_resume(retry, crate::bot::tests::creds(), reqwest::Client::new(), client(), context.clone())
        .await
        .unwrap();
    assert_eq!(prepared.checkpoint().session_id, 31);
    assert_eq!(prepared.checkpoint().generation, 1);
    assert_eq!(prepared.bot.held_qty, 1.0);
    assert!(prepared.bot.in_position);
    assert!(prepared.checkpoint().runtime["resume_attempt"].is_null());
    let restored = prepared.bot.restore_local_engine(&prepared.checkpoint().runtime["local"]).unwrap();
    assert_eq!(restored.checkpoint().unwrap()["seq"], 9);
    let requests = exchange.received_requests().await.unwrap();
    assert!(
        requests
            .iter()
            .all(|req| req.method == "POST" && matches!(req.url.path(), "/api/runner/recovery" | "/api/runner/resume"))
    );
    let resume_payloads: Vec<Value> = requests
        .iter()
        .filter(|req| req.url.path() == "/api/runner/resume")
        .map(|req| req.body_json().unwrap())
        .collect();
    assert_eq!(resume_payloads.len(), 2);
    assert_eq!(resume_payloads[0], resume_payloads[1]);
    assert_eq!(resume_payloads[1]["resume_id"], nonce);
    assert_eq!(resume_payloads[1]["snapshot"]["position_qty"], 1.0);
    assert_eq!(broker.submits().len(), 1);
}

#[tokio::test]
async fn crash_inside_an_order_never_calls_resume_or_resends_even_in_mock_mode() {
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};
    let server = MockServer::start().await;
    let dir = tempfile::tempdir().unwrap();
    let macro_ = json!({"symbol":"BTCUSDT","exchange":"binance","rule_type":"A"});
    let broker = FakeBroker::new(100.0, Market::Spot, Side::Long);
    let (mut bot, _) = bot_with(macro_.clone(), broker.clone(), RunMode::Mock);
    let context = attach(&mut bot, dir.path().join("recovery.sqlite"));
    bot.place(OrderSide::Buy, 0.1, false, 0.0).await.unwrap();
    let mut checkpoint = context.store.load(&context.recovery_id).unwrap().unwrap();
    checkpoint.server_base = server.uri();
    context.store.save(&checkpoint).unwrap();
    Mock::given(method("POST")).and(path("/api/runner/recovery")).respond_with(ResponseTemplate::new(200).set_body_json(json!({"generation":0,"macro":macro_,"macro_digest":"digest","mode":"mock","decides":"server","recoverable":true}))).mount(&server).await;
    let client = ServerClient::new(
        "fixture-member",
        &server.uri(),
        Arc::new(crate::outbox::EventOutbox::open(Some(dir.path().join("events.sqlite")))),
        reqwest::Client::new(),
    );
    let err = prepare_resume(checkpoint, crate::bot::tests::creds(), reqwest::Client::new(), client, context.clone())
        .await
        .err()
        .unwrap();
    assert!(err.contains("주문 처리 도중"));
    assert_eq!(server.received_requests().await.unwrap().len(), 1);
    assert_eq!(broker.submits().len(), 1);
    assert!(
        !context.store.load(&context.recovery_id).unwrap().unwrap().runtime["pending_orders"]
            .as_array()
            .unwrap()
            .is_empty()
    );
}

#[tokio::test]
async fn recovery_rejects_same_futures_quantity_with_changed_average_or_leverage_and_refreshes_rules() {
    use crate::brokers::{BinanceBroker, BinanceUrls};
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};
    for (average, leverage, expected_error) in
        [(120.0, 2, Some("평단")), (100.0, 3, Some("레버리지")), (100.00000001, 2, None)]
    {
        let server = MockServer::start().await;
        let dir = tempfile::tempdir().unwrap();
        let fake = FakeBroker::new(100.0, Market::Futures, Side::Long);
        let (mut bot, _) = bot_with(
            json!({"symbol":"BTCUSDT","exchange":"binance","rule_type":"A","position_side":"long","leverage":2}),
            fake,
            RunMode::Live,
        );
        bot.held_qty = 0.1;
        bot.in_position = true;
        bot.entry_price = 100.0;
        bot.step = 0.001;
        let context = attach(&mut bot, dir.path().join("recovery.sqlite"));
        bot.persist_recovery_state().unwrap();
        let mut checkpoint = context.store.load(&context.recovery_id).unwrap().unwrap();
        checkpoint.runtime["account_baseline"] = json!(0.0);
        for endpoint in ["/fapi/v1/openOrders", "/fapi/v1/openAlgoOrders"] {
            Mock::given(method("GET"))
                .and(path(endpoint))
                .respond_with(ResponseTemplate::new(200).set_body_json(json!([])))
                .mount(&server)
                .await;
        }
        Mock::given(method("GET")).and(path("/fapi/v1/exchangeInfo")).respond_with(ResponseTemplate::new(200).set_body_json(json!({"symbols":[{"symbol":"BTCUSDT","filters":[{"filterType":"LOT_SIZE","stepSize":"0.01"},{"filterType":"MIN_NOTIONAL","notional":"5"}]}]}))).mount(&server).await;
        Mock::given(method("GET")).and(path("/fapi/v2/positionRisk")).respond_with(ResponseTemplate::new(200).set_body_json(json!([{"symbol":"BTCUSDT","positionSide":"BOTH","positionAmt":"0.1","entryPrice":average.to_string(),"leverage":leverage.to_string()}]))).mount(&server).await;
        let broker = BinanceBroker::new(
            "fixture-access",
            "fixture-secret",
            Market::Futures,
            "BTCUSDT",
            Side::Long,
            true,
            2,
            crate::brokers::null_log(),
            reqwest::Client::new(),
            Some(BinanceUrls { spot: server.uri(), futures: server.uri() }),
        );
        let result = super::reconcile::reconcile_position(&mut bot, &checkpoint, &broker).await;
        if let Some(expected_error) = expected_error {
            assert!(result.unwrap_err().contains(expected_error));
        } else {
            result.unwrap();
        }
        assert_eq!(bot.step, 0.01);
        assert!(server.received_requests().await.unwrap().iter().all(|request| request.method == "GET"));
    }
}
