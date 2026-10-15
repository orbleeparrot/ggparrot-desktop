//! 한 매크로를 실제로 구동하는 루프. heartbeat·서버 명령 실행·로컬 안전망을 한 태스크에서 돌림.
//!
//! 종료 경로는 두 가지 — 로컬(화면의 종료 버튼) → `BotHandle::command`, 원격(웹의 [내 에이전트]) → heartbeat 응답의 action.
//! 두 경우 모두 stop_only(포지션 유지) / close_and_stop(청산 후)을 지원함.
//! 진입/청산 판단은 서버가 함. 로컬에 남은 판단은 안전망(손절 %, 일일 손실, 최대 보유시간)뿐임.

use serde_json::{json, Map, Value};
use std::collections::VecDeque;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::sync::{mpsc, Notify};

use crate::brokers::{
    fee_inclusive, BinanceBroker, Broker, BrokerError, DomesticBroker, FeePaid, LogFn, MockBroker, OrderSide,
    SubmitRequest, DOMESTIC_BID_FEE_FALLBACK, VOLUME_DECIMALS,
};
use crate::connection::{preflight_fingerprint, safe_preflight_reason, PreflightResult};
use crate::log_kind::event_kind;
use crate::money::{
    grouped, money, order_qty, pnl_amount, pnl_pct, price_text, py_g, py_gp, py_num, qty_text, round_step,
    smallest_entry_notional,
};
use crate::risk::RiskGuard;
use crate::server::ServerClient;
use crate::types::{
    base_asset, exchange_label, exchange_name_of, macro_leverage, macro_side, market_of, order_cap, quote_of,
    Credentials, Exchange, KeyPair, Market, RunMode, Side,
};

/// 손절·강제 청산 주문이 이만큼 연달아 실패하면 실행을 멈추고 사용자에게 넘김.
const MAX_CLOSE_FAILURES: u32 = 5;
pub const POLL_SECONDS: f64 = 5.0;
/// 포지션을 든 채 현재가를 이만큼 못 받으면(손절을 못 보는 중) 한 번 더 크게 알림.
const PRICE_STALE_ALERT_MS: u64 = 60_000;
/// 진입 명령의 신호 가격과 지금 가격이 이만큼 넘게 다르면 진입하지 않음(끊겼다 돌아와 밀린 신호 등).
const MAX_SIGNAL_DEVIATION: f64 = 0.03;
/// 서버가 진입 명령에 단 만료 시각을 이만큼 넘겼으면 진입하지 않음(두 시계의 차이를 조금 봐줌).
const ENTRY_EXPIRY_GRACE_MS: i64 = 5_000;
/// 보유 중 추가 진입의 총 투자 한도 — 매크로 투자 금액의 이 배수(수수료·반올림 여유).
const EXPOSURE_TOLERANCE: f64 = 1.1;
/// 손절·강제 청산이 연달아 실패한 뒤 다시 시도하는 최장 간격.
const CLOSE_RETRY_MAX_MS: u64 = 60_000;
/// 시세 실패 원문 로그를 남기는 간격(틱) — 5초 틱이면 1분에 한 번.
const PRICE_LOG_EVERY_TICKS: u32 = 12;
/// 서버와 이만큼 끊기면 웹의 종료·정리 신호가 오지 않는다고 알림.
const OFFLINE_ALERT_MS: u64 = 5 * 60_000;
/// 틱 사이가 이만큼 벌어지면 잠자기 등으로 멈췄던 것으로 봄(주문 한 건이 길어도 1분 안팎).
const WALL_GAP_ALERT_MS: u64 = 2 * 60_000;
const DUST_FLOOR: f64 = VOLUME_DECIMALS;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StopMode {
    StopOnly,
    CloseAndStop,
}

