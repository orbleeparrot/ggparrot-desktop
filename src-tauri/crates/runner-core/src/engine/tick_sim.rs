//! 틱 기계 — **현재가를 볼 때마다** 판단하는 규칙(A · B · C).
//!
//! 봉을 기다리지 않는 이유가 규칙마다 다르다:
//!
//! * **B(지정가 밴드)** — "2,000원에 사고 3,000원에 판다". 봉이 닫히기를 기다리면 그 사이
//!   2,000원을 지나가 버린다.
//! * **A(익절·손절 재진입)** — 비면 바로 다시 산다. 봉을 기다릴 이유가 없다.
//! * **C(적립식)** — '며칠마다' 를 센다. 가격이 아니라 **달력**이 기준이다.
//!
//! 장부는 [`super::ledger`] 를 쓰지 않는다. 서버의 `PositionSim` 이 묶음(lot) 없이
//! 포지션 하나만 들고 있고 **실현손익 식도 미묘하게 다르기 때문**이다 — 틱 기계는
//! 진입 수수료를 실현손익에서 빼고, 봉 기계는 빼지 않는다(들어갈 때 이미 현금에서 나갔다).
//! 그 차이를 한 장부에 욱여넣으면 어느 한쪽이 서버와 달라진다.

use super::ledger::{buy_fill, is_exit, sell_fill, Costs, Fill, Side};

const MS_HOUR: f64 = 3_600_000.0;
const MS_MINUTE: f64 = 60_000.0;
const MS_DAY: i64 = 86_400_000;

fn day_index(t_ms: i64) -> i64 {
    t_ms.div_euclid(MS_DAY)
}

/// A · B 가 쓰는 공통 위험 설정.
pub use super::candle_sim::Risk;

/// 틱 하나에서 할 판단.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TickRule {
    /// **A** — 평단 대비 `take_profit_pct`% 오르면 팔고, 비면 **바로 다시** 산다.
    TakeProfitReentry,
    /// **B** — `buy_price` 이하면 사고 `sell_price` 이상이면 판다(숏은 반대).
    Band,
}

/// A · B 를 돌리는 기계. 포지션 하나만 들고 있는다.
pub struct TickSim {
    pub side: Side,
    pub costs: Costs,
    pub risk: Risk,
    pub rule: TickRule,
    /// A 의 익절 폭(%).
    pub take_profit_pct: f64,
    /// B 의 사는 가격.
    pub buy_price: f64,
    /// B 의 파는 가격.
    pub sell_price: f64,

    pub initial_capital: f64,
    pub cash: f64,
    pub in_position: bool,
    pub qty: f64,
    /// 진입 체결가(슬리피지 반영). 익절 · 손절의 기준이다.
    pub entry_fill: f64,
    entry_commission: f64,
    /// 이 포지션에 묶인 자기 돈.
    margin: f64,
    pub closed_trades: Vec<f64>,

    day: Option<i64>,
    day_start_equity: f64,
    halted_today: bool,
    cooldown_until_ms: Option<i64>,
    entry_time_ms: Option<i64>,
    /// 지금 시각(ms). 봉이 올 때마다 그 봉의 시각으로 맞춘다 — 쿨다운 · 보유시간 ·
    /// 날짜가 이 값을 본다. 실행기는 실제 시계를, 계약 대조는 봉 시각을 넣는다.
    pub clock_ms: i64,
}

impl TickSim {
    pub fn new(side: Side, costs: Costs, risk: Risk, rule: TickRule, initial_capital: f64) -> Self {
        Self {
            side,
            costs,
            risk,
            rule,
            take_profit_pct: 0.0,
            buy_price: 0.0,
            sell_price: 0.0,
            initial_capital,
            cash: initial_capital,
            in_position: false,
            qty: 0.0,
            entry_fill: 0.0,
            entry_commission: 0.0,
            margin: 0.0,
            closed_trades: Vec::new(),
            day: None,
            day_start_equity: initial_capital,
            halted_today: false,
            cooldown_until_ms: None,
            entry_time_ms: None,
            clock_ms: 0,
        }
    }

    pub fn equity(&self, price: f64) -> f64 {
        if !self.in_position {
            return self.cash;
        }
        if self.side == Side::Long {
            self.cash + self.margin + self.qty * (price - self.entry_fill)
        } else {
            self.cash + self.qty * (self.entry_fill - price)
        }
    }

