//! 실행기가 거래소에서 **직접** 받는 마감봉.
//!
//! 지금은 서버가 봉을 받아 판단하고 명령만 내려보낸다. 그 구조에서는 우리 서버가 잠깐
//! 멈추면 사용자는 자기 PC 가 멀쩡한데도 매수 · 매도 시점을 놓친다 — 원인이 전적으로 우리
//! 쪽에 있고, 전 사용자에게 동시에, 사용자에게는 보이지도 않는다. 그래서 봉부터 가져온다.
//!
//! 여기서 지키는 것 셋:
//!
//! * **마감된 봉만 준다.** 진행 중 봉으로 판단하면 같은 봉에서 샀다 팔았다 한다. 거래소마다
//!   '진행 중' 표시 방식이 달라 시각으로 가른다 — 봉이 열린 시각 + 간격 ≤ 지금.
//! * **봉 격자는 [`crate::intervals`] 가 정한다.** 빗썸 봉은 KST 경계에서 열리므로 직접
//!   계산하면 일봉이 9시간 어긋난다. 서버와 다른 경계를 쓰면 같은 매크로가 다른 봉을 본다.
//! * **같은 봉을 두 번 주지 않는다.** 지표는 받은 봉을 누적하므로 중복은 조용히 결과를 바꾼다.
//!
//! 거래소 **공개 시세**만 쓴다 — 키도 서명도 없다(주문은 [`crate::brokers`] 가 한다).

use serde_json::Value;
use std::time::Duration;

use crate::intervals::{grid_offset, is_grid_aligned, step_ms};

/// 국내 거래소는 한 번에 200개까지 준다. 그보다 많이 필요하면 커서로 거슬러 올라간다.
const KRW_PAGE: usize = 200;
/// 커서를 거슬러 올라가는 횟수 상한 — 쪽이 안 밀리는 거래소 응답에서 무한 반복을 막는다.
const MAX_PAGES: usize = 10;
/// 업비트 · 빗썸이 지원하는 분봉 단위.
const KRW_MINUTES: &[i64] = &[1, 3, 5, 10, 15, 30, 60, 240];
const TIMEOUT: Duration = Duration::from_secs(10);
const KST_SHIFT_MS: i64 = 9 * 3_600_000;

/// 봉 하나. `t` 는 봉이 **열린** 시각(ms, UTC). `v`(거래량)는 모르면 None —
/// 0.0 과 섞지 않는다(필터가 0 을 "거래 없음" 으로 읽어 막는다).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Candle {
    pub t: i64,
    pub o: f64,
    pub h: f64,
    pub l: f64,
    pub c: f64,
    pub v: Option<f64>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum CandleError {
    /// 네트워크 · 거래소 응답 문제. 다음 바퀴에 다시 받으면 된다.
    Fetch(String),
    /// 설정이 잘못됐다(모르는 거래소 · 간격 · 시장). 다시 시도해도 같다.
    Unsupported(String),
}

impl CandleError {
    pub fn message(&self) -> &str {
        match self {
            CandleError::Fetch(m) | CandleError::Unsupported(m) => m,
        }
    }

    /// 다시 시도할 만한가. `false` 면 세션을 열어 두는 것이 의미 없다.
    pub fn retryable(&self) -> bool {
        matches!(self, CandleError::Fetch(_))
    }
}

impl std::fmt::Display for CandleError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.message())
    }
}

fn num(value: Option<&Value>) -> Option<f64> {
    match value? {
        Value::Number(n) => n.as_f64(),
        Value::String(s) => s.parse().ok(),
        _ => None,
    }
}

/// 국내 거래소 봉 주소. 지원하지 않는 간격이면 `Unsupported`.
fn krw_path(interval: &str) -> Result<String, CandleError> {
    if interval == "1d" {
        return Ok("/v1/candles/days".into());
    }
    let step = step_ms(interval)
        .ok_or_else(|| CandleError::Unsupported(format!("지원하지 않는 봉 간격이에요: {interval}")))?;
    let minutes = step / 60_000;
    if !KRW_MINUTES.contains(&minutes) {
        return Err(CandleError::Unsupported(format!(
            "국내 거래소가 지원하지 않는 봉 간격이에요: {interval}"
        )));
    }
    Ok(format!("/v1/candles/minutes/{minutes}"))
}

