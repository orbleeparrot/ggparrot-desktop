//! 장부 — 돈과 수량을 세는 계산기.
//!
//! "언제 살까" 를 정하는 것은 규칙(D · E · F · G · H · I)이고, 그 결정이 떨어진 뒤
//! **얼마에 몇 개를 사고 돈이 어떻게 되는지**를 전부 여기서 센다. 슬리피지 · 수수료 ·
//! 평단 · 분할 청산 · 실현손익이 전부 장부의 일이다.
//!
//! 여섯 규칙이 전부 이 위에 올라탄다. 그래서 장부가 1원이라도 다르면 **여섯 규칙이 전부**
//! 어긋난다 — 판단 계약(`tests/fixtures/engine_golden.json`)의 `signal_price` ·
//! `notional_frac` · `qty_frac` 이 전부 여기서 나온 숫자다.
//!
//! 서버(파이썬 `backend/engine/candles.py` 의 `CandleSim`, `stepper.py`)와 **식까지 같게**
//! 두었다. 식을 '정리' 하면 부동소수점 결과가 달라져 계약이 깨진다 — 같은 순서로 더하고 뺀다.

/// 포지션 방향. 지금 실행기가 받는 매크로는 전부 롱이지만(국내 현물은 롱 1배만),
/// 장부 식은 숏도 서버와 같게 둔다 — 한쪽만 옮기면 나중에 갈린다.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Side {
    Long,
    Short,
}

/// 진입 한 번이 만든 묶음. 그리드 · 마틴게일은 여러 개가 쌓인다.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Lot {
    /// 수량.
    pub qty: f64,
    /// 체결가(슬리피지를 이미 반영한 값).
    pub fill: f64,
    /// 이 묶음에 묶인 **자기 돈**. 레버리지 명목금액이 아니다(1배면 `qty * fill` 과 같다).
    pub margin: f64,
}

/// 체결 하나. 서버 `stepper.Fill` 과 같은 칸을 갖는다.
#[derive(Debug, Clone, PartialEq)]
pub struct Fill {
    /// `buy` | `sell` | `short` | `cover`.
    pub side: &'static str,
    pub price: f64,
    pub qty: f64,
    pub equity_after: f64,
    pub return_pct: f64,
    /// 사람이 읽는 한 줄("RSI 23.1 ≤ 25 · 진입"). 활동 기록과 주문 명령에 그대로 간다.
    pub reason: String,
    /// 이 체결 **직전**의 보유 수량. 청산 비율(`qty / qty_before`)을 낼 때 쓴다.
    pub qty_before: f64,
}

/// 청산 쪽 체결인가. 서버 `EXIT_SIDES` 와 같다.
pub fn is_exit(side: &str) -> bool {
    side == "sell" || side == "cover"
}

fn default_reason(side: &str) -> &'static str {
    if is_exit(side) {
        "청산"
    } else {
        "진입"
    }
}

/// 살 때는 불리하게(비싸게) 체결된다 — 시장가라 호가를 올라타기 때문이다.
pub fn buy_fill(close: f64, slippage_pct: f64) -> f64 {
    close * (1.0 + slippage_pct / 100.0)
}

/// 팔 때는 불리하게(싸게) 체결된다.
pub fn sell_fill(close: f64, slippage_pct: f64) -> f64 {
    close * (1.0 - slippage_pct / 100.0)
}

/// 수수료 · 슬리피지 · 레버리지 — 매크로의 `fees` · `leverage` 에서 온다.
#[derive(Debug, Clone, Copy)]
pub struct Costs {
    /// 한쪽 수수료(%).
    pub commission_pct: f64,
    /// 체결 밀림(%).
    pub slippage_pct: f64,
    /// 보유 중인 숏에 봉마다 붙는 비용(%). 현물은 0.
    pub funding_pct: f64,
    /// 1 이면 현물(청산 없음).
    pub leverage: f64,
}

impl Default for Costs {
    fn default() -> Self {
        Self { commission_pct: 0.0, slippage_pct: 0.0, funding_pct: 0.0, leverage: 1.0 }
    }
}