    fn fill(&self, side: &'static str, price: f64, qty: f64, mark: f64, reason: &str) -> Fill {
        let equity_after = self.equity(mark);
        let return_pct = (equity_after - self.initial_capital) / self.initial_capital * 100.0;
        let after = if self.in_position { self.qty } else { 0.0 };
        let qty_before = if is_exit(side) { after + qty } else { (after - qty).max(0.0) };
        Fill {
            side,
            price,
            qty,
            equity_after,
            return_pct,
            reason: reason.to_string(),
            qty_before,
        }
    }

    /// 손절선에 닿았는가 — 진입가 기준, **현재가**로 본다(봉의 극값이 아니다).
    fn stop_hit(&self, price: f64) -> bool {
        let Some(sl) = self.risk.stop_loss_pct.filter(|v| *v > 0.0) else { return false };
        if self.side == Side::Long {
            price <= self.entry_fill * (1.0 - sl / 100.0)
        } else {
            price >= self.entry_fill * (1.0 + sl / 100.0)
        }
    }

    /// 이익 실현 조건.
    fn profit_hit(&self, price: f64) -> bool {
        match (self.rule, self.side) {
            (TickRule::TakeProfitReentry, Side::Long) => {
                price >= self.entry_fill * (1.0 + self.take_profit_pct / 100.0)
            }
            (TickRule::TakeProfitReentry, Side::Short) => {
                price <= self.entry_fill * (1.0 - self.take_profit_pct / 100.0)
            }
            (TickRule::Band, Side::Long) => price >= self.sell_price,
            (TickRule::Band, Side::Short) => price <= self.buy_price,
        }
    }

    /// 새로 살 조건. A 는 **늘 산다**(비면 바로 재진입).
    fn should_enter(&self, price: f64) -> bool {
        match (self.rule, self.side) {
            (TickRule::TakeProfitReentry, _) => true,
            (TickRule::Band, Side::Long) => price <= self.buy_price,
            (TickRule::Band, Side::Short) => price >= self.sell_price,
        }
    }

    fn entry_blocked(&self, now_ms: i64) -> bool {
        if self.halted_today {
            return true;
        }
        matches!(self.cooldown_until_ms, Some(until) if now_ms < until)
    }

    fn close(&mut self, exec_price: f64, mark: f64, now_ms: i64, is_stop: bool) -> Fill {
        let traded = self.qty;
        let comm = self.costs.commission_pct;
        let (side, f, pnl) = if self.side == Side::Long {
            let f = sell_fill(exec_price, self.costs.slippage_pct);
            let exit_comm = self.qty * f * comm / 100.0;
            self.cash += self.margin + self.qty * (f - self.entry_fill) - exit_comm;
            // 봉 기계와 달리 **진입 수수료까지 뺀다** — 서버 `PositionSim._do_close` 그대로.
            (
                "sell",
                f,
                self.qty * (f - self.entry_fill) - self.entry_commission - exit_comm,
            )
        } else {
            let f = buy_fill(exec_price, self.costs.slippage_pct); // cover
            let exit_comm = self.qty * f * comm / 100.0;
            self.cash += self.qty * (self.entry_fill - f) - exit_comm;
            (
                "cover",
                f,
                self.qty * (self.entry_fill - f) - self.entry_commission - exit_comm,
            )
        };
        self.closed_trades.push(pnl);
        self.in_position = false;
        self.qty = 0.0;
        self.margin = 0.0;
        self.entry_time_ms = None;
        if is_stop && self.risk.cooldown_minutes > 0.0 {
            self.cooldown_until_ms =
                Some(now_ms + (self.risk.cooldown_minutes * MS_MINUTE) as i64);
        }
        self.fill(side, f, traded, mark, if is_stop { "손절" } else { "청산" })
    }

