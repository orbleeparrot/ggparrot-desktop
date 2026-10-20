//! **지표형(F · G · J)의 공통 토대.** 봉이 닫힐 때 지표를 보고 **다음 봉에** 실행한다.
//!
//! 왜 다음 봉인가: 지표는 **종가**로 계산된다. 그 종가를 알았을 때는 이미 그 봉이 끝난
//! 뒤다. 그 봉 안에서 체결했다고 치면 "종가를 미리 알고 그 봉에서 샀다" 는 뜻이 되고,
//! 백테스트가 실거래보다 좋게 나온다. 그래서 **판정은 마감에, 체결은 다음 시가에** 한다.
//!
//! 세 규칙이 다른 것은 [`Signal::evaluate`] 하나뿐이다 — "이 종가를 보니 다음 봉에서
//! 무엇을 할까". 실행 · 익절 · 공통 위험은 전부 여기서 한 번만 돈다.

use crate::engine::candle_sim::{Bar, SimContext, Strategy};
use crate::engine::ledger::Side;

/// 다음 봉에 할 일.
#[derive(Debug, Clone, PartialEq)]
pub enum Intent {
    Enter,
    Exit,
}

/// 지표가 내는 판정. 사유 한 줄이 활동 기록과 주문 명령에 그대로 간다.
#[derive(Debug, Clone, Default)]
pub struct Pending {
    pub intent: Option<Intent>,
    /// 사람이 읽는 한 줄 — "RSI 23.1 ≤ 30", "골든크로스".
    pub note: String,
}

/// 지표 하나가 구현하는 것.
pub trait Signal {
    /// 이 **종가**를 보고 다음 봉의 의도를 정한다.
    ///
    /// `in_position` 은 지금 들고 있는가 — 같은 지표라도 보유 중이면 청산 쪽을 본다.
    /// `side` 는 롱 · 숏(숏이면 조건이 거울처럼 뒤집힌다).
    fn evaluate(&mut self, close: f64, in_position: bool, side: Side, pending: &mut Pending);

    /// 포지션에 매인 상태가 있으면 되돌린다(연속 횟수 등은 시장 사실이라 그대로 둔다).
    fn on_warmup_reset(&mut self) {}
}

/// 지표형 규칙 하나 — 판정은 [`Signal`] 에 맡기고 실행을 맡는다.
#[derive(Debug, Clone)]
pub struct IndicatorRule<S: Signal> {
    pub signal: S,
    /// 평단 대비 익절(%). 있으면 봉 **안에서** 걸린다(지표와 무관하게).
    pub take_profit: Option<f64>,
    pub invest_ratio: f64,
    pending: Pending,
}

impl<S: Signal> IndicatorRule<S> {
    pub fn new(signal: S, take_profit: Option<f64>, invest_ratio: f64) -> Self {
        Self { signal, take_profit, invest_ratio, pending: Pending::default() }
    }

    /// 지금 정해 둔 의도(화면 표시 · 상태 저장용).
    pub fn pending(&self) -> &Pending {
        &self.pending
    }

    /// 정해 둔 의도를 `price` 에 실행하고 의도를 지운다.
    ///
    /// `mark` 는 체결의 평가 기준가다. 백테스트는 다음 봉의 **시가**로 부르고 기준가는
    /// 그 봉의 종가를 쓴다 — 실시간도 같은 모양을 맞춘다.
    fn drain(&mut self, ctx: &mut SimContext<'_>, price: f64, mark: f64) {
        match self.pending.intent.take() {
            Some(Intent::Exit) if ctx.in_position() => {
                let reason = if self.pending.note.is_empty() {
                    String::new()
                } else {
                    format!("{} · 청산", self.pending.note)
                };
                ctx.close_all(price, price, false, &reason);
            }
            Some(Intent::Enter) if !ctx.in_position() && !ctx.entry_blocked() => {
                let reason = if self.pending.note.is_empty() {
                    "진입".to_string()
                } else {
                    format!("{} · 진입", self.pending.note)
                };
                if ctx.ledger.side == Side::Short {
                    ctx.open_short(price, mark, &reason);
                } else {
                    let margin = self.invest_ratio * ctx.cash();
                    ctx.open_long(margin, price, mark, &reason);
                }
            }
            _ => {}
        }
    }
}

impl<S: Signal> Strategy for IndicatorRule<S> {
    fn on_bar(&mut self, ctx: &mut SimContext<'_>, bar: &Bar) {
        // 1) 직전 마감봉에서 정한 의도를 이 봉의 **시가**에 실행한다.
        self.drain(ctx, bar.o, bar.c);

        // 2) 손절 · 보유시간 · 일일 한도.
        if ctx.common_risk(bar) {
            return;
        }

        // 3) 익절은 봉 **안에서** 걸린다 — 지표를 다시 계산하기 전에 본다.
        if let Some(tp) = self.take_profit.filter(|_| ctx.in_position()) {
            let avg = ctx.avg_entry();
            if ctx.ledger.side == Side::Short {
                let target = avg * (1.0 - tp / 100.0);
                if bar.l <= target {
                    ctx.close_all(target, target, false, "");
                }
            } else {
                let target = avg * (1.0 + tp / 100.0);
                if bar.h >= target {
                    ctx.close_all(target, target, false, "");
                }
            }
        }

        // 4) 이 **종가**로 지표를 갱신하고 다음 봉의 의도를 정한다.
        let side = ctx.ledger.side;
        let in_position = ctx.in_position();
        self.signal.evaluate(bar.c, in_position, side, &mut self.pending);
    }

    fn execute_pending(&mut self, ctx: &mut SimContext<'_>, price: f64) {
        // 마감봉만 오는 실시간에서 다음 봉까지 기다리면 한 간격 늦게 체결된다.
        self.drain(ctx, price, price);
    }

    fn on_warmup_reset(&mut self) {
        // 의도는 **남긴다** — 백테스트가 다음 봉 시가에 실행하는 의도와 같은 것이다
        // (서버 `CandleSim.warmup` 의 주석 그대로). 지표의 연속 횟수도 시장 사실이다.
        self.signal.on_warmup_reset();
    }
}