/// 현금 · 보유 묶음 · 실현손익을 센다.
#[derive(Debug, Clone)]
pub struct Ledger {
    pub side: Side,
    pub costs: Costs,
    /// 이 세션이 굴리는 돈. 수익률의 분모이자 진입 금액 비율의 기준이다.
    pub initial_capital: f64,
    /// 아직 포지션에 안 들어간 돈.
    pub cash: f64,
    pub lots: Vec<Lot>,
    /// 청산마다 쌓이는 실현손익(수수료 뺀 값).
    pub closed_trades: Vec<f64>,
}

impl Ledger {
    pub fn new(side: Side, costs: Costs, initial_capital: f64) -> Self {
        Self { side, costs, initial_capital, cash: initial_capital, lots: Vec::new(), closed_trades: Vec::new() }
    }

    // --- 조회 ---------------------------------------------------------
    pub fn total_qty(&self) -> f64 {
        self.lots.iter().map(|l| l.qty).sum()
    }

    pub fn in_position(&self) -> bool {
        self.total_qty().abs() > 1e-12
    }

    /// 평단 — 묶음들의 수량 가중 평균 체결가.
    pub fn avg_entry(&self) -> f64 {
        let q = self.total_qty();
        if q <= 1e-12 {
            return 0.0;
        }
        self.lots.iter().map(|l| l.qty * l.fill).sum::<f64>() / q
    }

    /// 이 포지션에 묶인 **자기 돈**. 레버리지 명목금액이 아니다 — 사용자가 "내 자금의 몇
    /// 퍼센트를 시장에 넣었나" 라고 할 때 뜻하는 값이다.
    pub fn committed_margin(&self) -> f64 {
        self.lots.iter().map(|l| l.margin).sum()
    }

    /// 지금 가격 기준 자산. 서버 `CandleSim.equity` 와 식까지 같다.
    pub fn equity(&self, price: f64) -> f64 {
        if self.side == Side::Short {
            // 숏은 묶음 하나만 쓴다.
            return match self.lots.first() {
                None => self.cash,
                Some(lot) => self.cash + lot.qty * (lot.fill - price),
            };
        }
        let margin: f64 = self.lots.iter().map(|l| l.margin).sum();
        let unreal: f64 = self.lots.iter().map(|l| l.qty * (price - l.fill)).sum();
        self.cash + margin + unreal
    }

    /// 체결 한 줄을 만든다. 자산은 **`mark`**(판단 기준가)로 재고, 체결가로 재지 않는다 —
    /// 서버와 같게. 장부가 이미 바뀐 뒤에 불리므로 직전 수량은 되계산한다.
    fn fill(&self, side: &'static str, price: f64, qty: f64, mark: f64, reason: &str) -> Fill {
        let equity_after = self.equity(mark);
        let return_pct = (equity_after - self.initial_capital) / self.initial_capital * 100.0;
        let after = self.total_qty().abs();
        let qty_before = if is_exit(side) { after + qty } else { (after - qty).max(0.0) };
        let reason = if reason.is_empty() { default_reason(side).to_string() } else { reason.to_string() };
        Fill { side, price, qty, equity_after, return_pct, reason, qty_before }
    }

    // --- 열기 · 닫기 ---------------------------------------------------
    /// 롱 진입. `margin` 은 넣을 **자기 돈**(현금보다 많으면 현금까지만).
    ///
    /// 돈이 없거나 너무 적으면 None — 아무 일도 일어나지 않는다.
    pub fn open_long(&mut self, margin: f64, close: f64, mark: f64, reason: &str) -> Option<Fill> {
        let margin = margin.min(self.cash);
        if margin <= 1e-9 {
            return None;
        }
        let notional = margin * self.costs.leverage;
        let f = buy_fill(close, self.costs.slippage_pct);
        let qty = notional / f;
        self.cash -= margin + notional * self.costs.commission_pct / 100.0;
        self.lots.push(Lot { qty, fill: f, margin });
        Some(self.fill("buy", f, qty, mark, reason))
    }

