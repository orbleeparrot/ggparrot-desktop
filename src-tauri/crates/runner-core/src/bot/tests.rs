//! 봇 루프 테스트 — 서버 명령 처리·주문 실행·종료 경로의 핵심 사례.

use super::*;
use crate::brokers::{Order, OrderRules};
use crate::money::qty_text;
use async_trait::async_trait;
use std::sync::atomic::{AtomicUsize, Ordering};

#[derive(Default)]
struct FakeState {
    price: f64,
    submits: Vec<SubmitRequest>,
    /// 다음 submit 결과를 덮어씀(없으면 현재가로 전량 체결).
    next: VecDeque<Order>,
    bid_fee: Option<f64>,
    step: f64,
    /// 현재가 요청이 실패함(거래소에 닿지 않음).
    price_down: bool,
}

struct FakeBroker {
    st: Mutex<FakeState>,
    market: Market,
    side: Side,
    rehearsals: AtomicUsize,
}

impl FakeBroker {
    fn new(price: f64, market: Market, side: Side) -> Arc<Self> {
        Arc::new(FakeBroker {
            st: Mutex::new(FakeState { price, ..Default::default() }),
            market,
            side,
            rehearsals: AtomicUsize::new(0),
        })
    }
    fn push(&self, o: Order) {
        self.st.lock().unwrap().next.push_back(o);
    }
    fn submits(&self) -> Vec<SubmitRequest> {
        self.st.lock().unwrap().submits.clone()
    }
}

fn filled(qty: f64, price: f64) -> Order {
    Order {
        status: "FILLED".into(),
        executed_qty: qty,
        avg_price: price,
        acquired_qty: qty,
        fees_known: true,
        order_id: Some("1".into()),
        raw_status: "FILLED".into(),
        fee: None,
    }
}

#[async_trait]
impl Broker for FakeBroker {
    fn market(&self) -> Market {
        self.market
    }
    fn side(&self) -> Side {
        self.side
    }
    fn symbol(&self) -> String {
        "BTCUSDT".into()
    }
    fn fee_from_base_asset(&self) -> bool {
        false
    }
    fn bid_fee(&self) -> Option<f64> {
        self.st.lock().unwrap().bid_fee
    }
    fn last_error_name(&self) -> String {
        String::new()
    }
    fn set_log(&self, _log: LogFn) {}
    async fn check_account(&self) -> Result<(), BrokerError> {
        Ok(())
    }
    async fn ensure_ready(&self) -> Result<bool, BrokerError> {
        Ok(true)
    }
    async fn load_market(&self) -> Result<bool, BrokerError> {
        Ok(true)
    }
    async fn order_rules(&self) -> Result<OrderRules, BrokerError> {
        Ok(OrderRules { step: self.st.lock().unwrap().step, min_notional: 0.0 })
    }
    async fn rehearse(&self, _notional: f64, _spend: Option<f64>) -> (bool, String) {
        self.rehearsals.fetch_add(1, Ordering::SeqCst);
        (true, "가짜 확인 통과".into())
    }
    async fn price(&self) -> Result<f64, BrokerError> {
        let st = self.st.lock().unwrap();
        if st.price_down {
            return Err(BrokerError::Network("ticker timed out".into()));
        }
        Ok(st.price)
    }
    async fn submit(&self, req: SubmitRequest) -> Result<Order, BrokerError> {
        let mut st = self.st.lock().unwrap();
        st.submits.push(req.clone());
        if let Some(o) = st.next.pop_front() {
            return Ok(o);
        }
        let qty = req.base_qty.unwrap_or_else(|| req.notional.unwrap_or(0.0) / st.price);
        Ok(filled(qty, st.price))
    }
}

fn creds() -> Credentials {
    let mut c = Credentials { member_key: "ggp_member".into(), ..Default::default() };
    for ex in ["binance", "upbit"] {
        c.exchanges.insert(ex.into(), KeyPair { api_key: "k".repeat(24), api_secret: "s".repeat(24) });
    }
    c
}

fn bot_with(m: Value, broker: Arc<FakeBroker>, mode: RunMode) -> (Bot, mpsc::UnboundedReceiver<BotEvent>) {
    let (tx, rx) = mpsc::unbounded_channel();
    let mut bot = Bot::new(m, creds(), mode, None, tx, reqwest::Client::new());
    bot.broker = Some(broker as Arc<dyn Broker>);
    (bot, rx)
}

fn logs(rx: &mut mpsc::UnboundedReceiver<BotEvent>) -> Vec<String> {
    let mut out = vec![];
    while let Ok(e) = rx.try_recv() {
        if let BotEvent::Log(l) = e {
            out.push(l);
        }
    }
    out
}

#[tokio::test]
async fn buy_command_spends_capital_fraction_with_cap() {
    let fb = FakeBroker::new(100.0, Market::Spot, Side::Long);
    let (mut bot, _rx) =
        bot_with(json!({"symbol": "BTCUSDT", "params": {"initial_capital": 1000}}), fb.clone(), RunMode::Live);
    let ack = bot
        .execute_command(&json!({"id": 1, "action": "buy", "notional_frac": 0.05, "reason": "진입"}), 100.0)
        .await
        .unwrap();
    assert_eq!(ack["ok"], true);
    assert_eq!(fb.submits()[0].base_qty, Some(0.5));
    assert!(bot.in_position);
    assert_eq!(bot.entry_price, 100.0);
    // 상한 100 USDT
    let ack = bot.execute_command(&json!({"id": 2, "action": "buy", "notional_frac": 1.0}), 100.0).await.unwrap();
    assert_eq!(ack["executed_qty"], 1.0);
}

#[tokio::test]
async fn duplicate_command_is_not_executed_twice() {
    let fb = FakeBroker::new(100.0, Market::Spot, Side::Long);
    let (mut bot, _rx) = bot_with(json!({"symbol": "BTCUSDT"}), fb.clone(), RunMode::Live);
    let cmd = json!({"id": 7, "action": "buy", "notional_frac": 0.5});
    bot.execute_command(&cmd, 100.0).await.unwrap();
    let again = bot.execute_command(&cmd, 100.0).await.unwrap();
    assert_eq!(again["ok"], true);
    assert_eq!(again["outcome"], "duplicate_command_no_reexecution");
    assert_eq!(fb.submits().len(), 1);
}

#[tokio::test]
async fn exit_when_flat_is_noop_ack_and_recorded() {
    let fb = FakeBroker::new(100.0, Market::Spot, Side::Long);
    let (mut bot, mut rx) = bot_with(json!({"symbol": "BTCUSDT"}), fb.clone(), RunMode::Live);
    let ack = bot.execute_command(&json!({"id": 3, "action": "sell", "reason": "청산"}), 100.0).await.unwrap();
    assert_eq!(ack["ok"], true);
    assert_eq!(ack["executed_qty"], 0.0);
    assert_eq!(ack["outcome"], "no_position_no_order");
    assert!(fb.submits().is_empty());
    assert!(logs(&mut rx).iter().any(|l| l == "[신호] 청산 → 보유 없음, 건너뜀"));
    let again = bot.execute_command(&json!({"id": 3, "action": "sell"}), 100.0).await.unwrap();
    assert_eq!(again["outcome"], "duplicate_command_no_reexecution");
}

#[tokio::test]
async fn failed_order_acks_error_and_can_be_retried() {
    let fb = FakeBroker::new(100.0, Market::Spot, Side::Long);
    let (mut bot, _rx) = bot_with(json!({"symbol": "BTCUSDT"}), fb.clone(), RunMode::Live);
    fb.push(Order {
        status: "REJECTED".into(),
        executed_qty: 0.0,
        avg_price: 0.0,
        acquired_qty: 0.0,
        fees_known: true,
        order_id: None,
        raw_status: "REJECTED".into(),
        fee: None,
    });
    let cmd = json!({"id": 9, "action": "buy", "notional_frac": 0.5});
    let ack = bot.execute_command(&cmd, 100.0).await.unwrap();
    assert_eq!(ack["ok"], false);
    assert!(ack["error"].as_str().unwrap().contains("거절했어요(REJECTED)"), "거절 이유를 그대로 알린다");
    assert!(!bot.in_position);
    let ack = bot.execute_command(&cmd, 100.0).await.unwrap();
    assert_eq!(ack["ok"], true, "실패한 id 는 기록하지 않으므로 다시 실행한다");
}