impl StopMode {
    pub fn as_str(self) -> &'static str {
        match self {
            StopMode::StopOnly => "stop_only",
            StopMode::CloseAndStop => "close_and_stop",
        }
    }
    pub fn parse(s: &str) -> Option<StopMode> {
        match s {
            "stop_only" => Some(StopMode::StopOnly),
            "close_and_stop" => Some(StopMode::CloseAndStop),
            _ => None,
        }
    }
    pub fn label(self) -> &'static str {
        match self {
            StopMode::StopOnly => "매크로만 종료",
            StopMode::CloseAndStop => "청산 후 종료",
        }
    }
}

#[derive(Clone, Debug)]
pub enum BotEvent {
    /// 화면 로그 한 줄.
    Log(String),
    /// heartbeat 직전 상태 스냅샷(서버에 올리는 것과 같은 모양).
    Status(Value),
    /// 연결·시세·정리 상태. 매 틱 보냄(화면·트레이가 저하 상태를 보이게).
    Health(RunHealth),
    /// 실행이 끝나 가는 중 — 서버에 종료를 보고하기 전에 먼저 알림(보고가 늦어도 화면이 바로 바뀌게).
    Ending {
        status: String,
        note: String,
    },
    /// 사람이 읽는 활동 한 줄(활동 기록). 서버로 가는 실행 기록과는 별개임.
    Activity(ActivityEvent),
    Finished {
        status: String,
        note: String,
    },
}

/// 실행 중 저하 상태. 시각은 벽시계 ms(0이면 해당 없음).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct RunHealth {
    /// 마지막 하트비트가 서버에 닿았는지 여부.
    pub online: bool,
    /// 서버가 회원 키를 받지 않음(401·403).
    pub auth_failed: bool,
    /// 서버가 요청을 거절한 HTTP 상태(일시적 끊김이 아님). 0이면 아님.
    pub rejected: u16,
    /// 서버와 끊긴 시각.
    pub offline_since: u64,
    /// 현재가를 못 받기 시작한 시각 — 그동안 손절·안전 규칙을 확인하지 못함.
    pub price_stale_since: u64,
    /// 손절·강제 청산 주문이 연달아 실패한 횟수(이만큼 남은 포지션을 정리하지 못함).
    pub close_failures: u32,
}

/// 벽시계 ms — 잠자기 동안 멈추는 Instant 대신 씀(깨어난 뒤 공백을 재려고).
pub(crate) fn wall_ms() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis() as u64).unwrap_or(0)
}

/// 활동 기록 한 줄. kind: start·signal·buy·sell·stop_loss·blocked·connection·error·stop·info.
#[derive(Clone, Debug, PartialEq)]
pub struct ActivityEvent {
    pub kind: &'static str,
    pub title: String,
    pub detail: String,
    /// 바로 앞의 원문 로그 줄(자세히 보기).
    pub raw: String,
    /// muted·ok·warn·danger
    pub tone: &'static str,
}

pub use logger::Logger;

fn obj(v: Value) -> Map<String, Value> {
    match v {
        Value::Object(m) => m,
        _ => Map::new(),
    }
}

fn num(v: Option<&Value>) -> f64 {
    match v {
        Some(Value::Number(n)) => n.as_f64().unwrap_or(0.0),
        Some(Value::String(s)) => s.trim().parse().unwrap_or(0.0),
        Some(Value::Bool(true)) => 1.0,
        _ => 0.0,
    }
}

fn value_text(v: Option<&Value>) -> String {
    match v {
        None | Some(Value::Null) => String::new(),
        Some(Value::String(s)) => s.clone(),
        Some(v) => v.to_string(),
    }
}

/// 매수 수수료율. 못 읽었거나 말이 안 되는 값이면 보수적인 기본값.
pub fn bid_fee_rate(broker: Option<&Arc<dyn Broker>>) -> f64 {
    match broker.and_then(|b| b.bid_fee()) {
        Some(rate) if rate > 0.0 && rate < 0.05 => rate,
        _ => DOMESTIC_BID_FEE_FALLBACK,
    }
}