    /// 숏 진입. 서버와 달리 `invest_ratio` 를 여기서 곱하지 않는다 — 부르는 쪽이 정한다.
    pub fn open_short(&mut self, margin: f64, close: f64, mark: f64, reason: &str) -> Option<Fill> {
        if margin <= 1e-9 {
            return None;
        }
        let notional = margin * self.costs.leverage;
        let f = sell_fill(close, self.costs.slippage_pct);
        let qty = notional / f;
        self.cash -= notional * self.costs.commission_pct / 100.0;
        self.lots.push(Lot { qty, fill: f, margin });
        Some(self.fill("short", f, qty, mark, reason))
    }

    /// 전량 청산. 보유가 없으면 None.
    pub fn close_all(&mut self, close: f64, mark: f64, reason: &str) -> Option<Fill> {
        if !self.in_position() {
            return None;
        }
        let qty = self.total_qty();
        let avg = self.avg_entry();
        let margin: f64 = self.lots.iter().map(|l| l.margin).sum();
        let comm = self.costs.commission_pct;
        let (side, f, pnl) = if self.side == Side::Short {
            let f = buy_fill(close, self.costs.slippage_pct); // cover
            let exit_comm = qty * f * comm / 100.0;
            // 진입 수수료는 들어갈 때 이미 현금에서 나갔다.
            self.cash += qty * (avg - f) - exit_comm;
            ("cover", f, qty * (avg - f) - exit_comm)
        } else {
            let f = sell_fill(close, self.costs.slippage_pct);
            let exit_comm = qty * f * comm / 100.0;
            // 묶였던 자기 돈 + 실현손익을 돌려준다(1배면 qty * f 와 같다).
            self.cash += margin + qty * (f - avg) - exit_comm;
            ("sell", f, qty * (f - avg) - exit_comm)
        };
        self.closed_trades.push(pnl);
        self.lots.clear();
        Some(self.fill(side, f, qty, mark, reason))
    }

    /// 롱 장부의 `fraction`(0 < f ≤ 1)만 판다. 묶음마다 같은 비율로 줄인다.
    ///
    /// 숏이거나 보유가 없으면 None. 전량은 [`Ledger::close_all`] 을 쓴다.
    pub fn reduce_long(&mut self, fraction: f64, close: f64, mark: f64, reason: &str) -> Option<Fill> {
        if fraction <= 0.0 || self.side == Side::Short || !self.in_position() {
            return None;
        }
        let fraction = fraction.min(1.0);
        let f = sell_fill(close, self.costs.slippage_pct);
        let comm = self.costs.commission_pct;
        let mut sold_qty = 0.0;
        let mut pnl = 0.0;
        for lot in self.lots.iter_mut() {
            let q = lot.qty * fraction;
            let m = lot.margin * fraction;
            let exit_comm = q * f * comm / 100.0;
            self.cash += m + q * (f - lot.fill) - exit_comm;
            pnl += q * (f - lot.fill) - exit_comm;
            lot.qty -= q;
            lot.margin -= m;
            sold_qty += q;
        }
        self.closed_trades.push(pnl);
        // 먼지만 남은 묶음은 버린다.
        self.lots.retain(|l| l.qty > 1e-12);
        Some(self.fill("sell", f, sold_qty, mark, reason))
    }

    /// 보유 중인 숏에 봉마다 붙는 비용. 현물(`funding_pct == 0`)은 아무 일도 없다.
    pub fn apply_funding(&mut self, close: f64) {
        if self.side != Side::Short || self.costs.funding_pct == 0.0 || !self.in_position() {
            return;
        }
        self.cash -= self.total_qty() * close * self.costs.funding_pct / 100.0;
    }

    /// 손절선에 닿았는가 — 봉의 **저가**(롱) · **고가**(숏)로 본다.
    ///
    /// 현재가가 아니라 봉의 극값으로 보는 이유: 봉 안에서 스쳤다면 실제로는 체결됐다.
    /// 종가만 보면 "그 사이 손절선을 지났는데 안 팔린" 것이 된다.
    pub fn stop_hit(&self, low: f64, high: f64, stop_loss_pct: f64) -> bool {
        if stop_loss_pct <= 0.0 || !self.in_position() {
            return false;
        }
        let avg = self.avg_entry();
        if self.side == Side::Short {
            high >= avg * (1.0 + stop_loss_pct / 100.0)
        } else {
            low <= avg * (1.0 - stop_loss_pct / 100.0)
        }
    }
}

#[cfg(test)]
mod tests;
