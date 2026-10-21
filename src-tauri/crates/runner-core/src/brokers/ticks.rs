//! 호가 단위 — 지정가 가격을 거래소가 받는 단위로 맞춤(매도는 올림 · 매수는 내림, 가격 제한을 어기지 않게).
//!
//! 지정가는 호가 단위의 배수여야 거래소가 받음. 더 큰 단위의 배수는 늘 받음(표의 단위는 서로 배수 관계 —
//! 1000 · 500 · 100 · 50 · 10 · 5 · 1 · 0.1 …) — 그래서 확실하지 않은 구간은 큰 쪽을 씀(가격이 조금 보수적이 될 뿐).

use rust_decimal::prelude::*;

use crate::money::decimal_of;

/// 업비트 원화 마켓 — 2025-07-31 정책. 2026-10-21 공개 API(`/v1/orderbook/instruments` tick_size)와 대조함
/// (DOGE 110원 → 1 · SAND 77원 → 0.1 · SHIB 0.0069원 → 0.00001). (이 가격 이상, 단위) — 큰 가격부터.
pub const UPBIT_KRW: [(f64, f64); 17] = [
    (2_000_000.0, 1000.0),
    (1_000_000.0, 1000.0),
    (500_000.0, 500.0),
    (100_000.0, 100.0),
    (50_000.0, 50.0),
    (10_000.0, 10.0),
    (5_000.0, 5.0),
    (1_000.0, 1.0),
    (100.0, 1.0),
    (10.0, 0.1),
    (1.0, 0.01),
    (0.1, 0.001),
    (0.01, 0.0001),
    (0.001, 0.00001),
    (0.0001, 0.000_001),
    (0.00001, 0.000_000_1),
    (0.0, 0.000_000_01),
];

/// 빗썸 원화 마켓 — 호가 단위 조회 API 가 없음. 2026-10-21 실제 호가창(최우선 호가부터 간격)으로 확인한 구간:
/// 2,000,000원 이상 1,000(BTC · ETH) · 100,000~500,000원 100(SOL) · 1,000~10,000원 1(XRP) · 100~1,000원 1
/// (DOGE · GTC · TRX · HBAR · ADA) · 10~100원 0.01(SAND) · 0.001~0.01원 0.0001(SHIB · PEPE).
/// 확인 못 한 구간은 업비트 표와 옛 표 중 큰 단위(어긋나도 배수라 거래소가 받음).
pub const BITHUMB_KRW: [(f64, f64); 14] = [
    (1_000_000.0, 1000.0),
    (500_000.0, 500.0),
    (100_000.0, 100.0),
    (50_000.0, 50.0),
    (10_000.0, 10.0),
    (5_000.0, 5.0),
    (1_000.0, 1.0),
    (100.0, 1.0),
    (10.0, 0.01),
    (1.0, 0.01),
    (0.1, 0.001),
    (0.01, 0.0001),
    (0.001, 0.0001),
    (0.0, 0.0001),
];

/// 표에서 이 가격의 단위.
pub fn table_tick(table: &[(f64, f64)], price: f64) -> f64 {
    table.iter().find(|(floor, _)| price >= *floor).map(|(_, tick)| *tick).unwrap_or(0.0)
}

/// 가격을 단위의 배수로 — 올림(매도) · 내림(매수). 십진수로 계산해 0.1 같은 부동소수 오차가 없음.
/// 가격 · 단위가 0 이하이거나 결과가 0 이면 None.
pub fn round_to_tick(price: f64, tick: f64, up: bool) -> Option<f64> {
    if !(price > 0.0 && tick > 0.0) {
        return None;
    }
    let (p, t) = (decimal_of(price)?, decimal_of(tick)?);
    let steps = p / t;
    let steps = if up { steps.ceil() } else { steps.floor() };
    let rounded = (steps * t).normalize();
    if rounded.is_zero() {
        return None;
    }
    rounded.to_f64()
}

/// 한 단계 큰 단위 — 가격 단위가 맞지 않다고 거절되면 이것으로 한 번 다시 냄(10배 — 표의 어느 단위의 배수이기도 함).
pub fn coarser(tick: f64) -> f64 {
    decimal_of(tick).map(|t| (t * Decimal::TEN).normalize()).and_then(|t| t.to_f64()).unwrap_or(tick * 10.0)
}

/// 주문 본문에 쓰는 가격 글자 — 지수 표기 없이(0.0001 · 1000).
pub fn price_plain(price: f64) -> String {
    decimal_of(price).map(|d| d.normalize().to_string()).unwrap_or_else(|| format!("{price}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn upbit_table_matches_the_live_tick_sizes() {
        // 2026-10-21 공개 API tick_size 와 같은 값.
        assert_eq!(table_tick(&UPBIT_KRW, 111_142_000.0), 1000.0);
        assert_eq!(table_tick(&UPBIT_KRW, 142_100.0), 100.0);
        assert_eq!(table_tick(&UPBIT_KRW, 1_809.0), 1.0);
        assert_eq!(table_tick(&UPBIT_KRW, 110.0), 1.0);
        assert_eq!(table_tick(&UPBIT_KRW, 77.3), 0.1);
        assert_eq!(table_tick(&UPBIT_KRW, 0.0069), 0.00001);
        assert_eq!(table_tick(&UPBIT_KRW, 100.0), 1.0, "경계는 위 구간");
        assert_eq!(table_tick(&UPBIT_KRW, 99.99), 0.1);
    }

    #[test]
    fn bithumb_table_matches_what_its_order_book_shows() {
        assert_eq!(table_tick(&BITHUMB_KRW, 111_161_000.0), 1000.0);
        assert_eq!(table_tick(&BITHUMB_KRW, 142_200.0), 100.0);
        assert_eq!(table_tick(&BITHUMB_KRW, 402.0), 1.0);
        assert_eq!(table_tick(&BITHUMB_KRW, 77.32), 0.01);
        assert_eq!(table_tick(&BITHUMB_KRW, 0.0069), 0.0001);
    }

    #[test]
    fn sell_rounds_up_and_buy_rounds_down_without_float_noise() {
        assert_eq!(round_to_tick(435.54, 1.0, true), Some(436.0));
        assert_eq!(round_to_tick(435.54, 1.0, false), Some(435.0));
        assert_eq!(round_to_tick(77.31, 0.1, true), Some(77.4));
        assert_eq!(round_to_tick(77.3, 0.1, true), Some(77.3), "이미 배수면 그대로(0.1 오차로 77.4 가 되지 않음)");
        assert_eq!(round_to_tick(0.006_93, 0.0001, false), Some(0.0069));
        assert_eq!(round_to_tick(0.006_93, 0.0001, true), Some(0.007));
        assert_eq!(round_to_tick(111_142_400.0, 1000.0, true), Some(111_143_000.0));
        assert_eq!(round_to_tick(0.000_000_123, 0.000_000_01, false), Some(0.000_000_12));
        assert_eq!(round_to_tick(0.4, 1.0, false), None, "내림이 0 이면 낼 수 없음");
        assert_eq!(round_to_tick(-1.0, 1.0, true), None);
        assert_eq!(round_to_tick(1.0, 0.0, true), None);
    }

    #[test]
    fn coarser_is_ten_times_and_prices_print_plain() {
        assert_eq!(coarser(0.1), 1.0);
        assert_eq!(coarser(0.0001), 0.001);
        assert_eq!(coarser(1000.0), 10000.0);
        assert_eq!(price_plain(0.0001), "0.0001");
        assert_eq!(price_plain(111_143_000.0), "111143000");
        assert_eq!(price_plain(77.4), "77.4");
    }
}
