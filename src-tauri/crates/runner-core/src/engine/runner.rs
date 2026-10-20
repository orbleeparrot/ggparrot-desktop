//! 매크로 하나를 엔진으로 세우고 봉을 흘려보낸다.
//!
//! 서버(파이썬 `engine/live_session.build_driver` + `command_from_fill`)가 하는 일을
//! 그대로 한다 — 매크로를 읽어 규칙과 장부를 만들고, 체결 의도를 주문 명령으로 바꾼다.
//!
//! 아직 전부는 아니다. [`build`] 가 모르는 규칙에는 [`BuildError::NotPorted`] 를 주고,
//! 판단 계약(`tests/engine_golden.rs`)이 어느 규칙까지 켜졌는지 보여 준다.

use serde_json::Value;

use super::candle_sim::{Bar, CandleSim, Risk};
use super::ledger::{is_exit, Costs, Fill, Ledger, Side};
use super::rules::breakout::{Breakout, ExitMode};
use super::rules::grid::{BandExit, Grid, GridMode};
use super::rules::indicator::IndicatorRule;
use super::rules::martingale::Martingale;
use super::rules::sar::Sar;
use super::tick_sim::{DcaSim, TickRule, TickSim};
use super::rules::signals::{
    BandExitTarget, BandStrategy, BollingerSignal, MaCrossSignal, RsiSignal,
};
use super::rules::trailing::{EntryMode, Trailing};
use super::indicators::MaType;

#[derive(Debug, Clone, PartialEq)]
pub enum BuildError {
    /// 아직 Rust 로 옮기지 않은 규칙. 서버가 판단하던 때처럼 돌릴 수는 있어도
    /// **로컬 판단으로는 못 연다** — 조용히 다르게 사는 것보다 안 여는 쪽이 낫다.
    NotPorted(String),
    /// 매크로를 읽을 수 없다.
    BadMacro(String),
}

impl std::fmt::Display for BuildError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            BuildError::NotPorted(rule) => {
                write!(f, "이 매매 방식({rule})은 실행기가 아직 직접 판단하지 못해요.")
            }
            BuildError::BadMacro(why) => write!(f, "매크로를 읽지 못했어요: {why}"),
        }
    }
}

fn num(macro_: &Value, path: &[&str]) -> Option<f64> {
    let mut node = macro_;
    for key in path {
        node = node.get(key)?;
    }
    match node {
        Value::Number(n) => n.as_f64(),
        Value::String(s) => s.parse().ok(),
        _ => None,
    }
}

fn text<'a>(macro_: &'a Value, key: &str) -> &'a str {
    macro_.get(key).and_then(Value::as_str).unwrap_or_default()
}

fn flag(macro_: &Value, path: &[&str], default: bool) -> bool {
    let mut node = macro_;
    for key in path {
        match node.get(key) {
            Some(next) => node = next,
            None => return default,
        }
    }
    node.as_bool().unwrap_or(default)
}

/// 매크로가 쓰는 자본. 서버 `session_capital` 과 같다.
pub const FALLBACK_CAPITAL: f64 = 100.0;

pub fn session_capital(macro_: &Value) -> f64 {
    num(macro_, &["params", "initial_capital"]).filter(|v| *v > 0.0).unwrap_or(FALLBACK_CAPITAL)
}

fn costs_of(macro_: &Value) -> Costs {
    Costs {
        commission_pct: num(macro_, &["fees", "commission_pct"]).unwrap_or(0.1),
        slippage_pct: num(macro_, &["fees", "slippage_pct"]).unwrap_or(0.05),
        funding_pct: num(macro_, &["fees", "funding_pct"]).unwrap_or(0.0),
        leverage: num(macro_, &["leverage"]).filter(|v| *v >= 1.0).unwrap_or(1.0),
    }
}

