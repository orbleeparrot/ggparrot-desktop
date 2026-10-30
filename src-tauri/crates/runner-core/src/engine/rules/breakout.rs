//! **I — 변동성 돌파(Larry Williams).** "올라가기 시작한 걸 확인하고 올라탐."
//!
//! 싸게 사서 비싸게 파는 방식이 아님. 지금 가격이 바닥인지 더 떨어질지는 알 수 없으므로,
//! **이번 봉의 시가 + 직전 봉 변동폭 × k**를 기준선으로 잡아 두고 그 선을 뚫고 올라오면
//! 삼. 바닥은 못 잡지만 떨어지는 칼도 잡지 않음.
//!
//! 이동평균 필터(`ma_filter_period`)를 켜면 **종가가 그 평균 위**일 때만 삼 — 내려가는
//! 흐름에서 잠깐 튀어오른 것을 거르기 위해서임. 두 조건이 **모두** 맞아야 진입함.
//!
//! 파는 방식은 셋 중 하나임:
//! * `next_open` — 다음 봉 시가에 무조건 팖(하루짜리 돌파의 원형).
//! * `trailing` — 고점 대비 `trail_percent`% 밀리면 팖.
//! * `take_profit` — 평단 대비 `take_profit`% 오르면 팖.

use crate::engine::candle_sim::{Bar, SimContext, Strategy};
use crate::engine::indicators::{Ma, MaType};
use crate::engine::levels::{Levels, price};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExitMode {
    /// 다음 봉 시가에 팖.
    NextOpen,
    /// 고점 대비 밀리면 팖.
    Trailing,
    /// 평단 대비 오르면 팖.
    TakeProfit,
}

#[derive(Debug, Clone)]
pub struct Breakout {
    /// 직전 봉 변동폭에 곱하는 값. 클수록 더 확실한 돌파만 잡음.
    pub k: f64,
    pub exit_mode: ExitMode,
    pub trail_percent: f64,
    pub take_profit: Option<f64>,
    pub invest_ratio: f64,

    /// 이동평균 필터. 없으면 조건이 하나(돌파선)뿐임.
    ma: Option<Ma>,
    /// 직전 봉까지의 이동평균 값.
    ma_value: Option<f64>,
    /// 직전 봉의 고가 − 저가. 돌파선의 재료임.
    pub(crate) prev_range: Option<f64>,
    /// 보유 중 고점(트레일용).
    peak: f64,
    /// 다음 틱에 팔기로 예약했는가(`next_open`).
    exit_next_open: bool,
}

impl Breakout {
    pub fn new(
        k: f64,
        exit_mode: ExitMode,
        trail_percent: f64,
        take_profit: Option<f64>,
        ma_filter_period: Option<usize>,
        invest_ratio: f64,
    ) -> Self {
        Self {
            k,
            exit_mode,
            trail_percent,
            take_profit,
            invest_ratio,
            ma: ma_filter_period.filter(|p| *p > 0).map(|p| Ma::new(MaType::Sma, p)),
            ma_value: None,
            prev_range: None,
            peak: 0.0,
            exit_next_open: false,
        }
    }

    /// 지금 노리는 돌파선(= 사는 선). 화면에 그리는 값임. 직전 봉을 아직 못 봤으면 없음.
    pub fn entry_target(&self, bar_open: f64) -> Option<f64> {
        self.prev_range.map(|range| bar_open + self.k * range)
    }

    /// 이동평균 필터가 지금 허락하는지. 필터가 없으면 늘 허락.
    pub fn ma_allows(&self, close: f64) -> bool {
        match self.ma_value {
            None => true,
            Some(ma) => close >= ma,
        }
    }

    pub fn ma_value(&self) -> Option<f64> {
        self.ma_value
    }

    /// 차트에 겹쳐 그릴 선 — 봉마다의 돌파선과, 필터가 있으면 이동평균선.
    ///
    /// 돌파선은 봉마다 다름(`그 봉 시가 + k x 직전 봉 변동폭`). 지금 봉의 선 하나만
    /// 가로로 그으면 "이 값이 늘 기준이었다"로 읽힘 — 실제로는 봉마다 새로 잡힘.
    /// 첫 봉은 직전 봉이 없어 비움.
    pub fn chart_series(&self, bars: &[crate::engine::candle_sim::Bar]) -> Vec<crate::engine::series::Series> {
        use crate::engine::series::Series;
        let mut targets = Vec::with_capacity(bars.len());
        for (i, bar) in bars.iter().enumerate() {
            targets.push(i.checked_sub(1).map(|j| bar.o + self.k * (bars[j].h - bars[j].l)));
        }
        let mut out =
            vec![Series {
                id: "breakout", label: format!("돌파선 k={}", crate::money::py_g(self.k)), values: targets
            }];
        if let Some(ma) = &self.ma {
            out.push(crate::engine::series::ma_series(ma, bars, "ma_filter", format!("이동평균 {}", ma.period())));
        }
        out
    }