/// 이 매크로가 고른 거래소·모드로 어댑터 하나를 만듦(모의 감싸기 전).
pub fn build_broker(
    exchange: Exchange,
    mode: RunMode,
    m: &Value,
    pair: &KeyPair,
    log: LogFn,
    http: reqwest::Client,
) -> Result<Arc<dyn Broker>, BrokerError> {
    let side = macro_side(m);
    let leverage = macro_leverage(m).unwrap_or(1);
    let symbol = crate::types::macro_symbol(m);
    let symbol = if symbol.is_empty() { "BTCUSDT".to_string() } else { symbol };
    let market = market_of(exchange.as_str(), side, leverage);
    match exchange {
        Exchange::Binance => {
            let testnet = mode == RunMode::Testnet;
            Ok(Arc::new(BinanceBroker::new(
                &pair.api_key,
                &pair.api_secret,
                market,
                &symbol,
                side,
                testnet,
                leverage,
                log,
                http,
                None,
            )))
        }
        _ => Ok(Arc::new(DomesticBroker::new(&pair.api_key, &pair.api_secret, exchange, &symbol, log, http, None)?)),
    }
}

/// 리허설 인자 (notional, spend) — 이 매크로의 진입 한 건이 실제로 쓸 가장 작은 금액.
pub fn rehearsal_amounts(m: &Value, exchange_name: &str, broker: Option<&Arc<dyn Broker>>) -> (f64, Option<f64>) {
    let side = macro_side(m);
    let leverage = macro_leverage(m).unwrap_or(1);
    let market = market_of(exchange_name, side, leverage);
    let quote = quote_of(&crate::types::macro_symbol(m));
    let budget = smallest_entry_notional(m, leverage, market, quote);
    let domestic = Exchange::parse(exchange_name).is_some_and(Exchange::is_domestic);
    if domestic {
        (fee_inclusive(budget, bid_fee_rate(broker)), Some(budget))
    } else {
        (budget, None)
    }
}

pub struct Bot {
    pub macro_: Value,
    credentials: Credentials,
    exchange_name: String,
    pub mode: RunMode,
    server: Option<ServerClient>,
    tx: mpsc::UnboundedSender<BotEvent>,
    pub logger: Logger,
    command: Arc<Mutex<Option<StopMode>>>,
    wake: Arc<Notify>,
    http: reqwest::Client,

    pub symbol: String,
    pub quote: &'static str,
    pub side: Side,
    pub leverage: u32,
    pub market: Market,
    pub broker: Option<Arc<dyn Broker>>,
    /// 테스트·연결 검사에서 만든 어댑터를 그대로 씀(모의 감싸기는 그대로 함).
    pub broker_override: Option<Arc<dyn Broker>>,
    pub step: f64,
    pub in_position: bool,
    pub entry_price: f64,
    pub held_qty: f64,
    pub realized: f64,
    /// 이번 실행에서 낸 수수료(견적 통화). 화면에만 씀 — 서버로 가는 realized_pnl은 수수료를 빼지 않음.
    pub fees_quote: f64,
    /// 모든 체결의 수수료를 견적 통화로 알았는지 여부(선물·BNB 수수료가 끼면 거짓).
    pub fees_complete: bool,
    /// 지금 실행 중인 서버 명령의 신호 가격(0이면 없음) — 체결 활동에 신호가와의 차이를 보임.
    signal_price: f64,
    pub position_uncertain: bool,
    pub position_dust_qty: f64,
    pub capital: f64,
    done_command_ids: VecDeque<Value>,
    pub pending_acks: Vec<Value>,
    /// 손절·강제 청산 주문이 연달아 실패한 수 — 다음 틱에 다시 시도하다 너무 많으면 멈춤.
    close_failures: u32,
    /// 회원 키 거절(401·403)을 한 번만 알림.
    auth_noticed: bool,
    offline_logged: bool,
    price_failing: bool,
    /// 저하 상태를 잰 시각·한 번만 알린 경고(→ [`RunHealth`]).
    offline_since: u64,
    offline_escalated: bool,
    rejected_noticed: u16,
    price_fail_since: u64,
    price_escalated: bool,
    /// 시세 실패가 이어진 틱 수 — 원문 로그는 처음과 1분마다만 남김(장애 동안 활동 기록·실행 기록을 채우지 않게).
    price_fail_ticks: u32,
    /// 정리 주문이 한도만큼 실패한 뒤 다음에 다시 시도할 벽시계 시각(그 전에는 시도하지 않음).
    close_retry_at: u64,
    /// 지난 틱의 벽시계 — 틱 사이가 크게 벌어지면 잠자기 등으로 멈췄던 것임.
    last_tick_ms: u64,
    pub preflight: Option<PreflightResult>,
    pub last_price: f64,
    pub last_fill_qty: f64,
    pub last_fill_price: f64,
    pub poll: Duration,
}

