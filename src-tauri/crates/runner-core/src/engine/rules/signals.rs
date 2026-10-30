//! 지표 판정 셋 — **F**(RSI) · **G**(볼린저) · **J**(이동평균 교차).
//!
//! 셋 다 [`super::indicator::Signal`] 하나만 구현한다. "이 종가를 보니 다음 봉에서
//! 무엇을 할까" 뿐이고, 실행 · 익절 · 공통 위험은 [`super::indicator::IndicatorRule`] 이 맡는다.
//!
//! 숏 매크로에서는 조건이 **거울처럼 뒤집힌다** — F 는 과매수에서 들어가고 과매도에서
//! 나오며, J 는 데드크로스에서 들어간다.

use crate::engine::candle_sim::Bar;
use crate::engine::indicators::{Bollinger, Ma, MaType, Rsi};
use crate::engine::ledger::Side;
use crate::engine::series::{Oscillator, Series, bollinger_series, ma_series, rsi_oscillator};
use crate::money::{py_g, py_gp};

use super::indicator::{Intent, Pending, Signal};
use crate::engine::levels::{Indicator, Levels, price};

/// 몇 봉 연속으로 조건이 맞아야 믿을 것인가. 1 이면 한 봉만 맞아도 된다.
fn streak(hit: bool, current: u32) -> u32 {
    if hit { current + 1 } else { 0 }
}

// --- F: RSI 문턱 ------------------------------------------------------------

/// **F — RSI.** 과매도에서 사고 과매수에서 판다(숏은 반대).
#[derive(Debug, Clone)]
pub struct RsiSignal {
    rsi: Rsi,
    /// 롱이 사는 문턱(아래로 내려오면).
    pub entry_threshold: f64,
    /// 롱이 파는 문턱(위로 올라가면).
    pub exit_threshold: f64,
    /// 몇 봉 연속이어야 믿을 것인가.
    pub confirm: u32,
    /// 지표로 팔 것인가 — `indicator` · `both` 만 지표 청산을 쓴다.
    pub exit_by_indicator: bool,
    entry_streak: u32,
    exit_streak: u32,
    last: Option<f64>,
}

impl RsiSignal {
    pub fn new(
        period: usize,
        entry_threshold: f64,
        exit_threshold: f64,
        confirm: u32,
        exit_by_indicator: bool,
    ) -> Self {
        Self {
            rsi: Rsi::new(period),
            entry_threshold,
            exit_threshold,
            confirm: confirm.max(1),
            exit_by_indicator,
            entry_streak: 0,
            exit_streak: 0,
            last: None,
        }
    }

    /// 마지막 RSI 값 — 화면에 보여 준다.
    pub fn value(&self) -> Option<f64> {
        self.last
    }
}

impl Signal for RsiSignal {
    fn evaluate(&mut self, close: f64, in_position: bool, side: Side, pending: &mut Pending) {
        let Some(v) = self.rsi.update(close) else { return };
        self.last = Some(v);
        let short = side == Side::Short;
        let entry_hit = if short { v >= self.exit_threshold } else { v <= self.entry_threshold };
        let exit_hit = if short { v <= self.entry_threshold } else { v >= self.exit_threshold };
        self.entry_streak = streak(entry_hit, self.entry_streak);
        self.exit_streak = streak(exit_hit, self.exit_streak);

        if !in_position {
            if self.entry_streak >= self.confirm {
                let threshold = if short { self.exit_threshold } else { self.entry_threshold };
                pending.note = format!("RSI {v:.1} {} {}", if short { "≥" } else { "≤" }, py_g(threshold));
                pending.intent = Some(Intent::Enter);
            }
        } else if self.exit_by_indicator && self.exit_streak >= self.confirm {
            let threshold = if short { self.entry_threshold } else { self.exit_threshold };
            pending.note = format!("RSI {v:.1} {} {}", if short { "≤" } else { "≥" }, py_g(threshold));
            pending.intent = Some(Intent::Exit);
        }
    }

    fn on_warmup_reset(&mut self) {}