    /// 현재가 한 틱. 체결이 났으면 하나 돌려준다 — 틱 하나에 체결은 **최대 하나**다.
    pub fn step(&mut self, price: f64, now_ms: i64) -> Option<Fill> {
        // 날이 바뀌면 그날의 기준 자산을 다시 잡는다.
        let day = day_index(now_ms);
        if self.day != Some(day) {
            self.day = Some(day);
            self.day_start_equity = self.equity(price);
            self.halted_today = false;
        }

        // 보유 중인 숏에는 틱마다 비용이 붙는다(현물은 0).
        if self.in_position && self.side == Side::Short && self.costs.funding_pct > 0.0 {
            self.cash -= self.qty * price * self.costs.funding_pct / 100.0;
        }

        if self.in_position {
            // 순서가 규칙이다(서버와 같다): 손절 → 최대 보유시간 → 일일 한도 → 익절.
            if self.stop_hit(price) {
                return Some(self.close(price, price, now_ms, true));
            }
            if let (Some(hours), Some(entered)) = (self.risk.max_holding_hours, self.entry_time_ms)
            {
                if now_ms - entered >= (hours * MS_HOUR) as i64 {
                    return Some(self.close(price, price, now_ms, false));
                }
            }
            if self.daily_loss_breached(price) {
                let fill = self.close(price, price, now_ms, false);
                self.halted_today = true;
                return Some(fill);
            }
            if self.profit_hit(price) {
                return Some(self.close(price, price, now_ms, false));
            }
            return None;
        }

        // 비어 있어도 일일 한도를 넘었으면 그날은 더 사지 않는다.
        if self.daily_loss_breached(price) {
            self.halted_today = true;
            return None;
        }
        if self.entry_blocked(now_ms) || !self.should_enter(price) {
            return None;
        }
        // 비어 있을 때 현금 == 자산이다.
        let margin = self.risk.invest_ratio * self.cash;
        if margin <= 0.0 {
            return None;
        }
        let notional = margin * self.costs.leverage;
        let (side, f) = if self.side == Side::Long {
            ("buy", buy_fill(price, self.costs.slippage_pct))
        } else {
            ("short", sell_fill(price, self.costs.slippage_pct))
        };
        self.qty = notional / f;
        self.entry_commission = notional * self.costs.commission_pct / 100.0;
        // 롱은 증거금까지 현금에서 빼고, 숏은 증거금을 현금에 남긴다(서버와 같다).
        self.cash -= if self.side == Side::Long {
            margin + self.entry_commission
        } else {
            self.entry_commission
        };
        self.entry_fill = f;
        self.margin = margin;
        self.in_position = true;
        self.entry_time_ms = Some(now_ms);
        Some(self.fill(side, f, self.qty, price, "진입"))
    }

    fn daily_loss_breached(&self, price: f64) -> bool {
        let Some(limit) = self.risk.daily_max_loss_pct.filter(|v| *v > 0.0) else { return false };
        if self.day.is_none() || self.day_start_equity <= 0.0 {
            return false;
        }
        (self.equity(price) - self.day_start_equity) / self.day_start_equity * 100.0 <= -limit
    }

    /// 웜업이 끝났다 — 장부만 되돌린다.
    pub fn warmup_done(&mut self) {
        self.cash = self.initial_capital;
        self.in_position = false;
        self.qty = 0.0;
        self.entry_fill = 0.0;
        self.entry_commission = 0.0;
        self.margin = 0.0;
        self.closed_trades.clear();
        self.day = None;
        self.day_start_equity = self.initial_capital;
        self.halted_today = false;
        self.cooldown_until_ms = None;
        self.entry_time_ms = None;
    }
}

/// **C — 적립식.** 정해진 날짜 간격마다 정해진 금액을 산다. 팔지 않는다(손절 제외).
///
/// 간격은 **달력**으로 센다. 예전에는 '봉 수' 로 셌는데, 실시간에서 한 스텝이 3초 틱이라
/// `interval_days=1` 이 '3초마다 매수' 가 됐다(모의 1분에 20회 매수). 일봉 위에서는 두 셈이
/// 같은 봉을 짚으므로 백테스트 결과는 그대로다.
///
/// **시각을 모르면 사지 않는다.** 적립식에서는 안 사는 쪽이 잘못 사는 쪽보다 안전하다.
pub struct DcaSim {
    pub costs: Costs,
    pub stop_loss_pct: Option<f64>,
    pub daily_max_loss_pct: Option<f64>,
    /// 한 번에 넣는 금액.
    pub amount_per_buy: f64,
    /// 며칠마다.
    pub interval_days: i64,

    pub initial_capital: f64,
    pub cash: f64,
    pub qty: f64,
    /// 지금까지 넣은 돈(수수료 포함).
    pub cost_basis: f64,
    /// 손절로 멈췄는가. 멈추면 더 사지 않는다.
    pub stopped: bool,
    pub buys_done: u32,
    next_buy_ms: Option<i64>,
    day: Option<i64>,
    day_start_equity: f64,
    halted_today: bool,
    /// 지금 시각(ms). 적립식은 **달력**이 기준이라 이 값이 전부다.
    pub clock_ms: i64,
}