pub struct BotHandle {
    command: Arc<Mutex<Option<StopMode>>>,
    wake: Arc<Notify>,
    pub quote: &'static str,
    pub symbol: String,
    pub side: Side,
    pub join: tokio::task::JoinHandle<()>,
}

impl BotHandle {
    /// 아무 일도 하지 않는 손잡이 — 앱 상태(실행 중·저하 표시) 테스트용. tokio 런타임 안에서 부름.
    #[doc(hidden)]
    pub fn idle_for_tests(symbol: &str) -> BotHandle {
        BotHandle {
            command: Arc::new(Mutex::new(None)),
            wake: Arc::new(Notify::new()),
            quote: quote_of(symbol),
            symbol: symbol.into(),
            side: Side::Long,
            join: tokio::spawn(async {}),
        }
    }

    /// 종료 명령. 첫 명령만 받음.
    pub fn command(&self, mode: StopMode) {
        let mut c = self.command.lock().unwrap();
        if c.is_none() {
            *c = Some(mode);
        }
        drop(c);
        self.wake.notify_one();
    }
}

/// 봇을 띄움. 결과는 tx로 옴(Log·Status·Finished).
pub fn spawn_bot(
    macro_: Value,
    credentials: Credentials,
    mode: RunMode,
    server: ServerClient,
    preflight: Option<PreflightResult>,
    http: reqwest::Client,
    tx: mpsc::UnboundedSender<BotEvent>,
) -> BotHandle {
    let mut bot = Bot::new(macro_, credentials, mode, Some(server), tx, http);
    bot.preflight = preflight;
    let handle_cmd = bot.command.clone();
    let handle_wake = bot.wake.clone();
    let (quote, symbol, side) = (bot.quote, bot.symbol.clone(), bot.side);
    let join = tokio::spawn(async move { bot.run().await });
    BotHandle { command: handle_cmd, wake: handle_wake, quote, symbol, side, join }
}