#[tokio::test]
async fn uncertain_order_escapes_and_blocks_further_orders() {
    let fb = FakeBroker::new(100.0, Market::Spot, Side::Long);
    let (mut bot, _rx) = bot_with(json!({"symbol": "BTCUSDT"}), fb.clone(), RunMode::Live);
    fb.push(Order::unknown());
    let err = bot.execute_command(&json!({"id": 1, "action": "buy", "notional_frac": 0.5}), 100.0).await;
    assert!(err.is_err());
    assert!(bot.position_uncertain);
    assert!(bot.in_position, "모르는 주문은 평평한 상태로 보고하지 않는다");
    let ack = bot.execute_command(&json!({"id": 2, "action": "buy", "notional_frac": 0.5}), 100.0).await.unwrap();
    assert_eq!(ack["ok"], false);
    assert_eq!(fb.submits().len(), 1);
}

#[tokio::test]
async fn add_to_position_averages_entry_and_partial_exit_keeps_rest() {
    let fb = FakeBroker::new(100.0, Market::Spot, Side::Long);
    let (mut bot, _rx) =
        bot_with(json!({"symbol": "BTCUSDT", "params": {"initial_capital": 100}}), fb.clone(), RunMode::Live);
    bot.execute_command(&json!({"id": 1, "action": "buy", "notional_frac": 0.5}), 100.0).await.unwrap();
    fb.st.lock().unwrap().price = 50.0;
    bot.execute_command(&json!({"id": 2, "action": "buy", "notional_frac": 0.5}), 50.0).await.unwrap();
    assert!((bot.held_qty - 1.5).abs() < 1e-9);
    assert!((bot.entry_price - 66.666_666_666).abs() < 1e-6);
    let ack = bot.execute_command(&json!({"id": 3, "action": "sell", "qty_frac": 0.5}), 50.0).await.unwrap();
    assert_eq!(ack["ok"], true);
    assert!(bot.in_position);
    assert!((bot.held_qty - 0.75).abs() < 1e-9);
}

#[tokio::test]
async fn wrong_direction_command_is_rejected_without_order() {
    let fb = FakeBroker::new(100.0, Market::Futures, Side::Short);
    let (mut bot, _rx) =
        bot_with(json!({"symbol": "BTCUSDT", "position_side": "short", "leverage": 2}), fb.clone(), RunMode::Live);
    let ack = bot.execute_command(&json!({"id": 1, "action": "buy", "notional_frac": 0.5}), 100.0).await.unwrap();
    assert_eq!(ack["ok"], false);
    assert_eq!(ack["error"], "매크로 방향과 맞지 않는 명령이에요: buy");
    assert!(fb.submits().is_empty());
    let ack = bot.execute_command(&json!({"id": 2, "action": "explode"}), 100.0).await.unwrap();
    assert_eq!(ack["error"], "알 수 없는 명령 explode");
}

#[tokio::test]
async fn domestic_entry_uses_fee_inclusive_krw_amount() {
    let fb = FakeBroker::new(15_000_000.0, Market::Spot, Side::Long);
    fb.st.lock().unwrap().bid_fee = Some(0.0005);
    let (mut bot, _rx) = bot_with(
        json!({"symbol": "KRW-BTC", "exchange": "upbit", "params": {"initial_capital": 1_000_000}}),
        fb.clone(),
        RunMode::Live,
    );
    bot.step = 0.0;
    let ack =
        bot.execute_command(&json!({"id": 1, "action": "buy", "notional_frac": 1.0}), 15_000_000.0).await.unwrap();
    assert_eq!(ack["ok"], true);
    let req = &fb.submits()[0];
    assert_eq!(req.base_qty, None, "국내 매수는 금액 주문");
    assert!((req.notional.unwrap() - 150_000.0 / 1.0005).abs() < 1e-6);
}

#[tokio::test]
async fn local_stop_loss_only_when_in_position() {
    let fb = FakeBroker::new(100.0, Market::Spot, Side::Long);
    let (mut bot, _rx) =
        bot_with(json!({"symbol": "BTCUSDT", "risk": {"stop_loss_pct": 5}}), fb.clone(), RunMode::Live);
    assert!(!bot.local_stop_loss(10.0));
    bot.execute_command(&json!({"id": 1, "action": "buy", "notional_frac": 0.5}), 100.0).await.unwrap();
    assert!(!bot.local_stop_loss(96.0));
    assert!(bot.local_stop_loss(95.0));
}

#[tokio::test]
async fn close_records_realized_pnl_and_clears_quantity() {
    let fb = FakeBroker::new(100.0, Market::Spot, Side::Long);
    let (mut bot, _rx) = bot_with(json!({"symbol": "BTCUSDT"}), fb.clone(), RunMode::Live);
    bot.execute_command(&json!({"id": 1, "action": "buy", "notional_frac": 0.5}), 100.0).await.unwrap();
    fb.st.lock().unwrap().price = 110.0;
    assert!(bot.close_position().await.unwrap());
    assert!(!bot.in_position);
    assert_eq!(bot.held_qty, 0.0);
    assert!((bot.realized - 5.0).abs() < 1e-9);
    let snap = bot.snapshot();
    assert_eq!(snap["position_qty"], 0.0);
    assert_eq!(snap["realized_pnl"], 5.0);
}

#[tokio::test]
async fn mock_mode_runs_without_keys_but_live_does_not() {
    let fb = FakeBroker::new(100.0, Market::Spot, Side::Long);
    let (tx, mut rx) = mpsc::unbounded_channel();
    let mut bot =
        Bot::new(json!({"symbol": "BTCUSDT"}), Credentials::default(), RunMode::Live, None, tx, reqwest::Client::new());
    bot.broker_override = Some(fb.clone());
    assert!(!bot.connect());
    assert!(logs(&mut rx).iter().any(|l| l.contains("바이낸스 거래소 키가 필요해요")));

    let (tx, mut rx) = mpsc::unbounded_channel();
    let mut bot = Bot::new(
        json!({"symbol": "KRW-BTC", "exchange": "upbit", "leverage": 3}),
        creds(),
        RunMode::Live,
        None,
        tx,
        reqwest::Client::new(),
    );
    bot.broker_override = Some(fb);
    assert!(!bot.connect());
    assert!(logs(&mut rx).iter().any(|l| l.contains("원화 현물(롱 · 1배)만 돼요")));
}

