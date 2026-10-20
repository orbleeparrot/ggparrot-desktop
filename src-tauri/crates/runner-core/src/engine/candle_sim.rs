//! 봉 기계 — **봉이 닫힐 때만** 판단하는 규칙들의 공통 토대.
//!
//! D · E · F · G · H · I · J · K 여덟 가지가 이 위에 올라탄다. 규칙은
//! [`Strategy::on_bar`] 하나만 구현하고, 그 앞뒤로 모두가 똑같이 거치는 것 —
//! 손절 · 최대 보유시간 · 일일 손실 한도 · 쿨다운 — 은 여기서 한 번만 돈다.
//!
//! **순서가 규칙이다.** 서버(파이썬 `CandleSim._common_risk`)와 같은 순서로 보지 않으면
//! 한 봉이 손절선과 익절선을 동시에 스쳤을 때 어느 쪽이 먼저 걸리는지가 갈린다:
//!
//! 1. 강제청산(레버리지 > 1) — 가장 보수적으로, 다른 무엇보다 먼저
//! 2. 손절(평단 기준)
//! 3. 최대 보유시간
//! 4. 일일 손실 한도 → 그날은 더 사지 않는다
//! 5. 그제야 규칙 자신의 판단
//!
//! 진행 중인 봉으로는 절대 판단하지 않는다 — 그건 미래를 훔쳐보는 것이고, 백테스트가
//! 보여 준 결과와 실거래가 갈리는 가장 흔한 원인이다.

use super::ledger::{Fill, Ledger, Side};

/// 봉 하나. [`crate::candles::Candle`] 를 엔진이 쓰는 모양으로 좁힌 것.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Bar {
    /// 봉이 **열린** 시각(ms, UTC).
    pub t: i64,
    pub o: f64,
    pub h: f64,
    pub l: f64,
    pub c: f64,
    pub v: Option<f64>,
}

impl From<crate::candles::Candle> for Bar {
    fn from(c: crate::candles::Candle) -> Self {
        Bar { t: c.t, o: c.o, h: c.h, l: c.l, c: c.c, v: c.v }
    }
}

/// 공통 위험 설정 — 매크로의 `risk` 에서 온다. 0 · None 은 "끔".
#[derive(Debug, Clone, Copy)]
pub struct Risk {
    /// 진입 한 번에 쓰는 자본 비율(0 < r ≤ 1).
    pub invest_ratio: f64,
    /// 평단 대비 손절 폭(%). None 이면 손절 없음.
    pub stop_loss_pct: Option<f64>,
    /// 그날 자산이 이만큼 빠지면 청산하고 **그날은 더 사지 않는다**(%).
    pub daily_max_loss_pct: Option<f64>,
    /// 이 시간이 지나면 강제 청산.
    pub max_holding_hours: Option<f64>,
    /// 손절 뒤 이 시간 동안 다시 사지 않는다(분).
    pub cooldown_minutes: f64,
}

impl Default for Risk {
    fn default() -> Self {
        Self {
            invest_ratio: 1.0,
            stop_loss_pct: None,
            daily_max_loss_pct: None,
            max_holding_hours: None,
            cooldown_minutes: 0.0,
        }
    }
}

const MS_HOUR: i64 = 3_600_000;
const MS_MINUTE: i64 = 60_000;
const MS_DAY: i64 = 24 * MS_HOUR;

/// 그날이 바뀌었는지 가를 때 쓰는 UTC 날짜 번호(서버의 `ts.strftime("%Y-%m-%d")` 와 같은 구분).
fn day_index(t_ms: i64) -> i64 {
    t_ms.div_euclid(MS_DAY)
}