/// `to` 커서. 업비트는 UTC ISO(오프셋 포함), 빗썸은 KST 를 오프셋 없이 쓴다(문서 그대로).
/// 빗썸에 오프셋을 붙이면 9시간 어긋난 쪽을 받는다.
fn krw_cursor(stamp_ms: i64, exchange: &str) -> String {
    let bithumb = exchange.eq_ignore_ascii_case("bithumb");
    let shifted = if bithumb { stamp_ms + KST_SHIFT_MS } else { stamp_ms };
    let secs = shifted.div_euclid(1000);
    let millis = shifted.rem_euclid(1000);
    let naive = chrono::DateTime::from_timestamp(secs, (millis as u32) * 1_000_000)
        .unwrap_or_else(|| chrono::DateTime::from_timestamp(0, 0).expect("epoch"));
    let text = naive.format("%Y-%m-%dT%H:%M:%S%.3f").to_string();
    if bithumb {
        text
    } else {
        format!("{text}+00:00")
    }
}

/// 국내 거래소 응답 한 줄 → [`Candle`].
fn krw_candle(raw: &Value, interval: &str, exchange: &str) -> Result<Candle, CandleError> {
    // `timestamp` 는 마지막 체결 시각이지 봉의 시작이 아니다 — 문서화된 UTC 칸만 쓴다.
    let (text, is_kst) = match raw.get("candle_date_time_utc").and_then(Value::as_str) {
        Some(s) if !s.is_empty() => (s, false),
        _ => match raw.get("candle_date_time_kst").and_then(Value::as_str) {
            Some(s) if !s.is_empty() => (s, true),
            _ => return Err(CandleError::Fetch("거래소 봉에 시각이 없어요.".into())),
        },
    };
    let naive = chrono::NaiveDateTime::parse_from_str(text.trim_end_matches('Z'), "%Y-%m-%dT%H:%M:%S")
        .map_err(|_| CandleError::Fetch("거래소 봉 시각을 읽지 못했어요.".into()))?;
    let mut t = naive.and_utc().timestamp_millis();
    if is_kst {
        t -= KST_SHIFT_MS;
    }
    if !is_grid_aligned(t, interval, exchange) {
        // 격자에 안 맞으면 간격이나 거래소를 잘못 안 것이다 — 그대로 쓰면 지표가 어긋난다.
        return Err(CandleError::Fetch("거래소 봉 시작 시각이 봉 간격과 맞지 않아요.".into()));
    }
    let pick = |key: &str| {
        num(raw.get(key)).ok_or_else(|| CandleError::Fetch("거래소 봉에 값이 모자라요.".into()))
    };
    Ok(Candle {
        t,
        o: pick("opening_price")?,
        h: pick("high_price")?,
        l: pick("low_price")?,
        c: pick("trade_price")?,
        v: num(raw.get("candle_acc_trade_volume")),
    })
}

/// 바이낸스 kline 한 줄 → [`Candle`]. `[t, o, h, l, c, v, ...]`.
fn binance_candle(row: &Value) -> Result<Candle, CandleError> {
    let short = || CandleError::Fetch("거래소 봉에 값이 모자라요.".into());
    let arr = row.as_array().ok_or_else(short)?;
    let at = |i: usize| num(arr.get(i)).ok_or_else(short);
    Ok(Candle {
        t: at(0)? as i64,
        o: at(1)?,
        h: at(2)?,
        l: at(3)?,
        c: at(4)?,
        v: num(arr.get(5)),
    })
}

/// `(거래소 · 종목 · 간격 · 시장)` 하나의 마감봉을 가져온다.
#[derive(Debug, Clone)]
pub struct CandleSource {
    pub exchange: String,
    pub symbol: String,
    pub interval: String,
    pub market: String,
    pub step_ms: i64,
    base: String,
    path: String,
    http: reqwest::Client,
}