    fn levels(&self) -> Levels {
        // RSI 는 0~100 지표라 그릴 **가격** 선이 없다 — 가격축에 섞으면 축이 망가진다.
        // 대신 값과 두 문턱을 따로 실어, 화면이 아래 칸에 그리고 "지금 조건에 얼마나
        // 가까운가" 를 보여 줄 수 있게 한다.
        match self.last {
            Some(value) => Levels {
                indicator: Some(Indicator {
                    name: "RSI",
                    value,
                    entry_threshold: Some(self.entry_threshold),
                    exit_threshold: Some(self.exit_threshold),
                }),
                ..Default::default()
            },
            None => Levels::pending(),
        }
    }

    fn oscillator(&self, bars: &[Bar]) -> Option<Oscillator> {
        // RSI 는 0~100 이라 가격축에 섞으면 축이 망가진다 — 아래 칸에 따로 그린다.
        Some(rsi_oscillator(&self.rsi, bars, Some(self.entry_threshold), Some(self.exit_threshold)))
    }
}

// --- G: 볼린저 밴드 ---------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BandStrategy {
    /// 밴드 밖으로 벗어나면 **되돌아올 것**으로 보고 산다.
    Reversion,
    /// 밴드를 뚫으면 **그 방향으로 갈 것**으로 보고 산다.
    Breakout,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BandExitTarget {
    /// 중심선에 닿으면 판다.
    Mid,
    /// 반대쪽 밴드에 닿으면 판다.
    Opposite,
}

/// **G — 볼린저 밴드.**
#[derive(Debug, Clone)]
pub struct BollingerSignal {
    bands: Bollinger,
    pub strategy: BandStrategy,
    pub exit_target: BandExitTarget,
    /// 밴드가 **좁아져 있을 때만** 사는가(변동성 수축 뒤의 움직임을 노린다).
    pub squeeze_filter: bool,
    squeeze_window: std::collections::VecDeque<f64>,
    squeeze_lookback: usize,
    last: Option<(f64, f64, f64)>,
}

impl BollingerSignal {
    pub fn new(
        period: usize,
        num_std: f64,
        strategy: BandStrategy,
        exit_target: BandExitTarget,
        squeeze_filter: bool,
        squeeze_lookback: usize,
    ) -> Self {
        Self {
            bands: Bollinger::new(period, num_std),
            strategy,
            exit_target,
            squeeze_filter,
            squeeze_window: Default::default(),
            squeeze_lookback: squeeze_lookback.max(2),
            last: None,
        }
    }

    /// 마지막 `(중심, 위, 아래)` — 화면에 그리는 선이다.
    pub fn value(&self) -> Option<(f64, f64, f64)> {
        self.last
    }
}

impl Signal for BollingerSignal {
    fn evaluate(&mut self, close: f64, in_position: bool, side: Side, pending: &mut Pending) {
        let Some((mid, upper, lower)) = self.bands.update(close) else { return };
        self.last = Some((mid, upper, lower));
        let width = if mid != 0.0 { (upper - lower) / mid } else { 0.0 };
        if self.squeeze_window.len() == self.squeeze_lookback {
            self.squeeze_window.pop_front();
        }
        self.squeeze_window.push_back(width);
        // 지금 폭이 최근 중 가장 좁은 축에 드는가(5% 여유).
        let squeezed = if self.squeeze_filter && self.squeeze_window.len() >= 2 {
            let min = self.squeeze_window.iter().cloned().fold(f64::INFINITY, f64::min);
            width <= min * 1.05
        } else {
            true
        };
        let short = side == Side::Short;

        if !in_position {
            let hit = match (self.strategy, short) {
                (BandStrategy::Reversion, false) => close <= lower,
                (BandStrategy::Reversion, true) => close >= upper,
                (BandStrategy::Breakout, false) => close >= upper,
                (BandStrategy::Breakout, true) => close <= lower,
            };
            if hit && squeezed {
                let level = if short { upper } else { lower };
                pending.note = format!("볼린저 {} {} 이탈", if short { "상단" } else { "하단" }, py_gp(level, 4));
                pending.intent = Some(Intent::Enter);
            }
            return;
        }

        let exit_hit = match (self.exit_target, short) {
            (BandExitTarget::Mid, false) => close >= mid,
            (BandExitTarget::Mid, true) => close <= mid,
            (BandExitTarget::Opposite, false) => close >= upper,
            (BandExitTarget::Opposite, true) => close <= lower,
        };
        if exit_hit {
            let (label, level) = match (self.exit_target, short) {
                (BandExitTarget::Mid, _) => ("중심", mid),
                (BandExitTarget::Opposite, false) => ("상단", upper),
                (BandExitTarget::Opposite, true) => ("하단", lower),
            };
            pending.note = format!("볼린저 {label} {} 도달", py_gp(level, 4));
            pending.intent = Some(Intent::Exit);
        }
    }