/// 규칙 하나가 구현하는 것 — "이 봉에서 무엇을 할까" 뿐이다.
///
/// 장부를 직접 건드리지 않고 [`SimContext`] 를 거친다. 그래야 모든 규칙이 같은 손절 ·
/// 쿨다운 · 한도를 거치고, 한 규칙만 빠뜨리는 일이 생기지 않는다.
pub trait Strategy {
    /// 봉 하나를 보고 판단한다. 체결은 `ctx` 를 통해 낸다.
    fn on_bar(&mut self, ctx: &mut SimContext<'_>, bar: &Bar);

    /// **웜업을 되돌릴 때만** 불린다 — 포지션이 있어야 뜻이 있는 규칙 상태를 지운다
    /// (고점 · 눌림목 기준가 · 예약해 둔 청산).
    ///
    /// 포지션이 닫힐 때마다 부르면 안 된다. 서버에는 그런 훅이 없고, 규칙이 청산 자리에서
    /// 스스로 남기는 값이 있다(E 는 판 봉의 종가를 다음 눌림목 기준으로 삼는다). 여기서
    /// 지우면 그 값이 사라져 다음 진입 자리가 통째로 달라진다.
    fn on_warmup_reset(&mut self) {}

    /// 공통 위험(손절 · 보유시간 · 일일 한도)이 **강제로** 포지션을 닫았을 때.
    ///
    /// 그 봉에서 규칙 자신의 판단은 돌지 않으므로, 규칙이 그 사실을 알아야 하는 경우
    /// (예: "한 번 팔면 다시 사지 않는다") 여기서 처리한다.
    fn on_forced_exit(&mut self, _ctx: &mut SimContext<'_>) {}

    /// 틱에 실행하기로 **예약해 둔** 청산이 있으면 그 가격에 실행한다.
    ///
    /// 백테스트의 "마감에 판정하고 다음 시가에 체결" 을 실시간에서 맞추는 자리다.
    /// 마감봉만 오는 실시간에서 다음 `on_bar` 까지 기다리면 한 간격 늦게 체결된다.
    fn execute_pending(&mut self, _ctx: &mut SimContext<'_>, _price: f64) {}
}

/// 규칙이 장부와 공통 위험에 닿는 통로. 한 봉 동안만 산다.
pub struct SimContext<'a> {
    pub ledger: &'a mut Ledger,
    pub risk: &'a Risk,
    /// 이 봉에서 난 체결들(순서대로).
    pub fills: &'a mut Vec<Fill>,
    /// 지금 판단 중인 시각(ms) — 쿨다운 · 보유시간을 재는 기준.
    pub now_ms: i64,
    halted_today: &'a mut bool,
    cooldown_until_ms: &'a mut Option<i64>,
    entry_time_ms: &'a mut Option<i64>,
    stopped: &'a mut bool,
    went_flat: &'a mut bool,
}

impl SimContext<'_> {
    /// 지금 새로 사도 되는가. 멈춤 · 그날 중단 · 쿨다운을 본다.
    pub fn entry_blocked(&self) -> bool {
        if *self.stopped || *self.halted_today {
            return true;
        }
        match *self.cooldown_until_ms {
            Some(until) => self.now_ms < until,
            None => false,
        }
    }

    pub fn in_position(&self) -> bool {
        self.ledger.in_position()
    }

    pub fn avg_entry(&self) -> f64 {
        self.ledger.avg_entry()
    }

    /// 이 규칙을 **완전히 멈춘다**(밴드 이탈 등). 다시 시작하기 전까지 사지 않는다.
    pub fn stop_rule(&mut self) {
        *self.stopped = true;
    }

    /// 롱 진입. `margin` 은 넣을 자기 돈.
    pub fn open_long(&mut self, margin: f64, close: f64, mark: f64, reason: &str) -> bool {
        match self.ledger.open_long(margin, close, mark, reason) {
            Some(fill) => {
                if self.entry_time_ms.is_none() {
                    *self.entry_time_ms = Some(self.now_ms);
                }
                self.fills.push(fill);
                true
            }
            None => false,
        }
    }

    /// 숏 진입. 서버와 같게 `invest_ratio * cash` 를 넣는다.
    pub fn open_short(&mut self, close: f64, mark: f64, reason: &str) -> bool {
        let margin = self.risk.invest_ratio * self.ledger.cash;
        match self.ledger.open_short(margin, close, mark, reason) {
            Some(fill) => {
                *self.entry_time_ms = Some(self.now_ms);
                self.fills.push(fill);
                true
            }
            None => false,
        }
    }

    /// 전량 청산. `is_stop` 이면 쿨다운이 걸린다.
    pub fn close_all(&mut self, close: f64, mark: f64, is_stop: bool, reason: &str) -> bool {
        let reason = if reason.is_empty() {
            if is_stop {
                "손절"
            } else {
                "청산"
            }
        } else {
            reason
        };
        match self.ledger.close_all(close, mark, reason) {
            Some(fill) => {
                *self.entry_time_ms = None;
                if is_stop && self.risk.cooldown_minutes > 0.0 {
                    *self.cooldown_until_ms =
                        Some(self.now_ms + (self.risk.cooldown_minutes * MS_MINUTE as f64) as i64);
                }
                *self.went_flat = true;
                self.fills.push(fill);
                true
            }
            None => false,
        }
    }

    /// 보유의 `fraction` 만 판다(롱 전용).
    pub fn reduce_long(&mut self, fraction: f64, close: f64, mark: f64, reason: &str) -> bool {
        match self.ledger.reduce_long(fraction, close, mark, reason) {
            Some(fill) => {
                if !self.ledger.in_position() {
                    *self.entry_time_ms = None;
                    *self.went_flat = true;
                }
                self.fills.push(fill);
                true
            }
            None => false,
        }
    }

    /// 포지션 방향을 바꾼다(K 의 숏 전환). 보유가 없을 때만 뜻이 있다.
    pub fn flip_side(&mut self, side: Side) {
        self.ledger.side = side;
    }
}

/// 봉 기계 하나 — 장부 + 공통 위험 + 규칙.
pub struct CandleSim<S: Strategy> {
    pub ledger: Ledger,
    pub risk: Risk,
    strategy: Option<S>,
    day: Option<i64>,
    day_start_equity: f64,
    halted_today: bool,
    cooldown_until_ms: Option<i64>,
    entry_time_ms: Option<i64>,
    stopped: bool,
    /// 봉이 정한 체결 의도를 **다음 틱**이 가져간다(백테스트의 '다음 시가' 와 같은 자리).
    queue: std::collections::VecDeque<Fill>,
}

impl<S: Strategy> CandleSim<S> {
    pub fn new(ledger: Ledger, risk: Risk, strategy: S) -> Self {
        let day_start_equity = ledger.initial_capital;
        Self {
            ledger,
            risk,
            strategy: Some(strategy),
            day: None,
            day_start_equity,
            halted_today: false,
            cooldown_until_ms: None,
            entry_time_ms: None,
            stopped: false,
            queue: Default::default(),
        }
    }

