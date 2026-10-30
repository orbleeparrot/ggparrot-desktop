//! 지표선이 **엔진이 보는 값과 같은지**. 다르면 화면이 거짓을 그림.
use super::*;
use crate::engine::indicators::MaType;

fn bars(closes: &[f64]) -> Vec<Bar> {
    closes
        .iter()
        .enumerate()
        .map(|(i, &c)| Bar { t: i as i64 * 60_000, o: c, h: c + 1.0, l: c - 1.0, c, v: None })
        .collect()
}

#[test]
fn 이동평균선은_같은_설정의_지표를_그대로_돌린_값이다() {
    let rows = bars(&[10.0, 11.0, 12.0, 13.0, 14.0]);
    let live = Ma::new(MaType::Sma, 3);
    let line = ma_series(&live, &rows, "ma_fast", "단기 3".into());

    let mut check = Ma::new(MaType::Sma, 3);
    let want: Vec<Option<f64>> = rows.iter().map(|b| check.update(b.c)).collect();
    assert_eq!(line.values, want);
    assert_eq!(line.values.len(), rows.len());
}

#[test]
fn 값이_없는_앞쪽은_비워_둔다() {
    // 0으로 채우면 화면이 그 0을 진짜 지표값으로 그림.
    let rows = bars(&[10.0, 11.0, 12.0, 13.0]);
    let line = ma_series(&Ma::new(MaType::Sma, 3), &rows, "ma", "단기 3".into());
    assert_eq!(line.values[0], None);
    assert_eq!(line.values[1], None);
    assert!(line.values[2].is_some());
}

#[test]
fn 돌고_있는_지표의_웜업_상태가_섞이지_않는다() {
    // fresh()가 아니라 복제본을 쓰면 이미 먹은 봉 때문에 값이 밀림.
    let mut live = Ma::new(MaType::Sma, 3);
    for close in [100.0, 200.0, 300.0, 400.0] {
        live.update(close);
    }
    let rows = bars(&[10.0, 11.0, 12.0, 13.0]);
    let line = ma_series(&live, &rows, "ma", "단기 3".into());
    assert_eq!(line.values[0], None, "다시 먹였으므로 앞쪽은 비어 있어야 한다");
    assert_eq!(line.values[2], Some(11.0));
}

#[test]
fn 볼린저는_세_줄을_같은_길이로_낸다() {
    let rows = bars(&[10.0, 12.0, 11.0, 13.0, 12.0, 14.0]);
    let lines = bollinger_series(&Bollinger::new(3, 2.0), &rows);
    assert_eq!(lines.len(), 3);
    assert_eq!(lines.iter().map(|s| s.id).collect::<Vec<_>>(), ["band_upper", "band_mid", "band_lower"]);
    for line in &lines {
        assert_eq!(line.values.len(), rows.len());
    }
    // 위 ≥ 중심 ≥ 아래.
    for i in 0..rows.len() {
        if let (Some(u), Some(m), Some(l)) = (lines[0].values[i], lines[1].values[i], lines[2].values[i]) {
            assert!(u >= m && m >= l, "{i}: {u} {m} {l}");
        }
    }
}

#[test]
fn rsi_는_문턱과_함께_간다() {
    let rows = bars(&[10.0, 11.0, 10.5, 12.0, 11.5, 13.0, 12.5, 14.0]);
    let osc = rsi_oscillator(&Rsi::new(3), &rows, Some(30.0), Some(70.0));
    assert_eq!(osc.values.len(), rows.len());
    assert_eq!(osc.entry, Some(30.0));
    assert_eq!(osc.exit, Some(70.0));
    assert_eq!(osc.label, "RSI 3");
    for value in osc.values.iter().flatten() {
        assert!((0.0..=100.0).contains(value), "RSI 는 0~100 이다: {value}");
    }
}

#[test]
fn 돌파선은_봉마다_그_봉_시가에서_새로_잡힌다() {
    // 지금 봉의 선 하나만 가로로 그으면 "이 값이 늘 기준이었다"로 읽힘 — 실제로는
    // 봉마다 `그 봉 시가 + k x 직전 봉 변동폭`으로 새로 잡힘.
    use crate::engine::rules::breakout::{Breakout, ExitMode};
    let rows = vec![
        Bar { t: 0, o: 100.0, h: 110.0, l: 90.0, c: 105.0, v: None },
        Bar { t: 60_000, o: 106.0, h: 112.0, l: 104.0, c: 108.0, v: None },
        Bar { t: 120_000, o: 109.0, h: 115.0, l: 107.0, c: 114.0, v: None },
    ];
    // k=0.5, 이동평균 필터 없음.
    let rule = Breakout::new(0.5, ExitMode::NextOpen, 0.0, None, None, 1.0);
    let lines = rule.chart_series(&rows);
    assert_eq!(lines.len(), 1, "필터가 없으면 돌파선 하나뿐");
    assert_eq!(lines[0].id, "breakout");
    // 첫 봉은 직전 봉이 없어 비움.
    assert_eq!(lines[0].values[0], None);
    // 2번째: 시가 106 + 0.5 x (110-90) = 116
    assert_eq!(lines[0].values[1], Some(116.0));
    // 3번째: 시가 109 + 0.5 x (112-104) = 113
    assert_eq!(lines[0].values[2], Some(113.0));
}