    fn levels(&self) -> Levels {
        // 밴드 셋을 그대로 그린다 — 중심 · 위 · 아래.
        match self.value() {
            Some((mid, upper, lower)) => Levels {
                reference: price(mid),
                band_upper: price(upper),
                band_lower: price(lower),
                ..Default::default()
            },
            None => Levels::pending(),
        }
    }

    fn series(&self, bars: &[Bar]) -> Vec<Series> {
        bollinger_series(&self.bands, bars)
    }
}

// --- J: 이동평균 교차 -------------------------------------------------------

/// **J — 이동평균 교차.** 짧은 평균이 긴 평균을 위로 뚫으면(골든크로스) 산다.
#[derive(Debug, Clone)]
pub struct MaCrossSignal {
    fast: Ma,
    slow: Ma,
    /// 지표로 팔 것인가 — `dead_cross` · `both` 만 지표 청산을 쓴다.
    pub exit_by_indicator: bool,
    pub confirm: u32,
    golden_streak: u32,
    dead_streak: u32,
    last: Option<(f64, f64)>,
}

impl MaCrossSignal {
    pub fn new(kind: MaType, fast_period: usize, slow_period: usize, exit_by_indicator: bool, confirm: u32) -> Self {
        Self {
            fast: Ma::new(kind, fast_period),
            slow: Ma::new(kind, slow_period),
            exit_by_indicator,
            confirm: confirm.max(1),
            golden_streak: 0,
            dead_streak: 0,
            last: None,
        }
    }

    /// 마지막 `(짧은 평균, 긴 평균)` — 화면에 그리는 두 선이다.
    pub fn value(&self) -> Option<(f64, f64)> {
        self.last
    }
}

impl Signal for MaCrossSignal {
    fn evaluate(&mut self, close: f64, in_position: bool, side: Side, pending: &mut Pending) {
        let fast = self.fast.update(close);
        let slow = self.slow.update(close);
        let (Some(fast), Some(slow)) = (fast, slow) else { return };
        self.last = Some((fast, slow));
        let diff = fast - slow;
        // 교차한 **순간**이 아니라 '짧은 평균이 위에 있는 상태' 가 몇 봉 이어졌는지를 센다.
        self.golden_streak = streak(diff > 0.0, self.golden_streak);
        self.dead_streak = streak(diff < 0.0, self.dead_streak);
        let short = side == Side::Short;
        let enter_now = if short { self.dead_streak >= self.confirm } else { self.golden_streak >= self.confirm };
        let exit_now = if short { self.golden_streak >= self.confirm } else { self.dead_streak >= self.confirm };

        if !in_position {
            if enter_now {
                pending.note = if short { "데드크로스".to_string() } else { "골든크로스".to_string() };
                pending.intent = Some(Intent::Enter);
            }
        } else if self.exit_by_indicator && exit_now {
            pending.note = if short { "골든크로스".to_string() } else { "데드크로스".to_string() };
            pending.intent = Some(Intent::Exit);
        }
    }

    fn levels(&self) -> Levels {
        // 두 이동평균을 그린다. 교차가 신호이므로 둘이 같이 보여야 뜻이 있다.
        match self.value() {
            Some((fast, slow)) => Levels { reference: price(fast), band_upper: price(slow), ..Default::default() },
            None => Levels::pending(),
        }
    }

    fn series(&self, bars: &[Bar]) -> Vec<Series> {
        vec![
            ma_series(&self.fast, bars, "ma_fast", format!("단기 {}", self.fast.period())),
            ma_series(&self.slow, bars, "ma_slow", format!("장기 {}", self.slow.period())),
        ]
    }
}
