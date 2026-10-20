//! **K — SAR 방어 전환.** 롱으로 가다가 밀리면 일부를 덜어내고, 숏으로 **뒤집는다**.
//!
//! 열한 규칙 중 **유일하게 세션 도중에 방향이 바뀐다.** 그래서 숏이 되는 곳에서만 쓴다
//! (국내 거래소는 현물 롱 1배뿐이다).
//!
//! 흐름:
//!
//! ```text
//! 비어 있음 ──진입──▶ 롱 ──+long_take_profit%──▶ 익절 ──▶ 비어 있음 (다시 롱)
//!                     │
//!        평단에서 -drop_trigger% (방어 발동)
//!                     ▼
//!          보유의 partial_exit% 를 먼저 덜어낸다
//!                     │
//!        flip_to_short? 남은 것도 정리하고 **숏으로 전환**
//!                     ▼
//!                    숏 ──(더 떨어짐 -short_take_profit%)──▶ 익절
//!                     │  ╲──(튀어오름 +short_stop_loss%)──▶ 손절
//!                     ▼
//!                 비어 있음 ──reenter_long_after?──▶ 롱
//! ```
//!
//! **공통 손절선은 끈다.** 방어 발동(`drop_trigger_pct`)이 롱 구간의 손절 역할을 하고,
//! 숏 구간은 자기 손절선을 따로 갖는다. 둘을 같이 걸면 방어가 뜨기 전에 손절이 먼저 나가
//! 전환이 영영 일어나지 않는다.

use crate::engine::candle_sim::{Bar, SimContext, Strategy};
use crate::engine::ledger::Side;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Phase {
    Long,
    Short,
}

#[derive(Debug, Clone)]
pub struct Sar {
    /// 롱 익절(%). 없으면 익절하지 않고 방어만 본다.
    pub long_take_profit: Option<f64>,
    /// 평단에서 이만큼 빠지면 방어 발동(%).
    pub drop_trigger: f64,
    /// 방어 때 먼저 덜어내는 비율(0~1).
    pub partial_exit: f64,
    /// 덜어낸 뒤 숏으로 뒤집을 것인가.
    pub flip_to_short: bool,
    /// 숏 익절(%) — 더 떨어지면.
    pub short_take_profit: f64,
    /// 숏 손절(%) — 튀어오르면.
    pub short_stop_loss: f64,
    /// 숏이 끝난 뒤 다시 롱으로 들어갈 것인가.
    pub reenter_long_after: bool,
    pub invest_ratio: f64,

    phase: Phase,
    /// 이번 롱 구간에서 방어를 이미 썼는가.
    defended: bool,
}