#[tokio::test]
async fn full_loop_executes_server_commands_and_stops_on_remote_request() {
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, Request, ResponseTemplate};

    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/api/runner/start"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"session_id": 5})))
        .mount(&server)
        .await;
    let beats = Arc::new(AtomicUsize::new(0));
    let b2 = beats.clone();
    Mock::given(method("POST"))
        .and(path("/api/runner/heartbeat"))
        .respond_with(move |req: &Request| {
            let body: Value = serde_json::from_slice(&req.body).unwrap();
            match b2.fetch_add(1, Ordering::SeqCst) {
                0 => ResponseTemplate::new(200).set_body_json(json!({"action": "continue", "commands": [
                    {"id": 11, "seq": 1, "action": "buy", "notional_frac": 0.5, "reason": "골든크로스"}]})),
                1 => {
                    assert_eq!(body["acks"][0]["command_id"], 11);
                    assert_eq!(body["acks"][0]["ok"], true);
                    assert_eq!(body["in_position"], true);
                    ResponseTemplate::new(200).set_body_json(json!({"action": "stop_only"}))
                }
                _ => ResponseTemplate::new(200).set_body_json(json!({"action": "continue"})),
            }
        })
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(path("/api/runner/stopped"))
        .respond_with(|req: &Request| {
            let body: Value = serde_json::from_slice(&req.body).unwrap();
            assert_eq!(body["status"], "stopped");
            assert_eq!(body["note"], "매크로만 종료 — 포지션 유지");
            ResponseTemplate::new(200).set_body_json(json!({}))
        })
        .expect(1)
        .mount(&server)
        .await;

    let dir = tempfile::tempdir().unwrap();
    let outbox = Arc::new(crate::outbox::EventOutbox::open(Some(dir.path().join("o.sqlite3"))));
    let client = ServerClient::new("ggp_member", &server.uri(), outbox, reqwest::Client::new());
    client.start(&json!({})).await.unwrap();

    let fb = FakeBroker::new(100.0, Market::Spot, Side::Long);
    let (tx, mut rx) = mpsc::unbounded_channel();
    let mut bot =
        Bot::new(json!({"symbol": "BTCUSDT"}), creds(), RunMode::Live, Some(client), tx, reqwest::Client::new());
    bot.broker_override = Some(fb.clone());
    bot.poll = Duration::from_millis(10);
    tokio::time::timeout(Duration::from_secs(10), bot.run()).await.expect("loop ends");

    let mut finished = None;
    let mut lines = vec![];
    while let Ok(e) = rx.try_recv() {
        match e {
            BotEvent::Finished { status, note } => finished = Some((status, note)),
            BotEvent::Log(l) => lines.push(l),
            _ => {}
        }
    }
    assert_eq!(finished, Some(("stopped".into(), "매크로만 종료 — 포지션 유지".into())));
    assert_eq!(fb.submits().len(), 1);
    assert_eq!(fb.rehearsals.load(Ordering::SeqCst), 1);
    assert!(lines.iter().any(|l| l == "원격 종료 명령 수신: stop_only"), "{lines:?}");
    assert!(lines.iter().any(|l| l.starts_with("[신호] 골든크로스 → BUY 0.5 BTCUSDT @ 100.0")), "{lines:?}");
}

fn activities(rx: &mut mpsc::UnboundedReceiver<BotEvent>) -> Vec<ActivityEvent> {
    let mut out = vec![];
    while let Ok(e) = rx.try_recv() {
        if let BotEvent::Activity(a) = e {
            out.push(a);
        }
    }
    out
}

#[tokio::test]
async fn activity_reads_like_a_person_wrote_it() {
    let fb = FakeBroker::new(81806.0, Market::Spot, Side::Long);
    let (mut bot, mut rx) = bot_with(json!({"symbol": "BTCUSDT"}), fb.clone(), RunMode::Mock);
    bot.execute_command(
        &json!({"id": 1, "action": "buy", "notional_frac": 0.5, "reason": "RSI 30 이탈 반등"}),
        81806.0,
    )
    .await
    .unwrap();
    fb.st.lock().unwrap().price = 81831.58;
    bot.execute_command(&json!({"id": 2, "action": "sell", "reason": "RSI 70 돌파"}), 81831.58).await.unwrap();
    let acts = activities(&mut rx);
    let kinds: Vec<&str> = acts.iter().map(|a| a.kind).collect();
    assert_eq!(kinds, vec!["signal", "buy", "signal", "sell"], "{acts:?}");
    assert_eq!(acts[0].title, "매수 신호");
    assert_eq!(acts[0].detail, "RSI 30 이탈 반등");
    let qty = qty_text(50.0 / 81806.0);
    assert_eq!(acts[1].title, format!("BTC {qty}개를 81,806 USDT에 샀어요"));
    assert_eq!(acts[1].detail, "모의 체결 · 실제 주문은 보내지 않았어요");
    assert!(acts[1].raw.contains("BUY 체결"));
    assert!(acts[3].title.ends_with("개를 81,831.58 USDT에 팔았어요"), "{}", acts[3].title);
    assert!(acts[3].detail.starts_with("손익 +"), "{}", acts[3].detail);
    assert_eq!(acts[3].tone, "ok");
}

#[tokio::test]
async fn failures_and_blocks_become_activity() {
    let fb = FakeBroker::new(100.0, Market::Spot, Side::Long);
    let (mut bot, mut rx) = bot_with(json!({"symbol": "BTCUSDT"}), fb.clone(), RunMode::Live);
    fb.push(Order::unknown());
    let _ = bot.execute_command(&json!({"id": 1, "action": "buy", "notional_frac": 0.5}), 100.0).await;
    let acts = activities(&mut rx);
    assert!(acts.iter().any(|a| a.kind == "error" && a.title == "주문 결과를 확인하지 못했어요"), "{acts:?}");

    let (mut bot, mut rx) = bot_with(json!({"symbol": "BTCUSDT"}), fb.clone(), RunMode::Live);
    bot.execute_command(&json!({"id": 5, "action": "sell", "reason": "익절"}), 100.0).await.unwrap();
    let acts = activities(&mut rx);
    assert_eq!(acts.last().unwrap().title, "보유한 게 없어 정리 신호를 건너뛰었어요");
}

#[tokio::test]
async fn two_bots_run_side_by_side_with_their_own_sessions() {
    use std::collections::HashMap;
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, Request, ResponseTemplate};

    let server = MockServer::start().await;
    let next_id = Arc::new(AtomicUsize::new(100));
    let ni = next_id.clone();
    Mock::given(method("POST"))
        .and(path("/api/runner/start"))
        .respond_with(move |_: &Request| {
            ResponseTemplate::new(200).set_body_json(json!({"session_id": ni.fetch_add(1, Ordering::SeqCst)}))
        })
        .mount(&server)
        .await;
    // 세션 100은 두 번째 heartbeat에 매수 후 stop_only, 세션 101은 계속 돎(로컬 종료로 끝냄).
    let beats: Arc<Mutex<HashMap<i64, usize>>> = Arc::new(Mutex::new(HashMap::new()));
    let b2 = beats.clone();
    Mock::given(method("POST"))
        .and(path("/api/runner/heartbeat"))
        .respond_with(move |req: &Request| {
            let body: Value = serde_json::from_slice(&req.body).unwrap();
            let sid = body["session_id"].as_i64().unwrap();
            let n = {
                let mut m = b2.lock().unwrap();
                let e = m.entry(sid).or_insert(0);
                *e += 1;
                *e
            };
            let reply = match (sid, n) {
                (100, 1) => {
                    json!({"action": "continue", "commands": [{"id": 1, "action": "buy", "notional_frac": 0.5}]})
                }
                (100, _) => json!({"action": "stop_only"}),
                _ => json!({"action": "continue"}),
            };
            ResponseTemplate::new(200).set_body_json(reply)
        })
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(path("/api/runner/stopped"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({})))
        .expect(2)
        .mount(&server)
        .await;

    let dir = tempfile::tempdir().unwrap();
    let outbox = Arc::new(crate::outbox::EventOutbox::open(Some(dir.path().join("o.sqlite3"))));
    let mut handles = vec![];
    let mut rxs = vec![];
    for sym in ["BTCUSDT", "ETHUSDT"] {
        let client = ServerClient::new("ggp_member", &server.uri(), outbox.clone(), reqwest::Client::new());
        client.start(&json!({})).await.unwrap();
        let fb = FakeBroker::new(100.0, Market::Spot, Side::Long);
        let (tx, rx) = mpsc::unbounded_channel();
        let mut bot =
            Bot::new(json!({"symbol": sym}), creds(), RunMode::Live, Some(client), tx, reqwest::Client::new());
        bot.broker_override = Some(fb);
        bot.poll = Duration::from_millis(10);
        let stop = bot.command.clone();
        let wake = bot.wake.clone();
        handles.push((tokio::spawn(bot.run()), stop, wake));
        rxs.push(rx);
    }
    // 첫 번째는 서버가 끝냄
    tokio::time::timeout(Duration::from_secs(10), &mut handles[0].0).await.unwrap().unwrap();
    assert!(!handles[1].0.is_finished(), "두 번째는 계속 돈다");
    // 두 번째는 로컬 종료
    *handles[1].1.lock().unwrap() = Some(StopMode::StopOnly);
    handles[1].2.notify_one();
    tokio::time::timeout(Duration::from_secs(10), &mut handles[1].0).await.unwrap().unwrap();

    let finished = |rx: &mut mpsc::UnboundedReceiver<BotEvent>| {
        let mut f = None;
        let mut buys = 0;
        while let Ok(e) = rx.try_recv() {
            match e {
                BotEvent::Finished { status, note } => f = Some((status, note)),
                BotEvent::Activity(a) if a.kind == "buy" => buys += 1,
                _ => {}
            }
        }
        (f, buys)
    };
    let (f0, buys0) = finished(&mut rxs[0]);
    let (f1, buys1) = finished(&mut rxs[1]);
    assert_eq!(f0.unwrap().1, "매크로만 종료 — 포지션 유지");
    assert_eq!(buys0, 1);
    assert_eq!(f1.unwrap().1, "포지션 없이 종료");
    assert_eq!(buys1, 0, "다른 세션의 명령을 받지 않는다");
}

