//! **D — 그리드.** 정해진 가격대를 칸으로 나눠, 내려오면 사고 한 칸 오르면 판다.
//!
//! 칸마다 **독립**이다 — 2번 칸에서 산 물량은 3번 칸에 닿으면 그것만 팔고, 다른 칸은
//! 그대로 둔다. 그래서 장부도 칸별 묶음으로 들고 있는다(번호로 기억한다 — 자리로 기억하면
//! 앞 묶음이 팔릴 때마다 가리키는 곳이 밀린다).
//!
//! 가격대를 벗어나면(`band_exit_action`):
//! * `stop` — 전부 팔고 규칙을 멈춘다. 가격대를 벗어난 그리드는 더 이상 그 가격대가 아니다.
//! * `hold` — 새로 사지 않고 들고 있는다.
//!
//! 한 봉이 여러 칸을 지나는 일은 실제로 있다(급락). 그때는 그 봉에서 여러 칸을 한꺼번에 산다.

use std::collections::BTreeMap;

use crate::engine::candle_sim::{Bar, SimContext, Strategy};
use crate::engine::levels::Levels;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BandExit {
    /// 가격대를 벗어나면 전부 팔고 멈춘다.
    Stop,
    /// 가격대를 벗어나면 새로 사지 않고 들고 있는다.
    Hold,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GridMode {
    /// 같은 **금액** 간격.
    Arithmetic,
    /// 같은 **비율** 간격 — 낮은 쪽이 촘촘해진다.
    Geometric,
}

#[derive(Debug, Clone)]
pub struct Grid {
    pub lower: f64,
    pub upper: f64,
    /// 칸 경계들. 칸이 `n` 개면 경계는 `n + 1` 개다.
    pub levels: Vec<f64>,
    pub band_exit: BandExit,
    /// 칸 하나에 넣는 자기 돈.
    pub per_grid: f64,
    /// 칸 번호 → 그 칸에서 산 묶음의 번호. 한 칸 위에 닿으면 그 묶음만 판다.
    holdings: BTreeMap<usize, u64>,
}

impl Grid {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        lower: f64,
        upper: f64,
        grid_count: usize,
        mode: GridMode,
        band_exit: BandExit,
        per_grid_invest: Option<f64>,
        invest_ratio: f64,
        initial_capital: f64,
    ) -> Self {
        let n = grid_count.max(1);
        let levels: Vec<f64> = match mode {
            GridMode::Geometric => {
                let r = (upper / lower).powf(1.0 / n as f64);
                (0..=n).map(|i| lower * r.powi(i as i32)).collect()
            }
            GridMode::Arithmetic => {
                let step = (upper - lower) / n as f64;
                (0..=n).map(|i| lower + step * i as f64).collect()
            }
        };
        let budget = invest_ratio * initial_capital;
        let per_grid = per_grid_invest.filter(|v| *v > 0.0).unwrap_or(budget / n as f64);
        Self { lower, upper, levels, band_exit, per_grid, holdings: BTreeMap::new() }
    }

    /// 지금 사려고 기다리는 칸들의 가격. 화면에 그리는 선이다.
    pub fn open_buy_levels(&self) -> Vec<f64> {
        (0..self.levels.len().saturating_sub(1))
            .filter(|i| !self.holdings.contains_key(i))
            .map(|i| self.levels[i])
            .collect()
    }

    /// 지금 팔려고 기다리는 칸들의 가격.
    pub fn open_sell_levels(&self) -> Vec<f64> {
        self.holdings.keys().map(|i| self.levels[i + 1]).collect()
    }

    pub fn held_levels(&self) -> usize {
        self.holdings.len()
    }
}

impl Strategy for Grid {
    fn on_bar(&mut self, ctx: &mut SimContext<'_>, bar: &Bar) {
        // 손절 · 보유시간 · 한도는 **장부 전체**를 비울 수 있다 — 그러면 칸 기억도 함께 버린다.
        if ctx.common_risk(bar) {
            self.holdings.clear();
            return;
        }

        // 가격대를 벗어났는가.
        if bar.c > self.upper || bar.c < self.lower {
            if self.band_exit == BandExit::Stop {
                ctx.close_all(bar.c, bar.c, false, "");
                self.holdings.clear();
                ctx.stop_rule();
            }
            // hold: 새로 사지 않고 들고 있는다.
            return;
        }
        if ctx.is_stopped() {
            return;
        }

        // 팔기 — 들고 있는 칸 중 **한 칸 위**에 고가가 닿은 것.
        // 낮은 칸부터 본다(서버와 같은 순서 — 체결 순서가 기록에 그대로 남는다).
        let ready: Vec<usize> = self.holdings.keys().copied().filter(|i| bar.h >= self.levels[i + 1]).collect();
        for level in ready {
            let sell_price = self.levels[level + 1];
            if let Some(lot_id) = self.holdings.remove(&level) {
                let reason = format!("격자 {} 도달 · 매도", crate::money::py_g(sell_price));
                ctx.close_lot(lot_id, sell_price, bar.c, &reason);
            }
        }

        // 사기 — 아직 안 산 칸 중 저가가 닿은 것.
        if ctx.entry_blocked() {
            return;
        }
        for level in 0..self.levels.len() - 1 {
            if self.holdings.contains_key(&level) {
                continue;
            }
            let buy_price = self.levels[level];
            if bar.l <= buy_price
                && buy_price <= self.upper
                && ctx.cash() >= self.per_grid
                && let Some(lot_id) = ctx.open_lot(self.per_grid, buy_price, bar.c, "진입")
            {
                self.holdings.insert(level, lot_id);
            }
        }
    }

    fn levels(&self, _avg_entry: f64, _last_bar: Option<&Bar>) -> Levels {
        // 칸마다 독립이라 선이 여러 개다 — 사려고 기다리는 칸과 팔려고 기다리는 칸.
        Levels { buy_levels: self.open_buy_levels(), sell_levels: self.open_sell_levels(), ..Default::default() }
    }

    fn on_warmup_reset(&mut self) {
        self.holdings.clear();
    }
}