    fn context<'a>(&'a mut self, now_ms: i64, fills: &'a mut Vec<Fill>, went_flat: &'a mut bool) -> SimContext<'a> {
        SimContext {
            ledger: &mut self.ledger,
            risk: &self.risk,
            fills,
            now_ms,
            halted_today: &mut self.halted_today,
            cooldown_until_ms: &mut self.cooldown_until_ms,
            entry_time_ms: &mut self.entry_time_ms,
            stopped: &mut self.stopped,
            went_flat,
        }
    }

    /// 공통 위험을 먼저 본다. 강제 청산이 일어났으면 true — 그 봉에서 규칙은 판단하지 않는다.
    fn common_risk(&mut self, bar: &Bar, fills: &mut Vec<Fill>) -> bool {
        let day = day_index(bar.t);
        if self.day != Some(day) {
            self.day = Some(day);
            self.day_start_equity = self.ledger.equity(bar.o);
            self.halted_today = false;
        }
        let mut forced = false;
        let mut went_flat = false;

        // 1) 손절 — 봉의 저가(숏은 고가)로 본다. 봉 안에서 스쳤다면 실제로는 체결됐다.
        if let Some(sl) = self.risk.stop_loss_pct.filter(|v| *v > 0.0) {
            if self.ledger.in_position() && self.ledger.stop_hit(bar.l, bar.h, sl) {
                let avg = self.ledger.avg_entry();
                let px = if self.ledger.side == Side::Short {
                    avg * (1.0 + sl / 100.0)
                } else {
                    avg * (1.0 - sl / 100.0)
                };
                let mut ctx = self.context(bar.t, fills, &mut went_flat);
                forced = ctx.close_all(px, px, true, "손절");
            }
        }

        // 2) 최대 보유시간 — 종가에 강제 청산.
        if !forced {
            if let (Some(hours), Some(entered)) = (self.risk.max_holding_hours, self.entry_time_ms) {
                let held_ms = bar.t - entered;
                if self.ledger.in_position() && held_ms >= (hours * MS_HOUR as f64) as i64 {
                    let mut ctx = self.context(bar.t, fills, &mut went_flat);
                    forced = ctx.close_all(bar.c, bar.c, false, "최대 보유시간");
                }
            }
        }

        // 3) 일일 손실 한도 — 청산하고 그날은 더 사지 않는다. 비어 있어도 중단은 건다.
        if let Some(limit) = self.risk.daily_max_loss_pct.filter(|v| *v > 0.0) {
            let equity_now = self.ledger.equity(bar.c);
            let drawdown = (equity_now - self.day_start_equity) / self.day_start_equity * 100.0;
            if drawdown <= -limit {
                if self.ledger.in_position() {
                    let mut ctx = self.context(bar.t, fills, &mut went_flat);
                    if ctx.close_all(bar.c, bar.c, false, "일일 손실 한도") {
                        forced = true;
                    }
                }
                self.halted_today = true;
            }
        }

        if forced {
            let mut unused = Vec::new();
            let mut flat = false;
            self.with_strategy(bar.t, &mut unused, &mut flat, |strategy, ctx| {
                strategy.on_forced_exit(ctx)
            });
            debug_assert!(unused.is_empty(), "강제 청산 훅은 주문을 내지 않는다");
        }
        forced
    }

    /// 마감봉 하나를 넣는다. 돌려주는 값은 **대기 중인 체결 의도 수** — 주문은 여기서
    /// 나오지 않는다. 봉은 '무엇을 할지' 만 정하고 체결은 다음 [`CandleSim::tick`] 이다.
    pub fn on_bar(&mut self, bar: &Bar) -> usize {
        let mut fills = Vec::new();
        // 보유 중인 숏에는 봉마다 비용이 붙는다(현물은 0).
        self.ledger.apply_funding(bar.c);
        if !self.common_risk(bar, &mut fills) {
            let mut went_flat = false;
            self.with_strategy(bar.t, &mut fills, &mut went_flat, |strategy, ctx| {
                strategy.on_bar(ctx, bar)
            });
        }
        let count = fills.len();
        self.queue.extend(fills);
        count
    }

    /// 전략을 잠시 **들어내어** `ctx` 가 나머지 필드를 빌릴 수 있게 한다.
    ///
    /// Rust 는 `self.strategy` 와 `&mut self.ledger` 를 동시에 빌려주지 않는다.
    /// `Option::take` 로 소유권을 잠깐 옮겼다가 반드시 되돌려 놓는다 — 포인터를 직접
    /// 읽는 방식은 전략이 패닉하면 같은 값을 두 번 버리게 되어 쓰지 않는다.
    fn with_strategy<R>(
        &mut self,
        now_ms: i64,
        fills: &mut Vec<Fill>,
        went_flat: &mut bool,
        body: impl FnOnce(&mut S, &mut SimContext<'_>) -> R,
    ) -> R {
        let mut strategy = self.strategy.take().expect("전략은 늘 제자리에 있다");
        let result = {
            let mut ctx = SimContext {
                ledger: &mut self.ledger,
                risk: &self.risk,
                fills,
                now_ms,
                halted_today: &mut self.halted_today,
                cooldown_until_ms: &mut self.cooldown_until_ms,
                entry_time_ms: &mut self.entry_time_ms,
                stopped: &mut self.stopped,
                went_flat,
            };
            body(&mut strategy, &mut ctx)
        };
        self.strategy = Some(strategy);
        result
    }

    fn reset_strategy_state(&mut self) {
        if let Some(strategy) = self.strategy.as_mut() {
            strategy.on_warmup_reset();
        }
    }

    /// 규칙 자신을 들여다봐야 할 때(상태 저장 · 화면 표시).
    pub fn strategy(&self) -> &S {
        self.strategy.as_ref().expect("전략은 늘 제자리에 있다")
    }

    pub fn strategy_mut(&mut self) -> &mut S {
        self.strategy.as_mut().expect("전략은 늘 제자리에 있다")
    }

    /// 현재가 한 틱 — 대기 중인 체결 하나를 꺼낸다. 큐가 비었고 규칙이 '다음 시가에
    /// 청산' 을 예약해 두었으면 이 가격에 실행한다.
    pub fn tick(&mut self, price: f64) -> Option<Fill> {
        if self.queue.is_empty() {
            let mut fills = Vec::new();
            let mut went_flat = false;
            self.with_strategy(0, &mut fills, &mut went_flat, |strategy, ctx| {
                strategy.execute_pending(ctx, price)
            });
            self.queue.extend(fills);
        }
        self.queue.pop_front()
    }

    /// 웜업이 끝났다 — **장부만** 시작값으로 되돌린다.
    ///
    /// 과거 봉을 흘려보내는 동안 지표(이동평균 · RSI · 밴드폭)는 채워지고, 그 사이의
    /// 가상 체결은 버린다. 지표는 시장 사실이라 그대로 두고, 돈과 포지션만 되돌린다 —
    /// 서버 `CandleSim.reset_book` 과 같다. 안 되돌리면 웜업 중의 가짜 매수가
    /// 실거래 포지션으로 둔갑한다.
    pub fn warmup_done(&mut self) {
        let initial = self.ledger.initial_capital;
        self.ledger.cash = initial;
        self.ledger.lots.clear();
        self.ledger.closed_trades.clear();
        self.day = None;
        self.day_start_equity = initial;
        self.halted_today = false;
        self.cooldown_until_ms = None;
        self.entry_time_ms = None;
        self.stopped = false;
        self.queue.clear();
        // 포지션이 있어야 뜻이 있는 규칙 상태(고점 · 기준가 · 예약)도 함께 되돌린다.
        self.reset_strategy_state();
    }

    /// 아직 안 꺼낸 체결 수.
    pub fn pending(&self) -> usize {
        self.queue.len()
    }
}