#[tokio::test]
async fn a_partially_filled_then_expired_order_still_reports_the_fill() {
    let fb = FakeBroker::new(100.0, Market::Spot, Side::Long);
    let (mut bot, _rx) = bot_with(json!({"symbol": "BTCUSDT"}), fb.clone(), RunMode::Live);
    fb.push(Order {
        status: "CANCELED".into(),
        executed_qty: 0.2,
        avg_price: 100.0,
        acquired_qty: 0.2,
        fees_known: true,
        order_id: Some("1".into()),
        raw_status: "EXPIRED".into(),
        fee: None,
    });
    let ack = bot.execute_command(&json!({"id": 21, "action": "buy", "notional_frac": 0.5}), 100.0).await.unwrap();
    assert_eq!(ack["ok"], false);
    assert_eq!(ack["outcome"], "partially_executed");
    assert_eq!(ack["executed_qty"], 0.2, "체결된 만큼은 서버에 알린다");
    assert_eq!(ack["fill_price"], 100.0);
}

fn events(rx: &mut mpsc::UnboundedReceiver<BotEvent>) -> Vec<BotEvent> {
    let mut out = vec![];
    while let Ok(e) = rx.try_recv() {
        out.push(e);
    }
    out
}

fn acts_of(evs: &[BotEvent]) -> Vec<ActivityEvent> {
    evs.iter()
        .filter_map(|e| match e {
            BotEvent::Activity(a) => Some(a.clone()),
            _ => None,
        })
        .collect()
}

#[tokio::test]
async fn price_outage_reaches_the_screen_and_escalates_while_holding() {
    let fb = FakeBroker::new(100.0, Market::Spot, Side::Long);
    let (mut bot, mut rx) = bot_with(json!({"symbol": "BTCUSDT"}), fb.clone(), RunMode::Mock);
    bot.in_position = true;
    bot.held_qty = 1.0;
    bot.entry_price = 100.0;
    fb.st.lock().unwrap().price_down = true;
    let mut guard = RiskGuard::new(&json!({}), 1000.0, 100.0);

    assert!(bot.fetch_price(fb.as_ref(), &mut guard).await.is_none());
    let evs = events(&mut rx);
    // 화면이 '실행 중·정상'으로 남지 않게 상태와 연결 상태를 함께 보냄.
    assert!(evs.iter().any(|e| matches!(e, BotEvent::Status(_))), "{evs:?}");
    let health = evs.iter().find_map(|e| match e {
        BotEvent::Health(h) => Some(h.clone()),
        _ => None,
    });
    assert!(health.is_some_and(|h| h.price_stale_since > 0), "시세 끊김 시각이 화면에 간다");
    let acts = acts_of(&evs);
    assert_eq!(acts[0].title, "현재가를 받지 못하고 있어요");
    assert!(acts[0].detail.contains("손절"), "포지션이 있으면 손절을 못 본다고 말한다: {}", acts[0].detail);

    // 1분이 지나도록 못 받으면 한 번 더 크게(danger) 알림 — 한 번만.
    bot.price_fail_since = unix_ms() - PRICE_STALE_ALERT_MS - 1;
    bot.fetch_price(fb.as_ref(), &mut guard).await;
    bot.fetch_price(fb.as_ref(), &mut guard).await;
    let danger: Vec<_> = acts_of(&events(&mut rx)).into_iter().filter(|a| a.tone == "danger").collect();
    assert_eq!(danger.len(), 1, "{danger:?}");
    assert_eq!(danger[0].title, "현재가를 1분 넘게 받지 못하고 있어요");

    // 돌아오면 알리고 끊김 시각을 지움.
    fb.st.lock().unwrap().price_down = false;
    assert_eq!(bot.fetch_price(fb.as_ref(), &mut guard).await, Some(100.0));
    assert_eq!(bot.price_fail_since, 0);
    assert!(acts_of(&events(&mut rx)).iter().any(|a| a.title == "현재가를 다시 받고 있어요"));
}

#[tokio::test]
async fn entries_wait_while_a_stop_loss_close_keeps_failing() {
    let fb = FakeBroker::new(100.0, Market::Spot, Side::Long);
    let (mut bot, mut rx) = bot_with(json!({"symbol": "BTCUSDT"}), fb.clone(), RunMode::Live);
    bot.in_position = true;
    bot.held_qty = 1.0;
    bot.entry_price = 100.0;
    bot.close_failures = 1;
    let mut guard = RiskGuard::new(&json!({}), 1000.0, 100.0);
    let reply = crate::server::HeartbeatReply {
        action: "continue".into(),
        commands: vec![json!({"id": 7, "action": "buy", "notional_frac": 0.5})],
        ..Default::default()
    };
    bot.apply_reply(&reply, &mut guard, 100.0).await.unwrap();
    assert!(fb.submits().is_empty(), "정리하지 못한 포지션이 있으면 물타기 주문을 내지 않는다");
    assert_eq!(bot.pending_acks[0]["ok"], false);
    let acts = acts_of(&events(&mut rx));
    assert!(acts.iter().any(|a| a.kind == "blocked" && a.detail.contains("정리하지 못한 포지션")), "{acts:?}");
}

#[tokio::test]
async fn a_server_ended_session_is_not_called_a_web_stop() {
    let fb = FakeBroker::new(100.0, Market::Spot, Side::Long);
    let (mut bot, mut rx) = bot_with(json!({"symbol": "BTCUSDT"}), fb.clone(), RunMode::Mock);
    let mut guard = RiskGuard::new(&json!({}), 1000.0, 100.0);
    let ended = crate::server::HeartbeatReply {
        action: "stop_only".into(),
        reason: "세션을 찾을 수 없어요.".into(),
        ..Default::default()
    };
    bot.apply_reply(&ended, &mut guard, 100.0).await.unwrap();
    let acts = acts_of(&events(&mut rx));
    assert_eq!(acts[0].title, "서버가 이 실행을 끝냈어요");
    assert!(acts[0].detail.contains("세션을 찾을 수 없어요"));
    assert_eq!(bot.get_command(), Some(StopMode::StopOnly));

    let (mut bot, mut rx) = bot_with(json!({"symbol": "BTCUSDT"}), fb, RunMode::Mock);
    let web = crate::server::HeartbeatReply { action: "close_and_stop".into(), ..Default::default() };
    bot.apply_reply(&web, &mut guard, 100.0).await.unwrap();
    assert_eq!(acts_of(&events(&mut rx))[0].title, "웹에서 종료를 요청했어요");
}

#[tokio::test]
async fn a_long_gap_between_ticks_is_reported() {
    let fb = FakeBroker::new(100.0, Market::Spot, Side::Long);
    let (mut bot, mut rx) = bot_with(json!({"symbol": "BTCUSDT"}), fb, RunMode::Mock);
    bot.check_wall_gap();
    assert!(acts_of(&events(&mut rx)).is_empty(), "첫 틱은 비교할 게 없다");
    bot.last_tick_ms = unix_ms() - 10 * 60_000;
    bot.check_wall_gap();
    let acts = acts_of(&events(&mut rx));
    assert_eq!(acts[0].title, "기기가 10분 동안 멈춰 있었어요");
}

