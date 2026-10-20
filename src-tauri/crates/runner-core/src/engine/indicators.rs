//! 지표 — 종가를 하나씩 먹으며 값을 만든다.
//!
//! 충분한 봉을 못 먹었으면 `None` 을 준다. 그 동안 규칙은 판단하지 않는다 —
//! 반쪽 지표로 판단하면 처음 몇 봉에서만 다른 답이 나오고, 그게 제일 찾기 어렵다.
//!
//! 서버(파이썬 `engine/candles.py` 의 `RSIState` · `BollingerState` · `MAState`)와
//! **식까지 같게** 두었다. 합을 매번 다시 더하는 것도 그대로다 — 누적합으로 바꾸면
//! 부동소수점 오차가 쌓여 수백 봉 뒤에 값이 갈린다.

use std::collections::VecDeque;

/// 고정 길이 창. 서버의 `deque(maxlen=period)` 와 같다.
#[derive(Debug, Clone)]
struct Window {
    period: usize,
    values: VecDeque<f64>,
}

impl Window {
    fn new(period: usize) -> Self {
        Self { period: period.max(1), values: VecDeque::new() }
    }

    fn push(&mut self, value: f64) {
        if self.values.len() == self.period {
            self.values.pop_front();
        }
        self.values.push_back(value);
    }

    fn full(&self) -> bool {
        self.values.len() >= self.period
    }

    /// 창 안의 합. **매번 다시 더한다** — 서버와 같은 순서로 더해야 같은 값이 나온다.
    fn sum(&self) -> f64 {
        self.values.iter().sum()
    }

    fn mean(&self) -> f64 {
        self.sum() / self.period as f64
    }
}

/// Wilder 의 RSI. 과매도 · 과매수를 재는 0~100 값.
#[derive(Debug, Clone)]
pub struct Rsi {
    period: usize,
    prev: Option<f64>,
    avg_gain: f64,
    avg_loss: f64,
    count: usize,
}

impl Rsi {
    pub fn new(period: usize) -> Self {
        Self { period: period.max(1), prev: None, avg_gain: 0.0, avg_loss: 0.0, count: 0 }
    }

    pub fn update(&mut self, close: f64) -> Option<f64> {
        let prev = match self.prev {
            // 첫 종가는 '변화' 가 없다 — 기준만 잡는다.
            None => {
                self.prev = Some(close);
                return None;
            }
            Some(p) => p,
        };
        let change = close - prev;
        self.prev = Some(close);
        let gain = change.max(0.0);
        let loss = (-change).max(0.0);
        let period = self.period as f64;
        self.count += 1;
        if self.count <= self.period {
            // 첫 `period` 개는 단순 평균으로 채운다.
            self.avg_gain += gain / period;
            self.avg_loss += loss / period;
            if self.count < self.period {
                return None;
            }
        } else {
            // 그 뒤로는 Wilder 평활 — 직전 평균에 새 값을 1/period 만큼 섞는다.
            self.avg_gain = (self.avg_gain * (period - 1.0) + gain) / period;
            self.avg_loss = (self.avg_loss * (period - 1.0) + loss) / period;
        }
        if self.avg_loss == 0.0 {
            // 내린 봉이 하나도 없다 — 나누지 않는다.
            return Some(100.0);
        }
        let rs = self.avg_gain / self.avg_loss;
        Some(100.0 - 100.0 / (1.0 + rs))
    }
}

/// 볼린저 밴드 — 중심선과 위 · 아래 선.
#[derive(Debug, Clone)]
pub struct Bollinger {
    num_std: f64,
    window: Window,
}

/// `(중심, 위, 아래)`.
pub type Bands = (f64, f64, f64);

impl Bollinger {
    pub fn new(period: usize, num_std: f64) -> Self {
        Self { num_std, window: Window::new(period) }
    }

    pub fn update(&mut self, close: f64) -> Option<Bands> {
        self.window.push(close);
        if !self.window.full() {
            return None;
        }
        let mean = self.window.mean();
        // 표본이 아니라 **모집단** 분산(n 으로 나눈다) — 서버와 같다.
        let var = self.window.values.iter().map(|x| (x - mean) * (x - mean)).sum::<f64>()
            / self.window.period as f64;
        let sd = var.sqrt();
        Some((mean, mean + self.num_std * sd, mean - self.num_std * sd))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MaType {
    /// 단순 평균.
    Sma,
    /// 지수 평균 — 최근 값에 더 무게를 준다.
    Ema,
}

/// 이동평균.
#[derive(Debug, Clone)]
pub struct Ma {
    kind: MaType,
    window: Window,
    ema: Option<f64>,
    k: f64,
}

impl Ma {
    pub fn new(kind: MaType, period: usize) -> Self {
        let period = period.max(1);
        Self { kind, window: Window::new(period), ema: None, k: 2.0 / (period as f64 + 1.0) }
    }

    pub fn update(&mut self, close: f64) -> Option<f64> {
        if self.kind == MaType::Ema {
            if self.ema.is_none() {
                self.window.push(close);
                if !self.window.full() {
                    return None;
                }
                // 첫 값은 단순 평균으로 심는다(서버와 같게).
                let seed = self.window.mean();
                self.ema = Some(seed);
                return Some(seed);
            }
            let prev = self.ema.expect("위에서 심었다");
            let next = close * self.k + prev * (1.0 - self.k);
            self.ema = Some(next);
            return Some(next);
        }
        self.window.push(close);
        if !self.window.full() {
            return None;
        }
        Some(self.window.mean())
    }
}

#[cfg(test)]
mod tests;