fn risk_of(macro_: &Value) -> Risk {
    Risk {
        invest_ratio: num(macro_, &["risk", "invest_ratio"]).filter(|v| *v > 0.0).unwrap_or(1.0),
        stop_loss_pct: num(macro_, &["risk", "stop_loss_pct"]).filter(|v| *v > 0.0),
        daily_max_loss_pct: num(macro_, &["risk", "daily_max_loss_pct"]).filter(|v| *v > 0.0),
        max_holding_hours: num(macro_, &["risk", "max_holding_hours"]).filter(|v| *v > 0.0),
        cooldown_minutes: num(macro_, &["risk", "cooldown_minutes"]).unwrap_or(0.0),
    }
}

fn side_of(macro_: &Value) -> Side {
    if text(macro_, "position_side").eq_ignore_ascii_case("short") {
        Side::Short
    } else {
        Side::Long
    }
}

/// 봉 기반 규칙 하나를 돌리는 엔진.
pub enum Engine {
    Grid(CandleSim<Grid>),
    Trailing(CandleSim<Trailing>),
    Breakout(CandleSim<Breakout>),
    Rsi(CandleSim<IndicatorRule<RsiSignal>>),
    Bollinger(CandleSim<IndicatorRule<BollingerSignal>>),
    MaCross(CandleSim<IndicatorRule<MaCrossSignal>>),
    Martingale(CandleSim<Martingale>),
    Sar(CandleSim<Sar>),
    /// A · B — 틱 기계. 봉을 넣어도 시계만 맞추고, 판단은 틱에서 한다.
    Tick(TickSim),
    /// C — 적립식. 달력이 기준이다.
    Dca(DcaSim),
}

impl Engine {
    /// 마감봉 하나. 돌려주는 값은 대기 중인 체결 수 — 주문은 [`Engine::tick`] 에서 나온다.
    ///
    /// 틱 기반 규칙(A · B · C)은 봉을 보지 않는다. 다만 **봉의 시각**은 받아 둔다 —
    /// 쿨다운 · 보유시간 · 적립식 달력이 그 시계를 본다.
    pub fn on_bar(&mut self, bar: &Bar) -> usize {
        match self {
            Engine::Grid(sim) => sim.on_bar(bar),
            Engine::Trailing(sim) => sim.on_bar(bar),
            Engine::Breakout(sim) => sim.on_bar(bar),
            Engine::Rsi(sim) => sim.on_bar(bar),
            Engine::Bollinger(sim) => sim.on_bar(bar),
            Engine::MaCross(sim) => sim.on_bar(bar),
            Engine::Martingale(sim) => sim.on_bar(bar),
            Engine::Sar(sim) => sim.on_bar(bar),
            Engine::Tick(sim) => {
                sim.clock_ms = bar.t;
                0
            }
            Engine::Dca(sim) => {
                sim.clock_ms = bar.t;
                0
            }
        }
    }

    /// 틱 기계의 시계를 지금으로 맞춘다. 실행기가 매 바퀴 부른다.
    pub fn set_clock(&mut self, now_ms: i64) {
        match self {
            Engine::Tick(sim) => sim.clock_ms = now_ms,
            Engine::Dca(sim) => sim.clock_ms = now_ms,
            _ => {}
        }
    }

    /// 현재가 한 틱 — 체결 하나.
    pub fn tick(&mut self, price: f64) -> Option<Fill> {
        match self {
            Engine::Grid(sim) => sim.tick(price),
            Engine::Trailing(sim) => sim.tick(price),
            Engine::Breakout(sim) => sim.tick(price),
            Engine::Rsi(sim) => sim.tick(price),
            Engine::Bollinger(sim) => sim.tick(price),
            Engine::MaCross(sim) => sim.tick(price),
            Engine::Martingale(sim) => sim.tick(price),
            Engine::Sar(sim) => sim.tick(price),
            Engine::Tick(sim) => {
                let now = sim.clock_ms;
                sim.step(price, now)
            }
            Engine::Dca(sim) => {
                let now = sim.clock_ms;
                sim.step(price, now)
            }
        }
    }