#[tokio::test]
async fn the_event_channel_closes_when_the_bot_is_gone() {
    // 서버 경고 콜백이 봇을 붙잡으면(참조 순환) 봇이 사라져도 채널이 닫히지 않아 앱이 정리를 못 함.
    let (tx, mut rx) = mpsc::unbounded_channel();
    let dir = tempfile::tempdir().unwrap();
    let outbox = Arc::new(crate::outbox::EventOutbox::open(Some(dir.path().join("o.sqlite3"))));
    let server = ServerClient::new("ggp_member", "http://127.0.0.1:9", outbox, reqwest::Client::new());
    let bot = Bot::new(
        json!({"symbol": "BTCUSDT"}),
        creds(),
        RunMode::Mock,
        Some(server.clone()),
        tx,
        reqwest::Client::new(),
    );
    drop(bot);
    let closed = tokio::time::timeout(Duration::from_secs(1), rx.recv()).await;
    assert!(matches!(closed, Ok(None)), "봇이 없어지면 수신이 끝나야 한다");
    drop(server);
}

#[tokio::test]
async fn a_partial_fill_reads_as_partial_not_as_a_failed_order() {
    let fb = FakeBroker::new(100.0, Market::Spot, Side::Long);
    let (mut bot, mut rx) = bot_with(json!({"symbol": "BTCUSDT"}), fb.clone(), RunMode::Live);
    fb.push(Order {
        status: "CANCELED".into(),
        executed_qty: 0.2,
        avg_price: 100.0,
        acquired_qty: 0.2,
        fees_known: true,
        order_id: Some("1".into()),
        raw_status: "EXPIRED".into(),
        fee: None,
    });
    let mut guard = RiskGuard::new(&json!({"max_holding_hours": 1}), 1000.0, 100.0);
    let reply = crate::server::HeartbeatReply {
        action: "continue".into(),
        commands: vec![json!({"id": 9, "action": "buy", "notional_frac": 0.5})],
        ..Default::default()
    };
    bot.apply_reply(&reply, &mut guard, 100.0).await.unwrap();
    assert!(bot.in_position && bot.held_qty > 0.0);
    let acts = acts_of(&events(&mut rx));
    assert!(acts.iter().any(|a| a.kind == "buy" && a.title.contains("샀어요")), "체결분은 체결로 남긴다: {acts:?}");
    assert!(acts.iter().any(|a| a.title == "주문이 일부만 체결됐어요"), "{acts:?}");
    assert!(!acts.iter().any(|a| a.title == "주문이 처리되지 않았어요"));
    assert!(guard.holding(), "일부만 체결돼도 보유 시간은 잰다");
}

#[tokio::test]
async fn mock_stops_never_talk_about_real_liquidation() {
    let fb = FakeBroker::new(100.0, Market::Spot, Side::Long);
    let (mut bot, _rx) = bot_with(json!({"symbol": "BTCUSDT"}), fb.clone(), RunMode::Mock);
    bot.in_position = true;
    bot.held_qty = 1.0;
    bot.entry_price = 90.0;
    assert_eq!(bot.finish_position(StopMode::CloseAndStop).await.unwrap(), "모의 보유분 정리 후 종료");
    assert!(!bot.in_position);

    let (mut bot, _rx) = bot_with(json!({"symbol": "BTCUSDT"}), fb.clone(), RunMode::Mock);
    bot.in_position = true;
    bot.held_qty = 1.0;
    assert_eq!(bot.finish_position(StopMode::StopOnly).await.unwrap(), "모의 보유분은 정리하지 않고 종료");

    // 실전에서 포지션을 남기고 끝내면 활동 기록에도 분명히 남김.
    let (mut bot, mut rx) = bot_with(json!({"symbol": "BTCUSDT"}), fb, RunMode::Live);
    bot.in_position = true;
    bot.held_qty = 1.0;
    assert_eq!(bot.finish_position(StopMode::StopOnly).await.unwrap(), "매크로만 종료 — 포지션 유지");
    assert!(acts_of(&events(&mut rx)).iter().any(|a| a.title == "포지션을 그대로 두고 끝냈어요" && a.tone == "warn"));
}

fn entry(id: i64, extra: Value) -> Value {
    let mut c = json!({"id": id, "action": "buy", "notional_frac": 0.5, "reason": "신호"});
    c.as_object_mut().unwrap().extend(extra.as_object().unwrap().clone());
    c
}

fn reply_with(cmds: Vec<Value>) -> crate::server::HeartbeatReply {
    crate::server::HeartbeatReply { action: "continue".into(), commands: cmds, ..Default::default() }
}

#[tokio::test]
async fn suspicious_entries_are_refused_before_any_order() {
    let fb = FakeBroker::new(100.0, Market::Spot, Side::Long);
    let (mut bot, mut rx) = bot_with(json!({"symbol": "BTCUSDT"}), fb.clone(), RunMode::Live);
    let mut guard = RiskGuard::new(&json!({}), 1000.0, 100.0);
    let cmds = vec![
        entry(1, json!({"notional_frac": "NaN"})),
        entry(2, json!({"expires_ms": (unix_ms() - 60_000) as f64})),
        entry(3, json!({"signal_price": 90.0})),
    ];
    bot.apply_reply(&reply_with(cmds), &mut guard, 100.0).await.unwrap();
    assert!(fb.submits().is_empty(), "이상한 진입 명령은 실주문을 내지 않는다");
    let errors: Vec<String> = bot.pending_acks.iter().map(|a| a["error"].as_str().unwrap().to_string()).collect();
    assert!(errors[0].contains("투자 비율"), "{errors:?}");
    assert!(errors[1].contains("유효 시간"), "{errors:?}");
    assert!(errors[2].contains("% 달라"), "{errors:?}");
    assert_eq!(acts_of(&events(&mut rx)).iter().filter(|a| a.kind == "blocked").count(), 3);

    // 신호 가격과 가까우면 진입함.
    bot.pending_acks.clear();
    bot.apply_reply(&reply_with(vec![entry(4, json!({"signal_price": 99.0}))]), &mut guard, 100.0).await.unwrap();
    assert_eq!(fb.submits().len(), 1);
}

#[tokio::test]
async fn adding_beyond_the_macro_capital_is_refused() {
    let fb = FakeBroker::new(100.0, Market::Spot, Side::Long);
    let m = json!({"symbol": "BTCUSDT", "params": {"initial_capital": 100}});
    let (mut bot, _rx) = bot_with(m, fb.clone(), RunMode::Live);
    let mut guard = RiskGuard::new(&json!({}), 100.0, 100.0);
    bot.in_position = true;
    bot.held_qty = 0.9;
    bot.entry_price = 100.0; // 이미 90 USDT 투자 중
    bot.apply_reply(&reply_with(vec![entry(5, json!({"notional_frac": 0.5}))]), &mut guard, 100.0).await.unwrap();
    assert!(fb.submits().is_empty(), "90 + 50 > 100 × 1.1");
    assert!(bot.pending_acks[0]["error"].as_str().unwrap().contains("투자 금액"));
}

#[tokio::test]
async fn a_stop_request_cancels_entries_in_the_same_reply_but_not_exits() {
    let fb = FakeBroker::new(100.0, Market::Spot, Side::Long);
    let (mut bot, _rx) = bot_with(json!({"symbol": "BTCUSDT"}), fb.clone(), RunMode::Live);
    let mut guard = RiskGuard::new(&json!({}), 1000.0, 100.0);
    bot.in_position = true;
    bot.held_qty = 0.5;
    bot.entry_price = 100.0;
    bot.set_command(StopMode::StopOnly);
    let cmds = vec![entry(6, json!({})), json!({"id": 7, "action": "sell", "reason": "익절"})];
    bot.apply_reply(&reply_with(cmds), &mut guard, 100.0).await.unwrap();
    let subs = fb.submits();
    assert_eq!(subs.len(), 1, "청산만 나간다");
    assert_eq!(subs[0].side, OrderSide::Sell);
    // '매크로만 종료' 뒤의 '청산 후 종료'는 받음(더 안전한 쪽).
    bot.set_command(StopMode::CloseAndStop);
    assert_eq!(bot.get_command(), Some(StopMode::CloseAndStop));
    bot.set_command(StopMode::StopOnly);
    assert_eq!(bot.get_command(), Some(StopMode::CloseAndStop), "반대로는 바꾸지 않는다");
}

