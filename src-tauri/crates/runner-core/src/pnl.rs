//! 수수료 반영 손익 — 화면 표시와 안전 규칙(하루 최대 손실·강제 청산)이 함께 쓰는 한 가지 계산.
//! - 평가손익(지금 정리하면) = 수량×(현재가−평단)×방향 − 남은 진입 수수료 − 수량×현재가×청산 수수료율
//! - 평가손익 % = 평가손익 ÷ 투입금, 투입금 = 수량×평단 + 남은 진입 수수료
//! - 실현손익 = 가격 손익 − 이번 정리 수수료 − 정리한 수량 몫의 진입 수수료(수량 비례)
//! - 서버로 가는 realized_pnl·unrealized_pct는 수수료 전(파이썬 실행기와 같음) — 여기서 건드리지 않음.
//!   수수료 반영 값(realized_net·unrealized_net·unrealized_net_pct)은 2026-10-18부터 서버에도 함께 보냄(웹이 같은 숫자를 보이게).

use crate::brokers::DOMESTIC_BID_FEE_FALLBACK;
use crate::money::pnl_amount;
use crate::types::{Exchange, Market, Side};

/// 바이낸스 현물 기본 수수료율(0.1%) — BNB 할인 전 값.
pub const BINANCE_SPOT_FEE: f64 = 0.001;
/// 바이낸스 선물 시장가(테이커) 기본 수수료율(0.05%).
pub const BINANCE_FUTURES_TAKER_FEE: f64 = 0.0005;
/// 업비트 원화 마켓 기본 수수료율(0.05%).
pub const UPBIT_FEE: f64 = 0.0005;

/// 수수료를 모를 때 쓰는 기본 수수료율 — 거래소·시장별. 모르는 거래소는 바이낸스 값.
/// 빗썸은 국내 주문 금액을 정할 때 쓰는 보수적인 값(0.25%)과 같음.
pub fn default_fee_rate(exchange: Option<Exchange>, market: Market) -> f64 {
    match (exchange, market) {
        (Some(Exchange::Upbit), _) => UPBIT_FEE,
        (Some(Exchange::Bithumb), _) => DOMESTIC_BID_FEE_FALLBACK,
        (_, Market::Futures) => BINANCE_FUTURES_TAKER_FEE,
        _ => BINANCE_SPOT_FEE,
    }
}

/// 거래소가 알려 준 수수료율로 쓸 만한 값인지 여부(0 이상 5% 미만).
pub fn plausible_rate(rate: f64) -> bool {
    rate.is_finite() && (0.0..0.05).contains(&rate)
}

/// 열린 포지션의 진입 수수료 중 이번에 정리한 수량 몫 — 수량 비례. 다 정리하면 전부.
pub fn allocate_entry_fee(open_fee: f64, held_qty: f64, closed_qty: f64) -> f64 {
    if open_fee <= 0.0 || held_qty <= 0.0 {
        return 0.0;
    }
    open_fee * (closed_qty / held_qty).clamp(0.0, 1.0)
}

/// 지금 정리하면 남는 평가손익(견적 통화).
pub fn net_unrealized(qty: f64, entry: f64, price: f64, side: Side, entry_fee: f64, exit_rate: f64) -> f64 {
    if qty <= 0.0 || entry <= 0.0 || price <= 0.0 {
        return 0.0;
    }
    pnl_amount(qty, entry, price, side) - entry_fee - qty * price * exit_rate
}

/// 투입금 — 수량×평단 + 남은 진입 수수료(지금 포지션에 들어간 돈).
pub fn invested(qty: f64, entry: f64, entry_fee: f64) -> f64 {
    if qty <= 0.0 || entry <= 0.0 {
        return 0.0;
    }
    qty * entry + entry_fee.max(0.0)
}

/// 평가손익 % — 금액 ÷ 투입금 × 100. 투입금이 없으면 0.
pub fn pct_of(amount: f64, invested: f64) -> f64 {
    if invested > 0.0 {
        amount / invested * 100.0
    } else {
        0.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn near(a: f64, b: f64) -> bool {
        (a - b).abs() < 1e-6
    }

    #[test]
    fn default_rates_per_exchange_and_market() {
        assert_eq!(default_fee_rate(Some(Exchange::Binance), Market::Spot), 0.001);
        assert_eq!(default_fee_rate(Some(Exchange::Binance), Market::Futures), 0.0005);
        assert_eq!(default_fee_rate(Some(Exchange::Upbit), Market::Spot), 0.0005);
        assert_eq!(default_fee_rate(Some(Exchange::Bithumb), Market::Spot), 0.0025);
        assert_eq!(default_fee_rate(None, Market::Spot), 0.001);
    }

    #[test]
    fn binance_spot_up_015_pct_is_still_a_loss_after_fees() {
        // 1,000 USDT 매수(수수료 0.1% = 1 USDT) 뒤 +0.15% — 수수료 전 +1.50, 지금 정리하면 −0.5015.
        let (qty, entry) = (10.0, 100.0);
        let price = 100.15;
        let amount = net_unrealized(qty, entry, price, Side::Long, 1.0, BINANCE_SPOT_FEE);
        assert!(near(amount, 1.5 - 1.0 - 1.0015), "{amount}");
        assert!(amount < 0.0);
        let base = invested(qty, entry, 1.0);
        assert!(near(base, 1001.0));
        assert!(near(pct_of(amount, base), -0.5015 / 1001.0 * 100.0));
    }

    #[test]
    fn short_profit_uses_the_same_formula() {
        // 숏 2개 100 → 90: 가격 손익 +20, 진입 수수료 0.1, 청산 수수료 2×90×0.0005 = 0.09.
        let amount = net_unrealized(2.0, 100.0, 90.0, Side::Short, 0.1, 0.0005);
        assert!(near(amount, 20.0 - 0.1 - 0.09));
    }

    #[test]
    fn entry_fee_is_allocated_by_closed_quantity() {
        assert!(near(allocate_entry_fee(3.0, 1.5, 0.5), 1.0));
        assert!(near(allocate_entry_fee(3.0, 1.5, 1.5), 3.0));
        assert!(near(allocate_entry_fee(3.0, 1.5, 2.0), 3.0), "보유보다 많이 정리해도 전부까지만");
        assert_eq!(allocate_entry_fee(3.0, 0.0, 1.0), 0.0);
    }

    #[test]
    fn flat_or_broken_inputs_are_zero() {
        assert_eq!(net_unrealized(0.0, 100.0, 101.0, Side::Long, 1.0, 0.001), 0.0);
        assert_eq!(invested(1.0, 0.0, 1.0), 0.0);
        assert_eq!(pct_of(5.0, 0.0), 0.0);
        assert!(plausible_rate(0.0) && plausible_rate(0.0025) && !plausible_rate(0.05) && !plausible_rate(f64::NAN));
    }
}