impl Bot {
    pub fn new(
        macro_: Value,
        credentials: Credentials,
        mode: RunMode,
        server: Option<ServerClient>,
        tx: mpsc::UnboundedSender<BotEvent>,
        http: reqwest::Client,
    ) -> Self {
        let exchange_name = exchange_name_of(&macro_);
        let symbol = {
            let s = crate::types::macro_symbol(&macro_);
            if s.is_empty() && macro_.get("symbol").is_none() {
                "BTCUSDT".to_string()
            } else {
                s
            }
        };
        let quote = quote_of(&symbol);
        let side = macro_side(&macro_);
        let leverage = macro_leverage(&macro_).unwrap_or(1);
        let market = market_of(&exchange_name, side, leverage);
        let capital = match num(macro_.get("params").and_then(|p| p.get("initial_capital"))) {
            c if c != 0.0 => c,
            _ => order_cap(quote),
        };
        let logger = Logger::new(tx.clone(), server.clone(), credentials.clone());
        if let Some(server) = &server {
            if !server.has_event_warning() {
                // 서버 → 봇 송신자를 약하게만 붙잡음. Logger를 담으면 서버 ↔ Logger 참조 순환이 생겨
                // 봇 작업이 죽어도 채널이 닫히지 않고, 앱이 '예기치 않게 끝났어요' 정리를 하지 못함.
                let weak = tx.downgrade();
                let creds = credentials.clone();
                server.set_event_warning(Arc::new(move |m: String| {
                    let Some(tx) = weak.upgrade() else { return };
                    let msg = crate::connection::mask_secrets(&m, &creds);
                    let _ = tx.send(BotEvent::Log(msg.clone()));
                    let _ = tx.send(BotEvent::Activity(ActivityEvent {
                        kind: "connection",
                        title: "실행 기록 보관에 문제가 생겼어요".into(),
                        detail: msg.clone(),
                        raw: msg,
                        tone: "warn",
                    }));
                }));
            }
        }
        Bot {
            macro_,
            credentials,
            exchange_name,
            mode,
            server,
            tx,
            logger,
            command: Arc::new(Mutex::new(None)),
            wake: Arc::new(Notify::new()),
            http,
            symbol,
            quote,
            side,
            leverage,
            market,
            broker: None,
            broker_override: None,
            step: 0.0,
            in_position: false,
            entry_price: 0.0,
            held_qty: 0.0,
            realized: 0.0,
            fees_quote: 0.0,
            fees_complete: market == Market::Spot,
            signal_price: 0.0,
            position_uncertain: false,
            position_dust_qty: 0.0,
            capital,
            done_command_ids: VecDeque::with_capacity(200),
            pending_acks: vec![],
            close_failures: 0,
            auth_noticed: false,
            offline_logged: false,
            price_failing: false,
            offline_since: 0,
            offline_escalated: false,
            rejected_noticed: 0,
            price_fail_since: 0,
            price_escalated: false,
            price_fail_ticks: 0,
            close_retry_at: 0,
            last_tick_ms: 0,
            preflight: None,
            last_price: 0.0,
            last_fill_qty: 0.0,
            last_fill_price: 0.0,
            poll: Duration::from_secs_f64(POLL_SECONDS),
        }
    }

    fn exchange(&self) -> Option<Exchange> {
        Exchange::parse(&self.exchange_name)
    }

    /// 국내 원화 현물 거래소인지 여부 — 브로커와 키를 고른 바로 그 값으로 판단함.
    pub fn domestic(&self) -> bool {
        self.exchange().is_some_and(Exchange::is_domestic)
    }

    /// 테스트넷 주소를 쓰는지 여부. 테스트넷은 바이낸스에만 있음.
    pub fn testnet(&self) -> bool {
        self.mode == RunMode::Testnet && self.exchange().is_some_and(Exchange::has_testnet)
    }

    fn log(&self, msg: impl AsRef<str>) {
        self.logger.log(msg);
    }

    /// 실행을 막는 오류 — 원문 로그 + 활동 한 줄.
    fn fail(&self, title: &str, msg: String) {
        self.log(&msg);
        let detail = msg.trim_start_matches("[오류] ").to_string();
        self.logger.activity("error", title, detail, msg, "danger");
    }