#[tokio::test]
async fn a_remote_stop_acks_the_commands_that_came_with_it() {
    let fb = FakeBroker::new(100.0, Market::Spot, Side::Long);
    let (mut bot, _rx) = bot_with(json!({"symbol": "BTCUSDT"}), fb.clone(), RunMode::Live);
    let mut guard = RiskGuard::new(&json!({}), 1000.0, 100.0);
    let reply = crate::server::HeartbeatReply {
        action: "stop_only".into(),
        commands: vec![entry(8, json!({}))],
        ..Default::default()
    };
    bot.apply_reply(&reply, &mut guard, 100.0).await.unwrap();
    assert!(fb.submits().is_empty());
    assert_eq!(bot.pending_acks[0]["outcome"], "stop_requested");
}

#[tokio::test]
async fn exits_still_run_while_the_price_feed_is_down() {
    let fb = FakeBroker::new(100.0, Market::Spot, Side::Long);
    let (mut bot, _rx) = bot_with(json!({"symbol": "BTCUSDT"}), fb.clone(), RunMode::Live);
    let mut guard = RiskGuard::new(&json!({}), 1000.0, 100.0);
    bot.in_position = true;
    bot.held_qty = 0.5;
    bot.entry_price = 100.0;
    bot.last_price = 100.0;
    // 서버 없는 봇의 하트비트는 명령이 없으므로, 시세 실패 틱의 명령 처리를 직접 부름.
    bot.skip_commands(&[entry(9, json!({}))], "price_unavailable", "x");
    let exits = reply_with(vec![json!({"id": 10, "action": "sell", "reason": "청산"})]);
    bot.apply_reply(&exits, &mut guard, bot.last_price).await.unwrap();
    assert_eq!(fb.submits().len(), 1, "시장가 청산은 보유 수량만 있으면 된다");
    assert!(!bot.in_position);
}

#[tokio::test]
async fn repeated_stop_loss_failures_keep_retrying_instead_of_ending_the_run() {
    let fb = FakeBroker::new(80.0, Market::Spot, Side::Long);
    let m = json!({"symbol": "BTCUSDT", "risk": {"stop_loss_pct": 5}});
    let (mut bot, mut rx) = bot_with(m, fb.clone(), RunMode::Live);
    let mut guard = RiskGuard::new(&json!({}), 1000.0, 100.0);
    bot.in_position = true;
    bot.held_qty = 1.0;
    bot.entry_price = 100.0;
    let rejected = || Order {
        status: "REJECTED".into(),
        executed_qty: 0.0,
        avg_price: 0.0,
        acquired_qty: 0.0,
        fees_known: true,
        order_id: None,
        raw_status: "-2010".into(),
        fee: None,
    };
    for _ in 0..MAX_CLOSE_FAILURES {
        fb.push(rejected());
        assert!(bot.enforce_safety_net(&mut guard, 80.0).await.is_ok(), "실행을 끝내지 않는다(손절 감시가 사라지므로)");
    }
    assert_eq!(bot.close_failures, MAX_CLOSE_FAILURES);
    assert!(bot.close_retry_at > unix_ms(), "간격을 두고 다시 시도한다");
    let before = fb.submits().len();
    bot.enforce_safety_net(&mut guard, 80.0).await.unwrap();
    assert_eq!(fb.submits().len(), before, "다음 시도 시각 전에는 보내지 않는다");
    assert!(acts_of(&events(&mut rx)).iter().any(|a| a.title.contains("연달아 실패")));
}

#[tokio::test]
async fn a_practice_key_hint_does_not_mention_an_allowed_ip() {
    let m = json!({"symbol": "BTCUSDT", "exchange": "binance", "position_side": "short", "leverage": 2});
    let fb = FakeBroker::new(100.0, Market::Futures, Side::Short);
    let (bot, mut rx) = bot_with(m.clone(), fb.clone(), RunMode::Testnet);
    assert!(!bot.rehearsal_failed("바이낸스 -2015 키 거절"));
    let hint = logs(&mut rx).into_iter().find(|l| l.contains("키인지")).unwrap();
    assert!(hint.contains("선물 데모(demo.binance.com) 키인지, 권한을 확인하세요."), "{hint}");
    assert!(!hint.contains("허용 IP"), "{hint}");

    let (bot, mut rx) = bot_with(m, fb, RunMode::Live);
    assert!(!bot.rehearsal_failed("바이낸스 -2015 키 거절"));
    let hint = logs(&mut rx).into_iter().find(|l| l.contains("키인지")).unwrap();
    assert!(hint.contains("실거래(binance.com) 키인지, 허용 IP와 권한을 확인하세요."), "{hint}");
}

// --- 수수료·신호가 차이·잔고 (화면 전용) -----------------------------------------

fn filled_with_fee(qty: f64, price: f64, fee: Option<crate::brokers::FeePaid>) -> Order {
    Order { fee, ..filled(qty, price) }
}

fn fee(quote: f64) -> Option<crate::brokers::FeePaid> {
    Some(crate::brokers::FeePaid { quote, other: vec![] })
}

#[tokio::test]
async fn fees_and_signal_slippage_reach_the_screen_but_not_the_server_snapshot() {
    let fb = FakeBroker::new(100.0, Market::Spot, Side::Long);
    let (mut bot, mut rx) = bot_with(json!({"symbol": "BTCUSDT"}), fb.clone(), RunMode::Live);
    fb.push(filled_with_fee(0.5, 100.0, fee(0.05)));
    let buy = json!({"id": 1, "action": "buy", "notional_frac": 0.5, "signal_price": 99.0});
    bot.execute_command(&buy, 100.0).await.unwrap();
    let acts = activities(&mut rx);
    let bought = acts.iter().find(|a| a.kind == "buy").unwrap();
    assert_eq!(bought.detail, "신호가 대비 +1.01% · 수수료 0.05 USDT");

    fb.st.lock().unwrap().price = 110.0;
    fb.push(filled_with_fee(0.5, 110.0, fee(0.055)));
    bot.execute_command(&json!({"id": 2, "action": "sell", "signal_price": 110.0}), 110.0).await.unwrap();
    let acts = activities(&mut rx);
    let sold = acts.iter().find(|a| a.kind == "sell").unwrap();
    assert_eq!(sold.detail, "손익 +5.00 USDT · 누적 +5.00 USDT · 신호가 대비 +0.00% · 수수료 0.055 USDT");

    // 서버로 가는 스냅샷은 파이썬 실행기와 같음 — 실현손익에서 수수료를 빼지 않고 수수료 칸도 없음.
    let snap = bot.snapshot();
    assert_eq!(snap["realized_pnl"], 5.0);
    assert!(snap.get("fees_paid").is_none() && snap.get("day_change").is_none());
    let ui = bot.ui_snapshot(bot.snapshot());
    assert!((ui["fees_paid"].as_f64().unwrap() - 0.105).abs() < 1e-12, "{ui}");
    assert_eq!(ui["realized_pnl"], 5.0);

    // 손절처럼 신호 없이 낸 주문에는 신호가 차이를 붙이지 않음.
    fb.push(filled_with_fee(0.5, 100.0, fee(0.0)));
    bot.execute_command(&json!({"id": 3, "action": "buy", "notional_frac": 0.5}), 100.0).await.unwrap();
    activities(&mut rx);
    fb.push(filled_with_fee(0.5, 90.0, None));
    assert!(bot.close_position().await.unwrap());
    let sold = activities(&mut rx).into_iter().find(|a| a.kind == "sell").unwrap();
    assert!(!sold.detail.contains("신호가") && !sold.detail.contains("수수료"), "{}", sold.detail);
    // 수수료를 모르는 체결이 끼면 화면에도 모름(null)으로 — 수수료 뺀 손익을 주장하지 않음.
    assert!(bot.ui_snapshot(bot.snapshot())["fees_paid"].is_null());
}