    /// 웜업이 끝났다 — 지표는 두고 장부만 되돌린다.
    pub fn warmup_done(&mut self) {
        match self {
            Engine::Grid(sim) => sim.warmup_done(),
            Engine::Trailing(sim) => sim.warmup_done(),
            Engine::Breakout(sim) => sim.warmup_done(),
            Engine::Rsi(sim) => sim.warmup_done(),
            Engine::Bollinger(sim) => sim.warmup_done(),
            Engine::MaCross(sim) => sim.warmup_done(),
            Engine::Martingale(sim) => sim.warmup_done(),
            Engine::Sar(sim) => sim.warmup_done(),
            Engine::Tick(sim) => sim.warmup_done(),
            Engine::Dca(sim) => sim.warmup_done(),
        }
    }

    /// 지금 가격 기준 자산. 명령이 같아도 이 값이 다르면 장부 식이 어긋난 것이다.
    pub fn equity(&self, price: f64) -> f64 {
        match self {
            Engine::Grid(sim) => sim.ledger.equity(price),
            Engine::Trailing(sim) => sim.ledger.equity(price),
            Engine::Breakout(sim) => sim.ledger.equity(price),
            Engine::Rsi(sim) => sim.ledger.equity(price),
            Engine::Bollinger(sim) => sim.ledger.equity(price),
            Engine::MaCross(sim) => sim.ledger.equity(price),
            Engine::Martingale(sim) => sim.ledger.equity(price),
            Engine::Sar(sim) => sim.ledger.equity(price),
            Engine::Tick(sim) => sim.equity(price),
            Engine::Dca(sim) => sim.equity(price),
        }
    }

    /// 지금 보유 중인가.
    pub fn in_position(&self) -> bool {
        match self {
            Engine::Grid(sim) => sim.ledger.in_position(),
            Engine::Trailing(sim) => sim.ledger.in_position(),
            Engine::Breakout(sim) => sim.ledger.in_position(),
            Engine::Rsi(sim) => sim.ledger.in_position(),
            Engine::Bollinger(sim) => sim.ledger.in_position(),
            Engine::MaCross(sim) => sim.ledger.in_position(),
            Engine::Martingale(sim) => sim.ledger.in_position(),
            Engine::Sar(sim) => sim.ledger.in_position(),
            Engine::Tick(sim) => sim.in_position,
            Engine::Dca(sim) => sim.qty > 0.0,
        }
    }
}