impl CandleSource {
    pub fn new(
        exchange: &str,
        symbol: &str,
        interval: &str,
        market: &str,
        http: reqwest::Client,
        // 시험이 가짜 거래소를 가리키게 하는 자리 — 운영에서는 늘 None(거래소 공개 주소).
        base_override: Option<String>,
    ) -> Result<Self, CandleError> {
        let exchange = exchange.trim().to_ascii_lowercase();
        let step = step_ms(interval)
            .ok_or_else(|| CandleError::Unsupported(format!("지원하지 않는 봉 간격이에요: {interval}")))?;
        let domestic = matches!(exchange.as_str(), "upbit" | "bithumb");
        if !domestic && exchange != "binance" {
            return Err(CandleError::Unsupported(format!("모르는 거래소예요: {exchange}")));
        }
        if domestic && market != "spot" {
            return Err(CandleError::Unsupported("국내 거래소는 현물 봉만 있어요.".into()));
        }
        let (base, path) = if domestic {
            let base = if exchange == "upbit" {
                crate::brokers::domestic::UPBIT_BASE
            } else {
                crate::brokers::domestic::BITHUMB_BASE
            };
            (base.to_string(), krw_path(interval)?)
        } else if market == "futures" {
            (crate::brokers::binance::FUTURES_BASE.to_string(), "/fapi/v1/klines".to_string())
        } else {
            (crate::brokers::binance::SPOT_BASE.to_string(), "/api/v3/klines".to_string())
        };
        let base = base_override.unwrap_or(base);
        Ok(Self {
            exchange,
            symbol: symbol.trim().to_ascii_uppercase(),
            interval: interval.to_string(),
            market: market.to_string(),
            step_ms: step,
            base,
            path,
            http,
        })
    }

    fn domestic(&self) -> bool {
        self.exchange != "binance"
    }

    /// 주소를 직접 조립한다 — 이 저장소의 reqwest 는 `default-features = false` 라
    /// `.query()` 가 없다(brokers/domestic.rs 와 같은 방식).
    fn url(&self, params: &[(String, String)]) -> String {
        if params.is_empty() {
            return format!("{}{}", self.base, self.path);
        }
        let mut ser = url::form_urlencoded::Serializer::new(String::new());
        for (k, v) in params {
            ser.append_pair(k, v);
        }
        format!("{}{}?{}", self.base, self.path, ser.finish())
    }

    async fn get(&self, query: &[(String, String)]) -> Result<Value, CandleError> {
        let response = self
            .http
            .get(self.url(query))
            .timeout(TIMEOUT)
            .send()
            .await
            .map_err(|e| CandleError::Fetch(format!("봉을 받지 못했어요: {e}")))?;
        let status = response.status();
        if !status.is_success() {
            return Err(CandleError::Fetch(format!(
                "거래소가 봉 요청을 거절했어요(HTTP {}).",
                status.as_u16()
            )));
        }
        response
            .json()
            .await
            .map_err(|_| CandleError::Fetch("거래소 봉 응답을 읽지 못했어요.".into()))
    }

    async fn fetch_binance(&self, limit: usize) -> Result<Vec<Candle>, CandleError> {
        let want = limit.saturating_add(1).clamp(1, 1000);
        let body = self
            .get(&[
                ("symbol".into(), self.symbol.clone()),
                ("interval".into(), self.interval.clone()),
                ("limit".into(), want.to_string()),
            ])
            .await?;
        let rows = body
            .as_array()
            .ok_or_else(|| CandleError::Fetch("거래소 봉 응답 모양이 예상과 달라요.".into()))?;
        rows.iter().map(binance_candle).collect()
    }

    async fn fetch_krw(&self, limit: usize) -> Result<Vec<Candle>, CandleError> {
        let want = limit.saturating_add(1).max(1);
        let mut rows: std::collections::BTreeMap<i64, Candle> = Default::default();
        let mut cursor: Option<i64> = None;
        for _ in 0..MAX_PAGES {
            let count = KRW_PAGE.min(want.saturating_sub(rows.len()));
            if count == 0 {
                break;
            }
            let mut query = vec![
                ("market".to_string(), self.symbol.clone()),
                ("count".to_string(), count.to_string()),
            ];
            if let Some(at) = cursor {
                query.push(("to".to_string(), krw_cursor(at, &self.exchange)));
            }
            let body = self.get(&query).await?;
            let page = body
                .as_array()
                .ok_or_else(|| CandleError::Fetch("거래소 봉 응답 모양이 예상과 달라요.".into()))?;
            if page.is_empty() {
                break;
            }
            let parsed: Vec<Candle> = page
                .iter()
                .map(|raw| krw_candle(raw, &self.interval, &self.exchange))
                .collect::<Result<_, _>>()?;
            let oldest = parsed.iter().map(|c| c.t).min().unwrap_or(i64::MAX);
            if let Some(at) = cursor {
                if oldest >= at {
                    // 커서가 안 밀리면 같은 쪽을 맴돈다 — 무한 반복 대신 가진 것으로 끝낸다.
                    break;
                }
            }
            let page_len = page.len();
            rows.extend(parsed.into_iter().map(|c| (c.t, c)));
            if rows.len() >= want || page_len < count {
                break;
            }
            cursor = Some(oldest);
        }
        Ok(rows.into_values().collect())
    }