#[tokio::test]
async fn fees_in_other_assets_are_shown_but_leave_the_total_unknown() {
    let fb = FakeBroker::new(100.0, Market::Spot, Side::Long);
    let (mut bot, mut rx) = bot_with(json!({"symbol": "BTCUSDT"}), fb.clone(), RunMode::Live);
    let bnb = crate::brokers::FeePaid { quote: 0.0, other: vec![("BNB".into(), 0.0003)] };
    fb.push(filled_with_fee(0.5, 100.0, Some(bnb)));
    bot.execute_command(&json!({"id": 1, "action": "buy", "notional_frac": 0.5}), 100.0).await.unwrap();
    let bought = activities(&mut rx).into_iter().find(|a| a.kind == "buy").unwrap();
    assert_eq!(bought.detail, "수수료 0.0003 BNB");
    assert!(bot.ui_snapshot(bot.snapshot())["fees_paid"].is_null());
}

#[tokio::test]
async fn futures_fees_are_unknown_from_the_start() {
    let fb = FakeBroker::new(100.0, Market::Futures, Side::Long);
    let (bot, _rx) = bot_with(json!({"symbol": "BTCUSDT", "leverage": 3}), fb, RunMode::Live);
    assert_eq!(bot.market, Market::Futures);
    let ui = bot.ui_snapshot(bot.snapshot());
    assert!(ui["fees_paid"].is_null());
    assert!(ui["day_change"].is_null());
}

#[test]
fn slippage_and_fee_texts() {
    assert_eq!(super::orders::slippage_text(100.0, 100.12).as_deref(), Some("신호가 대비 +0.12%"));
    assert_eq!(super::orders::slippage_text(100.0, 99.5).as_deref(), Some("신호가 대비 -0.50%"));
    assert_eq!(super::orders::slippage_text(0.0, 99.5), None);
    assert_eq!(super::orders::fee_text(fee(12.0).as_ref(), "KRW").as_deref(), Some("수수료 12 KRW"));
    assert_eq!(super::orders::fee_text(fee(0.0).as_ref(), "USDT"), None);
    assert_eq!(super::orders::fee_text(None, "USDT"), None);
}

#[tokio::test]
async fn the_balance_read_before_start_becomes_one_activity_line_outside_mock() {
    let m = json!({"symbol": "BTCUSDT"});
    for (mode, expected) in [(RunMode::Live, true), (RunMode::Mock, false)] {
        let fb = FakeBroker::new(100.0, Market::Spot, Side::Long);
        let (mut bot, mut rx) = bot_with(m.clone(), fb, mode);
        let mut pre = PreflightResult::new(true, "확인 통과", preflight_fingerprint(&m, mode, &creds()));
        pre.balance = "사용 가능 잔고 10 USDT · 보유 BTC 0.01".into();
        bot.preflight = Some(pre);
        assert!(bot.rehearse().await);
        let line = activities(&mut rx).into_iter().find(|a| a.title == "잔고를 확인했어요");
        assert_eq!(line.is_some(), expected, "{mode:?}");
        if let Some(line) = line {
            assert_eq!(line.detail, "사용 가능 잔고 10 USDT · 보유 BTC 0.01");
            assert_eq!((line.kind, line.tone), ("info", "muted"));
        }
    }
}

// --- 주문 결과 분류·보유 반영·명령 갈래의 경계 ------------------------------------

fn result(status: &str, exec: f64, avg: f64, acquired: f64, fees_known: bool) -> Order {
    Order {
        status: status.into(),
        executed_qty: exec,
        avg_price: avg,
        acquired_qty: acquired,
        fees_known,
        order_id: Some("1".into()),
        raw_status: if status == "CANCELED" { "EXPIRED".into() } else { status.into() },
        fee: None,
    }
}

/// (보유 중, 보유 수량, 진입가, 불확실, 실현 손익, 먼지 수량)
fn pos(bot: &Bot) -> (bool, f64, f64, bool, f64, f64) {
    (bot.in_position, bot.held_qty, bot.entry_price, bot.position_uncertain, bot.realized, bot.position_dust_qty)
}

fn close(a: f64, b: f64) -> bool {
    (a - b).abs() < 1e-9
}

