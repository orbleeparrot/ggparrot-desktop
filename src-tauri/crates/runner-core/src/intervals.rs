//! 봉 격자 — 한 봉이 **언제 열리고 언제 닫히는가**.
//!
//! 판단이 실행기로 내려오면 서버(파이썬 `backend/engine/intervals.py`)와 여기가 각자 봉
//! 경계를 계산한다. 경계가 한 칸만 어긋나도 같은 매크로가 다른 봉을 보고 **다른 자리에서**
//! 사고판다. 화면에는 아무 표시가 없다.
//!
//! 까다로운 데가 하나 있다 — **빗썸의 봉은 KST 경계에서 시작한다.** 업비트와 바이낸스는
//! UTC 경계다. 그래서 빗썸 4시간봉은 03:00 UTC 에, 일봉은 15:00 UTC 에 열린다. 이걸 모르면
//! 빗썸 일봉의 하루가 9시간 어긋난다.
//!
//! 파이썬 쪽과 **식까지 같게** 두었다. 값이 갈리면 `tests`의 대조표가 먼저 깨진다.

const MS_MINUTE: i64 = 60_000;
const MS_HOUR: i64 = 60 * MS_MINUTE;
const MS_DAY: i64 = 24 * MS_HOUR;
/// 빗썸 봉이 열리는 기준 시간대(KST = UTC+9)와 UTC 의 차이.
const KST_SHIFT_MS: i64 = 9 * MS_HOUR;

/// 간격 이름 → 길이(ms). 매크로의 `candle_interval` 이 쓰는 값이 전부다.
pub const INTERVALS: &[(&str, i64)] = &[
    ("1m", MS_MINUTE),
    ("3m", 3 * MS_MINUTE),
    ("5m", 5 * MS_MINUTE),
    ("15m", 15 * MS_MINUTE),
    ("1h", MS_HOUR),
    ("4h", 4 * MS_HOUR),
    ("1d", MS_DAY),
];

/// `interval` 한 봉의 길이(ms). 모르는 간격이면 None.
pub fn step_ms(interval: &str) -> Option<i64> {
    INTERVALS.iter().find(|(name, _)| *name == interval).map(|(_, ms)| *ms)
}

/// 봉 격자가 UTC 0시에서 얼마나 밀려 있는가(ms).
///
/// 빗썸만 KST 경계이므로 0 이 아니다 — 4시간봉 03:00 UTC, 일봉 15:00 UTC.
/// 1시간 이하는 9시간이 간격의 배수라 결과가 0 이지만 식은 그대로 둔다(간격이 늘어나도 맞는다).
pub fn grid_offset(exchange: &str, interval: &str) -> i64 {
    let Some(step) = step_ms(interval) else { return 0 };
    if exchange.eq_ignore_ascii_case("bithumb") {
        (-KST_SHIFT_MS).rem_euclid(step)
    } else {
        0
    }
}

/// `now_ms` 가 속한 봉이 열린 시각.
pub fn bar_open_ms(interval: &str, now_ms: i64, exchange: &str) -> Option<i64> {
    let step = step_ms(interval)?;
    let offset = grid_offset(exchange, interval);
    Some((now_ms - offset).div_euclid(step) * step + offset)
}

/// `now_ms` **다음에** 봉이 닫히는 시각. 경계 위에 정확히 있으면 다음 경계를 준다
/// (같은 시각을 돌려주면 폴링 루프가 0초 대기로 돈다).
pub fn next_close_ms(interval: &str, now_ms: i64, exchange: &str) -> Option<i64> {
    let step = step_ms(interval)?;
    let offset = grid_offset(exchange, interval);
    Some(((now_ms - offset).div_euclid(step) + 1) * step + offset)
}

