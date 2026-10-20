//! 매크로 하나를 엔진으로 세우고 봉을 흘려보낸다.
//!
//! 서버(파이썬 `engine/live_session.build_driver` + `command_from_fill`)가 하는 일을
//! 그대로 한다 — 매크로를 읽어 규칙과 장부를 만들고, 체결 의도를 주문 명령으로 바꾼다.
//!
//! 아직 전부는 아니다. [`build`] 가 모르는 규칙에는 [`BuildError::NotPorted`] 를 주고,
//! 판단 계약(`tests/engine_golden.rs`)이 어느 규칙까지 켜졌는지 보여 준다.

use serde_json::Value;

use super::candle_sim::{Bar, CandleSim, Risk, Strategy};
use super::ledger::{is_exit, Costs, Fill, Ledger, Side};
use super::rules::breakout::{Breakout, ExitMode};
use super::rules::grid::{BandExit, Grid, GridMode};
use super::rules::trailing::{EntryMode, Trailing};

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
}

macro_rules! dispatch {
    ($self:ident, $sim:ident => $body:expr) => {
        match $self {
            Engine::Grid($sim) => $body,
            Engine::Trailing($sim) => $body,
            Engine::Breakout($sim) => $body,
        }
    };
}

impl Engine {
    /// 마감봉 하나. 돌려주는 값은 대기 중인 체결 수 — 주문은 [`Engine::tick`] 에서 나온다.
    pub fn on_bar(&mut self, bar: &Bar) -> usize {
        dispatch!(self, sim => sim.on_bar(bar))
    }

    /// 현재가 한 틱 — 대기 중인 체결 하나.
    pub fn tick(&mut self, price: f64) -> Option<Fill> {
        dispatch!(self, sim => sim.tick(price))
    }

    /// 웜업이 끝났다 — 지표는 두고 장부만 되돌린다.
    pub fn warmup_done(&mut self) {
        dispatch!(self, sim => sim.warmup_done())
    }

    pub fn ledger(&self) -> &Ledger {
        dispatch!(self, sim => &sim.ledger)
    }

    pub fn pending(&self) -> usize {
        dispatch!(self, sim => sim.pending())
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
