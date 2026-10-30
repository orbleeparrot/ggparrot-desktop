//! 차트에 겹쳐 그릴 **지표선**.
//!
//! 값은 엔진이 판단에 쓰는 바로 그 지표로 만듦 — 돌고 있는 지표의 설정을 그대로 가진
//! 빈 지표에 받은 봉을 처음부터 다시 먹임. 화면이 따로 계산하면 산수가 두 벌이 되고,
//! 그중 하나는 엔진이 보는 것과 다름.
//!
//! 봉이 모자라 아직 값이 없는 앞쪽은 `None`으로 비움 — 0으로 채우면 화면이 그 0을
//! 진짜 지표값으로 그림.

use serde::Serialize;

use crate::engine::candle_sim::Bar;
use crate::engine::indicators::{Bollinger, Ma, Rsi};

/// 봉과 길이가 같은 선 하나.
#[derive(Debug, Clone, Serialize)]
pub struct Series {
    /// 화면이 색·굵기를 고르는 데 쓰는 이름(`ma_fast`, `band_upper` …).
    pub id: &'static str,
    /// 범례에 그대로 쓰는 말.
    pub label: String,
    pub values: Vec<Option<f64>>,
}

/// 아래 보조창에 따로 그리는 오실레이터(RSI) — 가격축에 섞으면 축이 망가짐.
#[derive(Debug, Clone, Serialize)]
pub struct Oscillator {
    pub label: String,
    pub values: Vec<Option<f64>>,
    /// 사는 문턱·파는 문턱. 없으면 선을 긋지 않음.
    pub entry: Option<f64>,
    pub exit: Option<f64>,
}

/// 이동평균선 하나.
pub fn ma_series(ma: &Ma, bars: &[Bar], id: &'static str, label: String) -> Series {
    let mut fresh = ma.fresh();
    let values = bars.iter().map(|bar| fresh.update(bar.c)).collect();
    Series { id, label, values }
}

/// 볼린저 밴드 세 줄(위·중심·아래). 한 번만 돌려 세 줄을 함께 만듦.
pub fn bollinger_series(bands: &Bollinger, bars: &[Bar]) -> Vec<Series> {
    let mut fresh = bands.fresh();
    let mut mid = Vec::with_capacity(bars.len());
    let mut upper = Vec::with_capacity(bars.len());
    let mut lower = Vec::with_capacity(bars.len());
    for bar in bars {
        match fresh.update(bar.c) {
            Some((m, u, l)) => {
                mid.push(Some(m));
                upper.push(Some(u));
                lower.push(Some(l));
            }
            None => {
                mid.push(None);
                upper.push(None);
                lower.push(None);
            }
        }
    }
    let period = bands.period();
    vec![
        Series { id: "band_upper", label: format!("위 밴드 {period}"), values: upper },
        Series { id: "band_mid", label: format!("중심선 {period}"), values: mid },
        Series { id: "band_lower", label: format!("아래 밴드 {period}"), values: lower },
    ]
}

/// RSI 보조창.
pub fn rsi_oscillator(rsi: &Rsi, bars: &[Bar], entry: Option<f64>, exit: Option<f64>) -> Oscillator {
    let mut fresh = rsi.fresh();
    let values = bars.iter().map(|bar| fresh.update(bar.c)).collect();
    Oscillator { label: format!("RSI {}", rsi.period()), values, entry, exit }
}

#[cfg(test)]
mod tests;