/// 거래소가 준 봉 시작 시각이 이 격자에 맞는가. 어긋나면 간격이나 거래소를 잘못 안 것이다.
pub fn is_grid_aligned(t_ms: i64, interval: &str, exchange: &str) -> bool {
    match step_ms(interval) {
        Some(step) => (t_ms - grid_offset(exchange, interval)).rem_euclid(step) == 0,
        None => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 서버(파이썬 `engine/intervals.py`)가 같은 입력에 내는 값. 손으로 옮겨 적은 대조표다 —
    /// 여기가 갈리면 같은 매크로가 서버와 실행기에서 **다른 봉**을 본다.
    const OFFSET_TABLE: &[(&str, &str, i64)] = &[
        ("binance", "1m", 0),
        ("binance", "1h", 0),
        ("binance", "4h", 0),
        ("binance", "1d", 0),
        ("upbit", "1m", 0),
        ("upbit", "1h", 0),
        ("upbit", "4h", 0),
        ("upbit", "1d", 0),
        ("bithumb", "1m", 0),
        ("bithumb", "5m", 0),
        ("bithumb", "15m", 0),
        ("bithumb", "1h", 0),
        ("bithumb", "4h", 3 * MS_HOUR),
        ("bithumb", "1d", 15 * MS_HOUR),
    ];

    #[test]
    fn 격자_오프셋이_서버와_같다() {
        for (exchange, interval, want) in OFFSET_TABLE {
            assert_eq!(grid_offset(exchange, interval), *want, "{exchange} {interval}");
        }
    }

    #[test]
    fn 빗썸_일봉은_한국시간_자정에_열린다() {
        // 2026-10-20 07:00 UTC = 16:00 KST → 그날 KST 봉은 2026-10-19 15:00 UTC 에 열렸다.
        let now = 1_792_476_000_000; // 2026-10-20T07:00:00Z
        let open = bar_open_ms("1d", now, "bithumb").unwrap();
        assert_eq!((open % MS_DAY) / MS_HOUR, 15);
        // 업비트 · 바이낸스는 UTC 자정.
        for exchange in ["upbit", "binance"] {
            let open = bar_open_ms("1d", now, exchange).unwrap();
            assert_eq!(open % MS_DAY, 0, "{exchange}");
        }
    }

    #[test]
    fn 경계_위에_있으면_다음_경계를_준다() {
        let on_boundary = 7 * MS_HOUR;
        assert_eq!(next_close_ms("1h", on_boundary, "binance").unwrap(), 8 * MS_HOUR);
        assert_eq!(bar_open_ms("1h", on_boundary, "binance").unwrap(), on_boundary);
    }

    #[test]
    fn 음수_시각에서도_내림이_맞다() {
        // 1970 이전은 실무에 없지만, rem_euclid 가 아니라 % 를 쓰면 여기서 격자가 뒤집힌다.
        let before_epoch = -(3 * MS_HOUR) - 1;
        let open = bar_open_ms("1h", before_epoch, "binance").unwrap();
        assert!(open <= before_epoch && before_epoch < open + MS_HOUR);
        assert!(is_grid_aligned(open, "1h", "binance"));
    }

    #[test]
    fn 격자에_맞는지_가른다() {
        assert!(is_grid_aligned(15 * MS_HOUR, "1d", "bithumb"));
        assert!(!is_grid_aligned(0, "1d", "bithumb"), "UTC 자정은 빗썸 일봉 경계가 아니다");
        assert!(is_grid_aligned(0, "1d", "upbit"));
        assert!(!is_grid_aligned(7 * MS_HOUR + 1, "1h", "binance"));
    }

    #[test]
    fn 모르는_간격은_없다고_말한다() {
        assert_eq!(step_ms("2h"), None);
        assert_eq!(bar_open_ms("2h", 0, "binance"), None);
        assert_eq!(next_close_ms("2h", 0, "binance"), None);
        assert!(!is_grid_aligned(0, "2h", "binance"));
    }

    #[test]
    fn 매크로가_쓰는_간격이_모두_있다() {
        // 서버 schema.py 의 CANDLE_TYPES 가 받는 값 — 하나라도 빠지면 그 매크로는 못 돈다.
        for interval in ["1m", "3m", "5m", "15m", "1h", "4h", "1d"] {
            assert!(step_ms(interval).is_some(), "{interval}");
        }
    }
}