    /// 사용자가 포지션을 확인할 곳 — 모의는 실제 계정이 없고, 테스트넷은 실전 거래소가 아님.
    fn where_to_check(&self) -> &'static str {
        match self.mode {
            RunMode::Mock => "모의 보유분이라 실제 계정에는 영향이 없어요.",
            RunMode::Testnet => "바이낸스 테스트넷에서 포지션을 확인하세요.",
            RunMode::Live => "거래소에서 포지션을 확인하세요.",
        }
    }

    /// 앱이 정리하지 못한 포지션을 누가 정리하는지 안내 — [`where_to_check`]의 '정리' 판.
    fn where_to_settle(&self) -> &'static str {
        match self.mode {
            RunMode::Mock => "모의 보유분이라 실제 계정에는 영향이 없어요.",
            RunMode::Testnet => "바이낸스 테스트넷에서 포지션을 직접 정리하세요.",
            RunMode::Live => "거래소에서 포지션을 직접 정리하세요.",
        }
    }

    fn mode_label(&self) -> &'static str {
        self.mode.label()
    }

    fn reconciliation_scope(&self) -> &'static str {
        if self.mode == RunMode::Mock {
            return "simulated_fee_model";
        }
        match (self.exchange(), self.market) {
            (Some(e), _) if e.is_domestic() => "domestic_trade_quantity_coverage",
            (Some(Exchange::Binance), Market::Spot) => "binance_spot_fill_fee_coverage",
            (Some(Exchange::Binance), Market::Futures) => "binance_futures_quantity_only",
            _ => "unknown",
        }
    }

    fn get_command(&self) -> Option<StopMode> {
        *self.command.lock().unwrap()
    }

    /// 종료 걸기. 처음 명령을 따르되 '매크로만 종료' 뒤의 '청산 후 종료'는 받음(더 안전한 쪽으로만 바꿈).
    pub fn set_command(&self, mode: StopMode) {
        let mut c = self.command.lock().unwrap();
        if c.is_none() || (*c == Some(StopMode::StopOnly) && mode == StopMode::CloseAndStop) {
            *c = Some(mode);
        }
        drop(c);
        self.wake.notify_one();
    }

    async fn sleep_for(&self, dur: Duration) {
        tokio::select! {
            _ = tokio::time::sleep(dur) => {}
            _ = self.wake.notified() => {}
        }
    }

    /// 다음 틱까지 — 하트비트·주문에 걸린 시간을 빼서 손절 확인 주기가 늘어지지 않게(너무 붙지 않게 최소 0.5초).
    async fn sleep_until_next(&self, tick: std::time::Instant) {
        let rest = self.poll.saturating_sub(tick.elapsed()).max(Duration::from_millis(500).min(self.poll));
        self.sleep_for(rest).await;
    }

    // --- 연결 ---------------------------------------------------------------
    fn broker(&self) -> Result<Arc<dyn Broker>, String> {
        self.broker.clone().ok_or_else(|| "거래소 연결이 준비되지 않았어요".to_string())
    }

    pub fn snapshot(&self) -> Value {
        let price = self.last_price;
        json!({
            "in_position": self.in_position,
            "position_uncertain": self.position_uncertain,
            "last_price": price,
            "entry_price": if self.in_position { self.entry_price } else { 0.0 },
            "position_qty": if self.in_position { self.held_qty } else { self.position_dust_qty },
            "realized_pnl": self.realized,
            "unrealized_pct": if self.in_position && price != 0.0 { pnl_pct(self.entry_price, price, self.side) } else { 0.0 },
            "note": if self.position_uncertain { "주문 결과가 불확실해요 · 거래소에서 주문과 포지션을 확인하세요.".to_string() } else { self.dust_note() },
        })
    }

    /// 화면으로만 보내는 상태 — 서버 스냅샷에 수수료(모르면 null)와 전일 대비 등락률(국내만)을 덧붙임.
    pub fn ui_snapshot(&self, mut snap: Value) -> Value {
        snap["fees_paid"] = if self.fees_complete { json!(self.fees_quote) } else { Value::Null };
        snap["day_change"] = json!(self.broker.as_ref().and_then(|b| b.day_change()));
        snap
    }

    fn dust_note(&self) -> String {
        if self.position_dust_qty != 0.0 {
            format!(
                "최소 주문 단위보다 적게 남은 수량 {} {}",
                py_gp(self.position_dust_qty, 12),
                crate::types::base_asset(&self.symbol)
            )
        } else {
            String::new()
        }
    }
}

mod commands;
mod entry_guard;
mod finish;
mod logger;
mod orders;
mod price;
mod report;
mod run_loop;
mod setup;

#[cfg(test)]
mod tests;