/// 매크로에서 엔진을 세운다. 모르는 규칙이면 [`BuildError::NotPorted`].
pub fn build(macro_: &Value) -> Result<Engine, BuildError> {
    let rule = text(macro_, "rule_type").to_ascii_uppercase();
    if rule.is_empty() {
        return Err(BuildError::BadMacro("rule_type 이 없어요".into()));
    }
    let capital = session_capital(macro_);
    let ledger = Ledger::new(side_of(macro_), costs_of(macro_), capital);
    let risk = risk_of(macro_);
    let p = |key: &str| num(macro_, &["params", key]);

    match rule.as_str() {
        "E" => {
            let entry_mode =
                if macro_.pointer("/params/entry_mode").and_then(Value::as_str) == Some("dip") {
                    EntryMode::Dip
                } else {
                    EntryMode::Immediate
                };
            let trail = p("trail_percent")
                .ok_or_else(|| BuildError::BadMacro("E: trail_percent 가 없어요".into()))?;
            let strategy = Trailing::new(
                entry_mode,
                p("entry_dip").unwrap_or(3.0),
                p("activation_profit").unwrap_or(5.0),
                trail,
                flag(macro_, &["params", "reenter_after_exit"], true),
                risk.invest_ratio,
            );
            Ok(Engine::Trailing(CandleSim::new(ledger, risk, strategy)))
        }
        "D" => {
            let need = |key: &str| {
                p(key).ok_or_else(|| BuildError::BadMacro(format!("D: {key} 가 없어요")))
            };
            let strategy = Grid::new(
                need("lower_price")?,
                need("upper_price")?,
                need("grid_count")? as usize,
                if macro_.pointer("/params/grid_mode").and_then(Value::as_str) == Some("geometric") {
                    GridMode::Geometric
                } else {
                    GridMode::Arithmetic
                },
                if macro_.pointer("/params/band_exit_action").and_then(Value::as_str) == Some("hold")
                {
                    BandExit::Hold
                } else {
                    BandExit::Stop
                },
                p("per_grid_invest"),
                risk.invest_ratio,
                capital,
            );
            Ok(Engine::Grid(CandleSim::new(ledger, risk, strategy)))
        }
        "I" => {
            let exit_mode = match macro_.pointer("/params/exit_mode").and_then(Value::as_str) {
                Some("trailing") => ExitMode::Trailing,
                Some("take_profit") => ExitMode::TakeProfit,
                _ => ExitMode::NextOpen,
            };
            let strategy = Breakout::new(
                p("k").unwrap_or(0.5),
                exit_mode,
                p("trail_percent").unwrap_or(2.0),
                p("take_profit"),
                p("ma_filter_period").map(|v| v as usize),
                risk.invest_ratio,
            );
            Ok(Engine::Breakout(CandleSim::new(ledger, risk, strategy)))
        }
        "F" => {
            // exit_mode: indicator | take_profit | both — 지표로 파는 것은 앞뒤 둘뿐이다.
            let exit_mode =
                macro_.pointer("/params/exit_mode").and_then(Value::as_str).unwrap_or("indicator");
            let signal = RsiSignal::new(
                p("rsi_period").unwrap_or(14.0) as usize,
                p("entry_threshold").unwrap_or(30.0),
                p("exit_threshold").unwrap_or(70.0),
                p("confirm_candles").unwrap_or(1.0) as u32,
                matches!(exit_mode, "indicator" | "both"),
            );
            // 익절은 exit_mode 가 take_profit · both 일 때만 쓴다.
            let tp = p("take_profit").filter(|_| matches!(exit_mode, "take_profit" | "both"));
            Ok(Engine::Rsi(CandleSim::new(
                ledger,
                risk,
                IndicatorRule::new(signal, tp, risk.invest_ratio),
            )))
        }
        "G" => {
            let signal = BollingerSignal::new(
                p("bb_period").unwrap_or(20.0) as usize,
                p("bb_std").unwrap_or(2.0),
                if macro_.pointer("/params/strategy").and_then(Value::as_str) == Some("breakout") {
                    BandStrategy::Breakout
                } else {
                    BandStrategy::Reversion
                },
                if macro_.pointer("/params/exit_target").and_then(Value::as_str) == Some("opposite")
                {
                    BandExitTarget::Opposite
                } else {
                    BandExitTarget::Mid
                },
                flag(macro_, &["params", "squeeze_filter"], false),
                p("squeeze_lookback").unwrap_or(50.0) as usize,
            );
            Ok(Engine::Bollinger(CandleSim::new(
                ledger,
                risk,
                IndicatorRule::new(signal, None, risk.invest_ratio),
            )))
        }
        "J" => {
            let exit_signal = macro_
                .pointer("/params/exit_signal")
                .and_then(Value::as_str)
                .unwrap_or("dead_cross");
            let signal = MaCrossSignal::new(
                if macro_.pointer("/params/ma_type").and_then(Value::as_str) == Some("EMA") {
                    MaType::Ema
                } else {
                    MaType::Sma
                },
                p("fast_period")
                    .ok_or_else(|| BuildError::BadMacro("J: fast_period 가 없어요".into()))?
                    as usize,
                p("slow_period")
                    .ok_or_else(|| BuildError::BadMacro("J: slow_period 가 없어요".into()))?
                    as usize,
                matches!(exit_signal, "dead_cross" | "both"),
                p("confirm_candles").unwrap_or(1.0) as u32,
            );
            let tp = p("take_profit").filter(|_| matches!(exit_signal, "take_profit" | "both"));
            Ok(Engine::MaCross(CandleSim::new(
                ledger,
                risk,
                IndicatorRule::new(signal, tp, risk.invest_ratio),
            )))
        }
        "H" => {
            let need = |key: &str| {
                p(key).ok_or_else(|| BuildError::BadMacro(format!("H: {key} 가 없어요")))
            };
            let strategy = Martingale::new(
                need("base_order_size")?,
                need("safety_order_size")?,
                need("price_deviation")?,
                p("safety_order_step_scale").unwrap_or(1.0),
                p("safety_order_volume_scale").unwrap_or(1.0),
                p("max_safety_orders").unwrap_or(5.0) as u32,
                need("take_profit")?,
            );
            Ok(Engine::Martingale(CandleSim::new(ledger, risk, strategy)))
        }
        "K" => {
            let need = |key: &str| {
                p(key).ok_or_else(|| BuildError::BadMacro(format!("K: {key} 가 없어요")))
            };
            let strategy = Sar::new(
                p("long_take_profit_pct"),
                need("drop_trigger_pct")?,
                p("partial_exit_pct").unwrap_or(50.0),
                flag(macro_, &["params", "flip_to_short"], true),
                need("short_take_profit_pct")?,
                need("short_stop_loss_pct")?,
                flag(macro_, &["params", "reenter_long_after"], true),
                risk.invest_ratio,
            );
            // 공통 손절선은 끈다 — 방어 발동이 롱 구간의 손절이고, 숏 구간은 자기 손절선을
            // 따로 갖는다. 둘을 같이 걸면 방어가 뜨기 전에 손절이 먼저 나가 전환이 안 일어난다.
            let mut risk = risk;
            risk.stop_loss_pct = None;
            Ok(Engine::Sar(CandleSim::new(ledger, risk, strategy)))
        }
        "A" | "B" => {
            let tick_rule =
                if rule == "A" { TickRule::TakeProfitReentry } else { TickRule::Band };
            let mut sim =
                TickSim::new(side_of(macro_), costs_of(macro_), risk, tick_rule, capital);
            if tick_rule == TickRule::Band {
                sim.buy_price = p("buy_price")
                    .ok_or_else(|| BuildError::BadMacro("B: buy_price 가 없어요".into()))?;
                sim.sell_price = p("sell_price")
                    .ok_or_else(|| BuildError::BadMacro("B: sell_price 가 없어요".into()))?;
            } else {
                sim.take_profit_pct = p("take_profit_pct")
                    .ok_or_else(|| BuildError::BadMacro("A: take_profit_pct 가 없어요".into()))?;
            }
            Ok(Engine::Tick(sim))
        }
        "C" => Ok(Engine::Dca(DcaSim::new(
            costs_of(macro_),
            risk.stop_loss_pct,
            risk.daily_max_loss_pct,
            p("amount_per_buy")
                .ok_or_else(|| BuildError::BadMacro("C: amount_per_buy 가 없어요".into()))?,
            p("interval_days").unwrap_or(1.0) as i64,
            capital,
        ))),
        other => Err(BuildError::NotPorted(other.to_string())),
    }
}