#[tokio::test]
async fn order_results_are_classified_at_their_boundaries() {
    // (거래소 결과, Ok인지, 오류 문장 앞부분, (보유 중, 보유 수량, 진입가, 불확실))
    type Case = (Order, bool, &'static str, (bool, f64, f64, bool));
    let cases: Vec<Case> = vec![
        (result("FILLED", 1.0, 100.0, 1.0, true), true, "", (true, 1.0, 100.0, false)),
        (
            result("FILLED", 1.0, 0.0, 1.0, true),
            false,
            "주문 결과를 확인하지 못했어요(거래소 상태: FILLED)",
            (true, 1.0, 0.0, true),
        ),
        (
            result("FILLED", 0.0, 0.0, 0.0, true),
            false,
            "주문 결과를 확인하지 못했어요(거래소 상태: FILLED)",
            (true, 0.0, 0.0, true),
        ),
        (
            result("OPEN", 0.5, 100.0, 0.5, true),
            false,
            "주문 결과를 확인하지 못했어요(거래소 상태: OPEN)",
            (true, 0.5, 100.0, true),
        ),
        (result("FILLED", 1.0, 100.0, 1.0, false), false, "주문 결과를 확인하지 못했어요", (true, 1.0, 100.0, true)),
        (
            result("REJECTED", 0.0, 0.0, 0.0, true),
            false,
            "바이낸스에서 주문을 거절했어요(REJECTED)",
            (false, 0.0, 0.0, false),
        ),
        (
            result("REJECTED", 0.3, 100.0, 0.3, true),
            false,
            "주문이 일부만 체결되고 끝났어요(거래소 상태: REJECTED)",
            (true, 0.3, 100.0, false),
        ),
        (
            result("CANCELED", 0.4, 100.0, 0.4, true),
            false,
            "주문이 일부만 체결되고 끝났어요(거래소 상태: EXPIRED)",
            (true, 0.4, 100.0, false),
        ),
        (
            result("CANCELED", 0.0, 0.0, 0.0, true),
            false,
            "주문 결과를 확인하지 못했어요(거래소 상태: EXPIRED)",
            (false, 0.0, 0.0, false),
        ),
    ];
    for (order, ok, err, want) in cases {
        let label = format!("{} {} {}", order.status, order.executed_qty, order.avg_price);
        let fb = FakeBroker::new(100.0, Market::Spot, Side::Long);
        let (mut bot, _rx) = bot_with(json!({"symbol": "BTCUSDT"}), fb.clone(), RunMode::Live);
        fb.push(order);
        let got = bot.place(OrderSide::Buy, 1.0, false, 0.0).await;
        assert_eq!(got.is_ok(), ok, "{label}: {got:?}");
        if let Err(e) = &got {
            assert!(e.starts_with(err), "{label}: {e}");
        }
        let p = pos(&bot);
        assert_eq!((p.0, p.3), (want.0, want.3), "{label}");
        assert!(close(p.1, want.1) && close(p.2, want.2), "{label}: {p:?}");
    }
}

#[tokio::test]
async fn fills_update_the_position_at_their_boundaries() {
    let spot = || {
        let fb = FakeBroker::new(100.0, Market::Spot, Side::Long);
        let (mut bot, rx) = bot_with(json!({"symbol": "BTCUSDT"}), fb.clone(), RunMode::Live);
        bot.step = 0.001;
        (bot.in_position, bot.held_qty, bot.entry_price) = (true, 1.0, 100.0);
        (bot, fb, rx)
    };
    // 단위보다 적게 남은 현물 — 먼지로 넘기고 보유를 끝냄.
    let (mut bot, fb, _rx) = spot();
    fb.push(result("FILLED", 0.9995, 110.0, 0.9995, true));
    assert_eq!(bot.place(OrderSide::Sell, 1.0, false, 0.0).await, Ok(true));
    let p = pos(&bot);
    assert!(!p.0 && close(p.1, 0.0005) && p.2 == 0.0 && !p.3 && close(p.4, 9.995) && close(p.5, 0.0005), "{p:?}");
    // 보유보다 많이 체결 — 손익은 보유분만, 보유는 0.
    let (mut bot, fb, _rx) = spot();
    fb.push(result("FILLED", 1.2, 110.0, 1.2, true));
    assert_eq!(bot.place(OrderSide::Sell, 1.0, false, 0.0).await, Ok(true));
    let p = pos(&bot);
    assert!(!p.0 && p.1 == 0.0 && p.2 == 0.0 && close(p.4, 10.0) && p.5 == 0.0, "{p:?}");
    // 1e-12 아래로 남은 수량은 0.
    let (mut bot, fb, _rx) = spot();
    fb.push(result("FILLED", 1.0 - 1e-13, 100.0, 1.0 - 1e-13, true));
    assert_eq!(bot.place(OrderSide::Sell, 1.0, false, 0.0).await, Ok(true));
    assert_eq!(bot.held_qty, 0.0);
    // 끝나지 않은 정리 주문 — 진입가를 지움 없이 보유와 불확실 표시를 남김.
    let (mut bot, fb, _rx) = spot();
    fb.push(result("OPEN", 0.5, 100.0, 0.5, true));
    assert!(bot.place(OrderSide::Sell, 1.0, false, 0.0).await.is_err());
    let p = pos(&bot);
    assert!(p.0 && close(p.1, 0.5) && p.2 == 100.0 && p.3, "{p:?}");
    // 만료로 끝난 현물 정리 — 먼지 규칙은 FILLED만, 남은 수량은 보유로.
    let (mut bot, fb, _rx) = spot();
    fb.push(result("CANCELED", 0.9995, 100.0, 0.9995, true));
    assert!(bot.place(OrderSide::Sell, 1.0, false, 0.0).await.unwrap_err().starts_with("주문이 일부만 체결"));
    let p = pos(&bot);
    assert!(p.0 && close(p.1, 0.0005) && p.2 == 100.0 && p.5 == 0.0, "{p:?}");
    // 추가 진입 — 받은 수량(acquired)으로 가중평균.
    let (mut bot, fb, _rx) = spot();
    fb.push(result("FILLED", 1.0, 50.0, 0.999, true));
    assert_eq!(bot.place(OrderSide::Buy, 1.0, false, 0.0).await, Ok(true));
    let p = pos(&bot);
    assert!(p.0 && close(p.1, 1.999) && close(p.2, (100.0 + 0.999 * 50.0) / 1.999), "{p:?}");
    // 평균가를 모르는 추가 진입 — 진입가·보유를 이번 체결로 바꾸고 불확실로 둠.
    let (mut bot, fb, _rx) = spot();
    fb.push(result("FILLED", 1.0, 0.0, 1.0, true));
    assert!(bot.place(OrderSide::Buy, 1.0, false, 0.0).await.is_err());
    let p = pos(&bot);
    assert!(p.0 && p.1 == 1.0 && p.2 == 0.0 && p.3, "{p:?}");

    // 선물 정리는 먼지 규칙 없이 남은 수량을 보유로 둠.
    let fb = FakeBroker::new(100.0, Market::Futures, Side::Short);
    let (mut bot, _rx) =
        bot_with(json!({"symbol": "BTCUSDT", "position_side": "short", "leverage": 2}), fb.clone(), RunMode::Live);
    bot.step = 0.001;
    (bot.in_position, bot.held_qty, bot.entry_price) = (true, 1.0, 100.0);
    fb.push(result("FILLED", 0.9995, 90.0, 0.9995, true));
    assert!(bot.place(OrderSide::Buy, 1.0, true, 0.0).await.unwrap_err().starts_with("주문이 일부만 체결"));
    let p = pos(&bot);
    assert!(p.0 && close(p.1, 0.0005) && p.2 == 100.0 && close(p.4, 9.995) && p.5 == 0.0, "{p:?}");
    // 남은 수량보다 단위 이상 적게 판 일부 정리는 성공.
    fb.push(result("FILLED", 0.0002, 90.0, 0.0002, true));
    bot.held_qty = 1.0;
    assert_eq!(bot.place(OrderSide::Buy, 0.0002, true, 0.0).await, Ok(true));
}

#[tokio::test]
async fn command_branches_at_their_boundaries() {
    let holding = || {
        let fb = FakeBroker::new(100.0, Market::Spot, Side::Long);
        let (mut bot, rx) = bot_with(json!({"symbol": "BTCUSDT"}), fb.clone(), RunMode::Live);
        bot.step = 0.001;
        (bot.in_position, bot.held_qty, bot.entry_price) = (true, 1.0, 100.0);
        (bot, fb, rx)
    };
    // 0.999 이상은 전량 정리, 그 아래는 단위로 내린 일부.
    let (mut bot, fb, _rx) = holding();
    let ack = bot.execute_command(&json!({"id": 1, "action": "sell", "qty_frac": 0.999}), 100.0).await.unwrap();
    assert_eq!(fb.submits()[0].base_qty, Some(1.0));
    assert_eq!(ack["outcome"], "execution_reported");
    let (mut bot, fb, _rx) = holding();
    bot.execute_command(&json!({"id": 1, "action": "sell", "qty_frac": 0.9985}), 100.0).await.unwrap();
    assert_eq!(fb.submits()[0].base_qty, Some(0.998));
    // 0이나 빠진 비율은 전량.
    for frac in [json!(0), Value::Null] {
        let (mut bot, fb, _rx) = holding();
        bot.execute_command(&json!({"id": 1, "action": "sell", "qty_frac": frac}), 100.0).await.unwrap();
        assert_eq!(fb.submits()[0].base_qty, Some(1.0));
    }
    // 부분 정리 수량이 단위보다 작으면 주문 없이 실패.
    let (mut bot, fb, _rx) = holding();
    let ack = bot.execute_command(&json!({"id": 1, "action": "sell", "qty_frac": 0.0001}), 100.0).await.unwrap();
    assert_eq!(ack["error"], "부분 청산 수량이 최소 단위보다 작아요.");
    assert!(fb.submits().is_empty());
    // 정리 전부터 불확실하면 주문 없이 실패 ack(루프는 계속).
    let (mut bot, fb, _rx) = holding();
    bot.position_uncertain = true;
    let ack = bot.execute_command(&json!({"id": 1, "action": "sell"}), 100.0).await.unwrap();
    assert_eq!(ack["ok"], false);
    assert_eq!(ack["outcome"], "execution_failed");
    assert!(ack["error"].as_str().unwrap().starts_with("포지션을 확인하지 못해"));
    assert!(fb.submits().is_empty());

    // 진입 비율은 0~1로 자름.
    let fresh = || {
        let fb = FakeBroker::new(100.0, Market::Spot, Side::Long);
        let (bot, rx) =
            bot_with(json!({"symbol": "BTCUSDT", "params": {"initial_capital": 50}}), fb.clone(), RunMode::Live);
        (bot, fb, rx)
    };
    let (mut bot, fb, _rx) = fresh();
    bot.execute_command(&json!({"id": 1, "action": "buy", "notional_frac": 2.0}), 100.0).await.unwrap();
    assert_eq!(fb.submits()[0].base_qty, Some(0.5));
    let (mut bot, fb, _rx) = fresh();
    let ack = bot.execute_command(&json!({"id": 1, "action": "buy", "notional_frac": -1}), 100.0).await.unwrap();
    assert_eq!(ack["error"], "주문 수량이 최소 단위보다 작아요.");
    assert!(fb.submits().is_empty());
    // 보유 중 추가 진입에서 보유가 늘지 않은 체결(받은 수량 0) — 체결 수량으로 보유·진입가를 채움.
    let (mut bot, fb, _rx) = holding();
    bot.capital = 100.0;
    fb.push(result("FILLED", 0.5, 50.0, 0.0, true));
    let ack = bot.execute_command(&json!({"id": 1, "action": "buy", "notional_frac": 0.25}), 50.0).await.unwrap();
    assert_eq!(
        ack,
        json!({"command_id": 1, "ok": true, "executed_qty": 0.5, "fill_price": 50.0, "error": "",
               "mode": "live", "outcome": "execution_reported"})
    );
    assert!(close(bot.held_qty, 1.5) && close(bot.entry_price, (100.0 + 0.5 * 50.0) / 1.5), "{:?}", pos(&bot));
}