impl DcaSim {
    pub fn new(
        costs: Costs,
        stop_loss_pct: Option<f64>,
        daily_max_loss_pct: Option<f64>,
        amount_per_buy: f64,
        interval_days: i64,
        initial_capital: f64,
    ) -> Self {
        Self {
            costs,
            stop_loss_pct,
            daily_max_loss_pct,
            amount_per_buy,
            interval_days: interval_days.max(1),
            initial_capital,
            cash: initial_capital,
            qty: 0.0,
            cost_basis: 0.0,
            stopped: false,
            buys_done: 0,
            next_buy_ms: None,
            day: None,
            day_start_equity: initial_capital,
            halted_today: false,
            clock_ms: 0,
        }
    }

    pub fn equity(&self, price: f64) -> f64 {
        self.cash + self.qty * price
    }

    /// 다음 매수 예정 시각 — 화면에 보여 준다.
    pub fn next_buy_ms(&self) -> Option<i64> {
        self.next_buy_ms
    }

    fn fill(&self, side: &'static str, price: f64, qty: f64, mark: f64, reason: &str) -> Fill {
        let equity_after = self.equity(mark);
        let return_pct = (equity_after - self.initial_capital) / self.initial_capital * 100.0;
        let qty_before = if is_exit(side) { self.qty + qty } else { (self.qty - qty).max(0.0) };
        Fill {
            side,
            price,
            qty,
            equity_after,
            return_pct,
            reason: reason.to_string(),
            qty_before,
        }
    }

    pub fn step(&mut self, price: f64, now_ms: i64) -> Option<Fill> {
        let day = day_index(now_ms);
        if self.day != Some(day) {
            self.day = Some(day);
            self.day_start_equity = self.equity(price);
            self.halted_today = false;
        }

        let mut fill = None;

        // 쌓아 둔 물량의 손절 — 넣은 돈 대비로 본다.
        if let Some(sl) = self.stop_loss_pct.filter(|v| *v > 0.0) {
            if self.qty > 0.0 && !self.stopped && self.qty * price <= self.cost_basis * (1.0 - sl / 100.0)
            {
                let f = sell_fill(price, self.costs.slippage_pct);
                let proceeds = self.qty * f;
                let traded = self.qty;
                self.cash += proceeds - proceeds * self.costs.commission_pct / 100.0;
                self.qty = 0.0;
                self.cost_basis = 0.0;
                self.stopped = true;
                fill = Some(self.fill("sell", f, traded, price, "손절"));
            }
        }

        // 일일 한도 — 그날은 더 사지 않는다(들고 있는 것은 그대로. 적립식은 쥐고 간다).
        if let Some(limit) = self.daily_max_loss_pct.filter(|v| *v > 0.0) {
            if self.day_start_equity > 0.0 {
                let drawdown =
                    (self.equity(price) - self.day_start_equity) / self.day_start_equity * 100.0;
                if drawdown <= -limit {
                    self.halted_today = true;
                }
            }
        }

        let scheduled = matches!(self.next_buy_ms, None) || matches!(self.next_buy_ms, Some(at) if now_ms >= at);
        let can_buy = !self.stopped
            && !self.halted_today
            && scheduled
            && self.cash >= self.amount_per_buy
            && fill.is_none();
        if can_buy {
            // 수수료를 **금액 안에서** 뗀다 — 정해진 금액만 쓰는 것이 적립식의 약속이다.
            let fee = self.amount_per_buy * self.costs.commission_pct / 100.0;
            let invest = self.amount_per_buy - fee;
            let f = buy_fill(price, self.costs.slippage_pct);
            let bought = invest / f;
            self.qty += bought;
            self.cash -= self.amount_per_buy;
            self.cost_basis += self.amount_per_buy;
            self.buys_done += 1;
            self.next_buy_ms = Some(now_ms + self.interval_days * MS_DAY);
            fill = Some(self.fill("buy", f, bought, price, "진입"));
        }
        fill
    }

    pub fn warmup_done(&mut self) {
        self.cash = self.initial_capital;
        self.qty = 0.0;
        self.cost_basis = 0.0;
        self.stopped = false;
        self.buys_done = 0;
        self.next_buy_ms = None;
        self.day = None;
        self.day_start_equity = self.initial_capital;
        self.halted_today = false;
    }
}