/// 체결 의도 하나를 주문 명령으로. 서버 `command_from_fill` 과 **식까지 같다**.
///
/// 진입은 **초기자본 대비 금액 비율**, 청산은 **직전 보유 수량 대비 비율**(모르면 전량).
/// 기준이 다른 이유: 진입 때는 실제 보유가 없고, 청산 때는 실계좌 수량이 장부와 조금씩
/// 다를 수 있어 비율로 말해야 실행기가 자기 수량으로 환산할 수 있다.
#[derive(Debug, Clone, PartialEq)]
pub struct Command {
    pub action: &'static str,
    pub notional_frac: f64,
    pub qty_frac: f64,
    pub signal_price: f64,
    pub reason: String,
}

pub fn command_from_fill(fill: &Fill, initial: f64) -> Command {
    if is_exit(fill.side) {
        let mut frac =
            if fill.qty_before <= 0.0 { 1.0 } else { (fill.qty / fill.qty_before).min(1.0) };
        if frac >= 0.999 {
            frac = 1.0;
        }
        return Command {
            action: fill.side,
            notional_frac: 0.0,
            qty_frac: frac,
            signal_price: fill.price,
            reason: fill.reason.clone(),
        };
    }
    let notional = fill.qty * fill.price;
    Command {
        action: fill.side,
        notional_frac: if initial > 0.0 { notional / initial } else { 0.0 },
        qty_frac: 0.0,
        signal_price: fill.price,
        reason: fill.reason.clone(),
    }
}