    pub fn trail_stop(&self) -> Option<f64> {
        if self.exit_mode == ExitMode::Trailing && self.peak > 0.0 {
            Some(self.peak * (1.0 - self.trail_percent / 100.0))
        } else {
            None
        }
    }

    /// 봉을 다 보고 나서 **마지막에** 부름 — 직전 봉 변동폭과 이동평균을 다음 봉용으로
    /// 갱신함. 판단 **앞**에 두면 진행 중인 봉으로 자기 자신을 판정하게 됨.
    fn roll(&mut self, bar: &Bar) {
        self.prev_range = Some(bar.h - bar.l);
        if let Some(ma) = self.ma.as_mut() {
            self.ma_value = ma.update(bar.c);
        }
    }

    /// 예약해 둔 청산을 `price`에 실행함. 봉 머리와 틱 양쪽에서 부름.
    fn drain_pending(&mut self, ctx: &mut SimContext<'_>, price: f64) {
        if self.exit_next_open && ctx.in_position() {
            ctx.close_all(price, price, false, "");
        }
        self.exit_next_open = false;
    }
}

impl Strategy for Breakout {
    fn on_bar(&mut self, ctx: &mut SimContext<'_>, bar: &Bar) {
        // 어제 돌파로 들어간 자리의 '다음 봉 시가 청산'을 이 봉의 시가에 실행함.
        self.drain_pending(ctx, bar.o);

        if ctx.common_risk(bar) {
            // 끌려 나갔어도 직전 봉 변동폭과 평균은 **그대로 갱신함** — 안 하면 다음 봉의
            // 돌파선이 한 칸 묵은 값으로 계산됨.
            self.exit_next_open = false;
            self.roll(bar);
            return;
        }

        if ctx.in_position() {
            self.peak = self.peak.max(bar.h);
            match self.exit_mode {
                ExitMode::Trailing => {
                    let trigger = self.peak * (1.0 - self.trail_percent / 100.0);
                    if bar.l <= trigger {
                        ctx.close_all(trigger, trigger, false, "");
                    }
                }
                ExitMode::TakeProfit => {
                    if let Some(tp) = self.take_profit {
                        let target = ctx.avg_entry() * (1.0 + tp / 100.0);
                        if bar.h >= target {
                            ctx.close_all(target, target, false, "");
                        }
                    }
                }
                // next_open은 다음 봉 머리에서 처리함.
                ExitMode::NextOpen => {}
            }
        } else if !ctx.entry_blocked()
            && let Some(target) = self.entry_target(bar.o)
        {
            // 돌파선을 **고가**가 넘었는지 + 이동평균 필터가 허락하는지. 둘 다여야 함.
            if bar.h >= target && self.ma_allows(bar.c) {
                let margin = self.invest_ratio * ctx.cash();
                if ctx.open_long(margin, target, bar.c, "진입") {
                    self.peak = bar.c;
                    if self.exit_mode == ExitMode::NextOpen {
                        self.exit_next_open = true;
                    }
                }
            }
        }
        self.roll(bar);
    }

    fn execute_pending(&mut self, ctx: &mut SimContext<'_>, price: f64) {
        // 마감봉만 오는 실시간에서 다음 봉까지 기다리면 한 간격 늦게 체결됨.
        self.drain_pending(ctx, price);
    }

    fn levels(&self, avg_entry: f64, last_bar: Option<&Bar>) -> Levels {
        let mut out = Levels { reference: self.ma_value.and_then(price), ..Default::default() };
        if avg_entry > 0.0 {
            out.trail_stop = self.trail_stop().and_then(price);
            out.armed = self.exit_mode == ExitMode::Trailing;
            if let (ExitMode::TakeProfit, Some(tp)) = (self.exit_mode, self.take_profit) {
                out.exit_target = price(avg_entry * (1.0 + tp / 100.0));
            }
            return out;
        }
        // 비어 있음 — 돌파선은 **이번 봉의 시가**에서 재므로 마지막 봉이 있어야 함.
        match last_bar.and_then(|bar| self.entry_target(bar.o)).and_then(price) {
            Some(target) => {
                out.entry_target = Some(target);
                out
            }
            None if self.prev_range.is_none() => Levels::pending(),
            None => out,
        }
    }

    fn on_warmup_reset(&mut self) {
        // 지표(직전 변동폭·이동평균)는 시장 사실이라 그대로 둠 — 포지션에 매인 것만 지움.
        self.peak = 0.0;
        self.exit_next_open = false;
    }
}