    /// 마감된 봉을 오래된 것부터 `limit` 개까지. 진행 중 봉은 빼고 준다.
    pub async fn recent_closed(&self, limit: usize, now_ms: i64) -> Result<Vec<Candle>, CandleError> {
        let raw =
            if self.domestic() { self.fetch_krw(limit).await? } else { self.fetch_binance(limit).await? };
        let mut closed: Vec<Candle> =
            raw.into_iter().filter(|c| self.is_closed(c.t, now_ms)).collect();
        if closed.is_empty() {
            return Err(CandleError::Fetch("마감된 봉이 아직 없어요.".into()));
        }
        closed.sort_by_key(|c| c.t);
        let keep = limit.max(1);
        if closed.len() > keep {
            closed.drain(..closed.len() - keep);
        }
        Ok(closed)
    }

    /// `open_ms` 에 열린 봉이 **다 닫혔는가**. 거래소의 표시 대신 시각으로 가른다.
    pub fn is_closed(&self, open_ms: i64, now_ms: i64) -> bool {
        open_ms + self.step_ms <= now_ms
    }

    /// 다음 봉이 닫히는 시각 — 폴링을 봉 경계에 맞추는 데 쓴다.
    pub fn next_close_ms(&self, now_ms: i64) -> i64 {
        let offset = grid_offset(&self.exchange, &self.interval);
        ((now_ms - offset).div_euclid(self.step_ms) + 1) * self.step_ms + offset
    }
}

/// [`CandleSource`] 위에서 **새로 마감된 봉만** 한 번씩 흘려보낸다.
///
/// `poll` 을 자주 불러도 된다 — 새로 마감된 게 없으면 빈 목록이다. 같은 봉을 두 번 주지
/// 않는 것이 이 구조체의 전부다(지표는 받은 봉을 누적하므로 중복이 결과를 바꾼다).
pub struct ClosedCandleStream {
    pub source: CandleSource,
    /// 이미 소비한 마지막 봉의 시작 시각. None 이면 첫 폴에서 기준만 잡고 아무것도 주지 않는다.
    pub last_t: Option<i64>,
}

impl ClosedCandleStream {
    pub fn new(source: CandleSource, since_t: Option<i64>) -> Self {
        Self { source, last_t: since_t }
    }

    /// 지표를 세울 과거 봉. 받은 마지막 봉까지 소비한 것으로 친다.
    pub async fn warmup(&mut self, count: usize, now_ms: i64) -> Result<Vec<Candle>, CandleError> {
        let candles = self.source.recent_closed(count, now_ms).await?;
        self.last_t = candles.last().map(|c| c.t);
        Ok(candles)
    }

    /// `last_t` 뒤에 새로 마감된 봉들(오래된 것부터).
    ///
    /// `lookback` 은 한 번에 되짚는 봉 수다 — 폴을 몇 번 건너뛰어도(절전 · 네트워크 끊김)
    /// 그 사이 봉을 주워 온다. 기준이 없으면(첫 폴) 기준만 잡고 빈 목록을 준다:
    /// 시작하자마자 묵은 봉의 신호로 사지 않는다.
    pub async fn poll(&mut self, lookback: usize, now_ms: i64) -> Result<Vec<Candle>, CandleError> {
        let candles = self.source.recent_closed(lookback.max(1), now_ms).await?;
        let Some(last) = self.last_t else {
            self.last_t = candles.last().map(|c| c.t);
            return Ok(Vec::new());
        };
        let fresh: Vec<Candle> = candles.into_iter().filter(|c| c.t > last).collect();
        if let Some(newest) = fresh.last() {
            self.last_t = Some(newest.t);
        }
        Ok(fresh)
    }
}

#[cfg(test)]
mod tests;
