//! **E — 트레일링 스탑.** 오를 때는 따라 올라가고, 고점에서 정해진 만큼 밀리면 판다.
//!
//! 세 숫자로 움직인다:
//!
//! * **진입** — 바로 사거나(`immediate`), 기준가에서 `entry_dip`% 눌렸을 때 산다(`dip`).
//! * **발동**(`activation_profit`) — 평단 대비 이만큼 올라야 트레일이 **켜진다**. 그 전에는
//!   트레일이 아무것도 지키지 않는다(손절선만 작동한다).
//! * **따라가기**(`trail_percent`) — 켜진 뒤로는 **고점** 대비 이만큼 밀리면 판다.
//!
//! 고점은 봉의 **고가**로, 밀림은 봉의 **저가**로 본다. 봉 안에서 스쳤다면 실제로는
//! 체결됐기 때문이다 — 종가만 보면 "그 사이 선을 지났는데 안 팔린" 것이 된다.
//!
//! 고점 · 발동 여부 · 눌림목 기준가 셋은 **봉에서 다시 구할 수 없다.** 재기동할 때
//! 체크포인트가 실어 보내지 않으면 트레일이 고점을 잊고 발동 전으로 돌아간다
//! (2026-10-12 KRW-CKB 제보가 그것이었다).

use crate::engine::candle_sim::{Bar, SimContext, Strategy};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EntryMode {
    /// 평평한 상태가 되면 바로 산다.
    Immediate,
    /// 기준가에서 `entry_dip`% 눌리기를 기다린다.
    Dip,
}

#[derive(Debug, Clone)]
pub struct Trailing {
    pub entry_mode: EntryMode,
    /// 눌림목 진입 폭(%).
    pub entry_dip: f64,
    /// 트레일이 켜지는 수익률(%).
    pub activation_profit: f64,
    /// 고점 대비 밀림 폭(%) — 여기서 판다.
    pub trail_percent: f64,
    /// 한 번 팔고 다시 살 것인가. 거짓이면 첫 청산 뒤 규칙을 멈춘다.
    pub reenter_after_exit: bool,
    /// 진입 한 번에 쓰는 자본 비율.
    pub invest_ratio: f64,

    /// 포지션을 들고 있는 동안의 고점.
    pub peak: f64,
    /// 트레일이 켜졌는가.
    pub armed: bool,
    /// 눌림목 진입의 기준가. 평평해지는 순간 잡는다.
    pub dip_ref: Option<f64>,
}

impl Trailing {
    pub fn new(
        entry_mode: EntryMode,
        entry_dip: f64,
        activation_profit: f64,
        trail_percent: f64,
        reenter_after_exit: bool,
        invest_ratio: f64,
    ) -> Self {
        Self {
            entry_mode,
            entry_dip,
            activation_profit,
            trail_percent,
            reenter_after_exit,
            invest_ratio,
            peak: 0.0,
            armed: false,
            dip_ref: None,
        }
    }

    /// 지금 트레일이 발동해 있다면 그 손절선. 화면에 그리는 선이 이 값이다.
    pub fn trail_stop(&self) -> Option<f64> {
        if self.armed && self.peak > 0.0 {
            Some(self.peak * (1.0 - self.trail_percent / 100.0))
        } else {
            None
        }
    }

    /// 트레일이 켜지는 가격(평단 기준). 아직 안 켜졌을 때 화면에 그린다.
    pub fn activation_price(&self, avg_entry: f64) -> Option<f64> {
        if self.armed || avg_entry <= 0.0 {
            None
        } else {
            Some(avg_entry * (1.0 + self.activation_profit / 100.0))
        }
    }

    /// 눌림목 진입을 노리는 가격. 보유 중이면 없다.
    pub fn entry_target(&self) -> Option<f64> {
        match (self.entry_mode, self.dip_ref) {
            (EntryMode::Dip, Some(reference)) => Some(reference * (1.0 - self.entry_dip / 100.0)),
            _ => None,
        }
    }
}

impl Strategy for Trailing {
    fn on_bar(&mut self, ctx: &mut SimContext<'_>, bar: &Bar) {
        // 손절 · 보유시간 · 일일 한도가 먼저다. 끌려 나갔으면 이 봉에서 더 판단하지 않는다 —
        // 다만 "한 번 팔면 다시 사지 않는다" 는 설정은 그때도 지킨다.
        if ctx.common_risk(bar) {
            if !self.reenter_after_exit {
                ctx.stop_rule();
            }
            return;
        }
        if ctx.in_position() {
            let avg = ctx.avg_entry();
            self.peak = self.peak.max(bar.h);
            if !self.armed && self.peak >= avg * (1.0 + self.activation_profit / 100.0) {
                self.armed = true;
            }
            if self.armed {
                let trigger = self.peak * (1.0 - self.trail_percent / 100.0);
                if bar.l <= trigger {
                    ctx.close_all(trigger, trigger, false, "");
                    if !self.reenter_after_exit {
                        ctx.stop_rule();
                    }
                    // 다음 눌림목의 기준은 **판 봉의 종가**다(서버와 같게).
                    self.dip_ref = Some(bar.c);
                }
            }
            return;
        }

        // 눌림목 기준가는 관문 **앞**에서 잡는다. 뒤에 두면 관문이 막는 동안 기준가가
        // 비어 있다가 "관문이 처음 허락한 봉" 의 시가로 잡히고, 오르는 구간에서는 문턱이
        // 함께 올라가 없던 매수를 하게 된다.
        if self.entry_mode == EntryMode::Dip && self.dip_ref.is_none() {
            self.dip_ref = Some(bar.o);
        }
        if ctx.entry_blocked() {
            return;
        }
        let margin = self.invest_ratio * ctx.ledger.cash;
        let opened = match self.entry_mode {
            EntryMode::Dip => {
                let reference = self.dip_ref.unwrap_or(bar.o);
                let target = reference * (1.0 - self.entry_dip / 100.0);
                if bar.l <= target {
                    ctx.open_long(margin, target, bar.c, "진입")
                } else {
                    false
                }
            }
            EntryMode::Immediate => ctx.open_long(margin, bar.c, bar.c, "진입"),
        };
        if opened {
            // 고점은 **산 봉의 종가**에서 시작한다 — 산 뒤의 고점만 세야 한다.
            self.peak = bar.c;
            self.armed = false;
        }
    }

    fn on_warmup_reset(&mut self) {
        self.peak = 0.0;
        self.armed = false;
        self.dip_ref = None;
    }
}
