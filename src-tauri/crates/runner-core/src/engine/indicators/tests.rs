//! 지표가 서버와 **같은 값**을 내는지.
//!
//! 기대값은 서버 지표(파이썬 `engine/candles.py`)에 **판단 계약과 같은 봉**을 흘려보내
//! 받아 적은 것임. 손으로 계산하지 않았음 — 손으로 하면 내 산수를 검증할 뿐임.
//!
//! 지표가 한 자리만 달라도 F·G·I·J가 전부 다른 자리에서 사고팖.

use super::*;

/// 계약과 같은 봉의 종가. `tests/fixtures/engine_golden.json`의 `candles[*][4]`임.
fn closes() -> Vec<f64> {
    let path: std::path::PathBuf =
        [env!("CARGO_MANIFEST_DIR"), "tests", "fixtures", "engine_golden.json"].iter().collect();
    let raw = std::fs::read_to_string(path).expect("판단 계약");
    let golden: serde_json::Value = serde_json::from_str(&raw).expect("판단 계약 모양");
    golden["candles"].as_array().expect("봉").iter().map(|row| row[4].as_f64().expect("종가")).collect()
}

#[track_caller]
fn near(got: f64, want: f64) {
    let scale = want.abs().max(1.0);
    assert!((got - want).abs() <= 1e-12 * scale, "서버와 다르다: got {got:.17}, want {want:.17}");
}

/// 값이 나오기 시작한 **첫 자리**와 마지막 몇 개를 함께 봄.
/// 첫 자리가 밀리면 웜업이 끝나는 시점이 달라져 초반 판단이 통째로 어긋남.
fn feed<T, F: FnMut(f64) -> Option<T>>(mut update: F) -> (usize, Vec<T>) {
    let mut first = usize::MAX;
    let mut values = Vec::new();
    for (i, close) in closes().into_iter().enumerate() {
        if let Some(v) = update(close) {
            if first == usize::MAX {
                first = i;
            }
            values.push(v);
        }
    }
    (first, values)
}

#[test]
fn rsi_가_서버와_같다() {
    let mut rsi = Rsi::new(7);
    let (first, values) = feed(|c| rsi.update(c));
    assert_eq!(first, 7, "첫 값이 나오는 자리가 서버와 다르다");
    let want = [20.74414947566696, 24.791521889455097, 18.743700291920675, 18.73302922092651];
    for (got, want) in values[values.len() - 4..].iter().zip(want) {
        near(*got, want);
    }
}

#[test]
fn rsi_는_내린_봉이_없으면_백을_준다() {
    // 나누기 0을 피하는 자리. 오르기만 한 구간에서 실제로 걸림.
    let mut rsi = Rsi::new(3);
    let mut last = None;
    for close in [1.0, 2.0, 3.0, 4.0, 5.0] {
        last = rsi.update(close);
    }
    near(last.expect("값"), 100.0);
}

#[test]
fn 단순이동평균이_서버와_같다() {
    let mut ma = Ma::new(MaType::Sma, 20);
    let (first, values) = feed(|c| ma.update(c));
    assert_eq!(first, 19, "20봉 평균은 20번째 봉에서 처음 나온다");
    let want = [1.7935899632153596, 1.733551156602362, 1.6741991931549496, 1.614863619789493];
    for (got, want) in values[values.len() - 4..].iter().zip(want) {
        near(*got, want);
    }
}

#[test]
fn 지수이동평균이_서버와_같다() {
    let mut ma = Ma::new(MaType::Ema, 12);
    let (first, values) = feed(|c| ma.update(c));
    assert_eq!(first, 11, "첫 값은 단순 평균으로 심는다");
    let want = [1.5530055304315444, 1.5140658703161167, 1.4568701986206423, 1.4084253310004398];
    for (got, want) in values[values.len() - 4..].iter().zip(want) {
        near(*got, want);
    }
}

#[test]
fn 볼린저_밴드가_서버와_같다() {
    let mut bb = Bollinger::new(10, 2.0);
    let (first, values) = feed(|c| bb.update(c));
    assert_eq!(first, 9);
    let want: [(f64, f64, f64); 2] = [
        (1.376335163767751, 1.6773076492436267, 1.0753626782918753),
        (1.327458984928285, 1.6051446378591119, 1.049773331997458),
    ];
    for ((mid, up, low), (w_mid, w_up, w_low)) in values[values.len() - 2..].iter().zip(want) {
        near(*mid, w_mid);
        near(*up, w_up);
        near(*low, w_low);
    }
}

#[test]
fn 밴드는_모집단_분산을_쓴다() {
    // 표본 분산(n-1)으로 바꾸면 밴드가 넓어져 진입이 늦어짐. 손으로 셀 수 있는 값으로 못 박음.
    let mut bb = Bollinger::new(4, 1.0);
    let mut bands = None;
    for close in [1.0, 2.0, 3.0, 4.0] {
        bands = bb.update(close);
    }
    let (mid, up, low) = bands.expect("값");
    near(mid, 2.5);
    // 분산 = ((1.5² + 0.5²) × 2) / 4 = 1.25 → 표준편차 = 1.118033988749895
    near(up, 2.5 + 1.25_f64.sqrt());
    near(low, 2.5 - 1.25_f64.sqrt());
}

#[test]
fn 창이_덜_찼으면_값을_주지_않는다() {
    // 반쪽 지표로 판단하면 처음 몇 봉에서만 다른 답이 나옴 — 제일 찾기 어려운 어긋남임.
    let mut sma = Ma::new(MaType::Sma, 5);
    for close in [1.0, 2.0, 3.0, 4.0] {
        assert!(sma.update(close).is_none());
    }
    assert!(sma.update(5.0).is_some());

    let mut bb = Bollinger::new(5, 2.0);
    for close in [1.0, 2.0, 3.0, 4.0] {
        assert!(bb.update(close).is_none());
    }
    assert!(bb.update(5.0).is_some());

    let mut rsi = Rsi::new(5);
    for close in [1.0, 2.0, 3.0, 4.0, 5.0] {
        assert!(rsi.update(close).is_none(), "첫 종가는 기준만 잡고, 변화 5개를 더 봐야 한다");
    }
    assert!(rsi.update(6.0).is_some());
}

#[test]
fn 창은_길이를_넘기지_않는다() {
    // 창이 자라면 옛 봉이 영원히 평균에 남음.
    let mut sma = Ma::new(MaType::Sma, 3);
    for close in [10.0, 20.0, 30.0, 40.0, 50.0] {
        sma.update(close);
    }
    near(sma.update(60.0).expect("값"), (40.0 + 50.0 + 60.0) / 3.0);
}