impl Sar {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        long_take_profit: Option<f64>,
        drop_trigger: f64,
        partial_exit_pct: f64,
        flip_to_short: bool,
        short_take_profit: f64,
        short_stop_loss: f64,
        reenter_long_after: bool,
        invest_ratio: f64,
    ) -> Self {
        Self {
            long_take_profit,
            drop_trigger,
            partial_exit: partial_exit_pct / 100.0,
            flip_to_short,
            short_take_profit,
            short_stop_loss,
            reenter_long_after,
            invest_ratio,
            phase: Phase::Long,
            defended: false,
        }
    }

    pub fn phase(&self) -> Phase {
        self.phase
    }

    pub fn defended(&self) -> bool {
        self.defended
    }

    /// 방어가 발동하는 가격(롱 구간). 화면에 그리는 선이다.
    pub fn defense_price(&self, avg_entry: f64) -> Option<f64> {
        if self.phase == Phase::Long && !self.defended && avg_entry > 0.0 {
            Some(avg_entry * (1.0 - self.drop_trigger / 100.0))
        } else {
            None
        }
    }

    /// 재기동 복구용.
    pub fn restore(&mut self, phase: Phase, defended: bool) {
        self.phase = phase;
        self.defended = defended;
    }

    /// 비어 있는 상태로 되돌린다 — 늘 롱부터 다시 시작한다.
    fn reset_after_flat(&mut self, ctx: &mut SimContext<'_>) {
        ctx.flip_side(Side::Long);
        self.phase = Phase::Long;
        self.defended = false;
    }

    /// 숏 구간이 끝났다.
    fn end_short(&mut self, ctx: &mut SimContext<'_>) {
        self.reset_after_flat(ctx);
        if !self.reenter_long_after {
            // 영영 비워 둔다.
            ctx.stop_rule();
        }
    }

    fn manage_long(&mut self, ctx: &mut SimContext<'_>, bar: &Bar) {
        let avg = ctx.avg_entry();

        // 익절이 먼저 — 봉 고가로 본다.
        if let Some(tp) = self.long_take_profit {
            let target = avg * (1.0 + tp / 100.0);
            if bar.h >= target {
                ctx.close_all(target, target, false, "");
                self.reset_after_flat(ctx);
                return;
            }
        }

        // 방어 발동 — 봉 저가로 본다. 한 롱 구간에 한 번만.
        let trigger = avg * (1.0 - self.drop_trigger / 100.0);
        if !self.defended && bar.l <= trigger {
            ctx.reduce_long(self.partial_exit, trigger, trigger, "부분 청산");
            self.defended = true;
            if !self.flip_to_short {
                return;
            }
            // 남은 것도 정리하고 숏으로 뒤집는다.
            if ctx.in_position() {
                ctx.close_all(trigger, trigger, false, "");
            }
            ctx.flip_side(Side::Short);
            if ctx.open_short(trigger, trigger, "진입") {
                self.phase = Phase::Short;
            } else {
                // 숏을 열 돈이 없다 — 비워 둔 채 롱으로 되돌린다.
                ctx.flip_side(Side::Long);
                self.phase = Phase::Long;
            }
        }
    }

    fn manage_short(&mut self, ctx: &mut SimContext<'_>, bar: &Bar) {
        let avg = ctx.avg_entry();
        let take_profit = avg * (1.0 - self.short_take_profit / 100.0); // 더 떨어지면 번다
        let stop_loss = avg * (1.0 + self.short_stop_loss / 100.0); // 튀어오르면 잃는다

        // 한 봉이 둘 다 스쳤으면 **손절이 이긴다**(보수적으로).
        if bar.h >= stop_loss {
            ctx.close_all(stop_loss, stop_loss, true, "");
            self.end_short(ctx);
            return;
        }
        if bar.l <= take_profit {
            ctx.close_all(take_profit, take_profit, false, "");
            self.end_short(ctx);
        }
    }
}

impl Strategy for Sar {
    fn on_bar(&mut self, ctx: &mut SimContext<'_>, bar: &Bar) {
        // 공통 손절선은 빌드 때 꺼 두었다 — 여기 남는 것은 보유시간 · 일일 한도이고,
        // 둘 다 지금 방향(롱/숏)을 따라 움직인다.
        if ctx.common_risk(bar) {
            self.reset_after_flat(ctx);
            return;
        }

        if ctx.in_position() {
            if ctx.ledger.side == Side::Short {
                self.manage_short(ctx, bar);
            } else {
                self.manage_long(ctx, bar);
            }
            return;
        }

        // 비어 있다 — 다시 롱으로.
        if ctx.entry_blocked() {
            return;
        }
        // 비어 있으면 늘 롱이다. 숏 진입을 되돌린 상태(실행기 세션)는 방향이 숏으로 남아
        // 있을 수 있어, 롱 장부 식을 쓰도록 먼저 맞춘다.
        ctx.flip_side(Side::Long);
        let margin = self.invest_ratio * ctx.cash();
        if ctx.open_long(margin, bar.c, bar.c, "진입") {
            self.phase = Phase::Long;
            self.defended = false;
        }
    }

    fn on_warmup_reset(&mut self) {
        self.phase = Phase::Long;
        self.defended = false;
    }
}
