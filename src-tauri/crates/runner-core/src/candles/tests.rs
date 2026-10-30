//! 실행기가 거래소에서 직접 받는 마감봉.
//!
//! 봉을 잘못 가져오면 조용히 다른 자리에서 사고팖 — 서버와 다른 경계, 진행 중인 봉,
//! 같은 봉 두 번. 셋 다 화면에는 아무 표시가 없음. 그래서 여기서 못 박음.

use super::*;
use serde_json::json;
use wiremock::matchers::{method, path, query_param};
use wiremock::{Mock, MockServer, Request, ResponseTemplate};

const HOUR: i64 = 3_600_000;
const DAY: i64 = 24 * HOUR;

fn source(server: &MockServer, exchange: &str, interval: &str) -> CandleSource {
    let symbol = if exchange == "binance" { "BTCUSDT" } else { "KRW-BTC" };
    CandleSource::new(exchange, symbol, interval, "spot", reqwest::Client::new(), Some(server.uri())).expect("source")
}

/// 거래소가 주는 모양 그대로 — 최신이 먼저.
fn krw_rows(start_ms: i64, count: i64, step: i64) -> Vec<serde_json::Value> {
    (0..count)
        .rev()
        .map(|i| {
            let t = start_ms + i * step;
            let stamp = chrono::DateTime::from_timestamp_millis(t).unwrap();
            json!({
                "candle_date_time_utc": stamp.format("%Y-%m-%dT%H:%M:%S").to_string(),
                "opening_price": 100.0 + i as f64,
                "high_price": 102.0 + i as f64,
                "low_price": 99.0 + i as f64,
                "trade_price": 101.0 + i as f64,
                "candle_acc_trade_volume": 10.0 + i as f64,
            })
        })
        .collect()
}

fn binance_rows(start_ms: i64, count: i64, step: i64) -> Vec<serde_json::Value> {
    (0..count)
        .map(|i| {
            let t = start_ms + i * step;
            json!([
                t,
                format!("{}", 100.0 + i as f64),
                format!("{}", 102.0 + i as f64),
                format!("{}", 99.0 + i as f64),
                format!("{}", 101.0 + i as f64),
                format!("{}", 10.0 + i as f64)
            ])
        })
        .collect()
}

async fn mount(server: &MockServer, url_path: &str, body: serde_json::Value) {
    Mock::given(method("GET"))
        .and(path(url_path))
        .respond_with(ResponseTemplate::new(200).set_body_json(body))
        .mount(server)
        .await;
}

// --- 마감 판정 -------------------------------------------------------------
#[tokio::test]
async fn 진행_중인_봉은_주지_않는다() {
    // 진행 중 봉으로 판단하면 같은 봉에서 샀다 팔았다 함.
    let server = MockServer::start().await;
    mount(&server, "/api/v3/klines", json!(binance_rows(7 * HOUR, 4, HOUR))).await;
    let got = source(&server, "binance", "1h").recent_closed(10, 10 * HOUR).await.unwrap();
    assert_eq!(got.iter().map(|c| c.t).collect::<Vec<_>>(), vec![7 * HOUR, 8 * HOUR, 9 * HOUR]);
}

#[tokio::test]
async fn 딱_마감된_봉은_포함한다() {
    let server = MockServer::start().await;
    mount(&server, "/api/v3/klines", json!(binance_rows(9 * HOUR, 1, HOUR))).await;
    let got = source(&server, "binance", "1h").recent_closed(5, 10 * HOUR).await.unwrap();
    assert_eq!(got.len(), 1);
}

#[tokio::test]
async fn 마감봉이_하나도_없으면_알린다() {
    let server = MockServer::start().await;
    mount(&server, "/api/v3/klines", json!(binance_rows(10 * HOUR, 1, HOUR))).await;
    let err = source(&server, "binance", "1h").recent_closed(5, 10 * HOUR + 60_000).await.unwrap_err();
    assert!(err.retryable(), "일시적인 상황이다 — 세션을 끝낼 이유가 아니다");
}

#[tokio::test]
async fn 요청한_개수만큼만_준다() {
    let server = MockServer::start().await;
    mount(&server, "/api/v3/klines", json!(binance_rows(50 * HOUR, 40, HOUR))).await;
    let got = source(&server, "binance", "1h").recent_closed(5, 100 * HOUR).await.unwrap();
    assert_eq!(got.len(), 5);
    assert_eq!(got[0].t, 85 * HOUR);
}

#[tokio::test]
async fn 바이낸스_문자열_값을_숫자로_읽는다() {
    let server = MockServer::start().await;
    mount(&server, "/api/v3/klines", json!(binance_rows(7 * HOUR, 2, HOUR))).await;
    let got = source(&server, "binance", "1h").recent_closed(1, 10 * HOUR).await.unwrap();
    let last = got[0];
    assert_eq!((last.o, last.h, last.l, last.c), (101.0, 103.0, 100.0, 102.0));
    assert_eq!(last.v, Some(11.0));
}

// --- 국내 거래소 -----------------------------------------------------------
#[tokio::test]
async fn 업비트_응답을_엔진_모양으로_읽는다() {
    let server = MockServer::start().await;
    mount(&server, "/v1/candles/minutes/60", json!(krw_rows(7 * HOUR, 3, HOUR))).await;
    let got = source(&server, "upbit", "1h").recent_closed(10, 10 * HOUR).await.unwrap();
    assert_eq!(got.iter().map(|c| c.t).collect::<Vec<_>>(), vec![7 * HOUR, 8 * HOUR, 9 * HOUR]);
    let first = got[0];
    assert_eq!((first.o, first.h, first.l, first.c, first.v), (100.0, 102.0, 99.0, 101.0, Some(10.0)));
}

#[tokio::test]
async fn 일봉은_days_주소를_쓴다() {
    let server = MockServer::start().await;
    mount(&server, "/v1/candles/days", json!(krw_rows(7 * DAY, 3, DAY))).await;
    assert_eq!(source(&server, "upbit", "1d").recent_closed(3, 10 * DAY).await.unwrap().len(), 3);
}

#[tokio::test]
async fn 이백_개를_넘으면_커서로_거슬러_올라간다() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/v1/candles/minutes/60"))
        .and(query_param("count", "200"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!(krw_rows(150 * HOUR, 200, HOUR))))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/v1/candles/minutes/60"))
        .and(query_param("count", "41"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!(krw_rows(109 * HOUR, 41, HOUR))))
        .mount(&server)
        .await;
    let got = source(&server, "upbit", "1h").recent_closed(240, 400 * HOUR).await.unwrap();
    assert_eq!(got.len(), 240);
}

#[tokio::test]
async fn 커서가_안_밀리면_멈춘다() {
    // 같은 쪽을 계속 받으면 무한 반복이 됨 — 가진 것으로 끝냄.
    let server = MockServer::start().await;
    mount(&server, "/v1/candles/minutes/60", json!(krw_rows(150 * HOUR, 200, HOUR))).await;
    let got = source(&server, "upbit", "1h").recent_closed(240, 400 * HOUR).await.unwrap();
    assert_eq!(got.len(), 200);
}

#[tokio::test]
async fn 빗썸은_한국시간_커서를_오프셋_없이_보낸다() {
    // 빗썸 문서가 그러함 — 오프셋을 붙이면 9시간 어긋난 쪽을 받음.
    let server = MockServer::start().await;
    mount(&server, "/v1/candles/minutes/60", json!(krw_rows(150 * HOUR, 200, HOUR))).await;
    let _ = source(&server, "bithumb", "1h").recent_closed(240, 400 * HOUR).await;
    let cursors: Vec<String> = server
        .received_requests()
        .await
        .unwrap()
        .iter()
        .filter_map(|r: &Request| r.url.query_pairs().find(|(k, _)| k == "to").map(|(_, v)| v.into_owned()))
        .collect();
    assert!(!cursors.is_empty(), "커서를 한 번은 보내야 한다");
    for cursor in &cursors {
        assert!(!cursor.contains('+') && !cursor.ends_with('Z'), "빗썸 커서에 오프셋이 붙었다: {cursor}");
    }
}

#[tokio::test]
async fn 업비트는_협정시_오프셋을_붙여_보낸다() {
    let server = MockServer::start().await;
    mount(&server, "/v1/candles/minutes/60", json!(krw_rows(150 * HOUR, 200, HOUR))).await;
    let _ = source(&server, "upbit", "1h").recent_closed(240, 400 * HOUR).await;
    let cursor = server
        .received_requests()
        .await
        .unwrap()
        .iter()
        .find_map(|r: &Request| r.url.query_pairs().find(|(k, _)| k == "to").map(|(_, v)| v.into_owned()))
        .expect("커서");
    assert!(cursor.ends_with("+00:00"), "업비트 커서: {cursor}");
}

#[tokio::test]
async fn 격자에_안_맞는_봉은_거부한다() {
    let server = MockServer::start().await;
    let mut rows = krw_rows(7 * HOUR, 2, HOUR);
    rows[0]["candle_date_time_utc"] = json!("2026-10-20T07:17:00"); // 정시가 아님
    mount(&server, "/v1/candles/minutes/60", json!(rows)).await;
    let err = source(&server, "upbit", "1h").recent_closed(5, 10 * HOUR).await.unwrap_err();
    assert!(err.message().contains("봉 간격"), "{}", err.message());
}

// --- 설정 오류 ---------------------------------------------------------------
#[test]
fn 모르는_거래소와_간격은_만들_때_막는다() {
    let http = reqwest::Client::new();
    for (exchange, symbol, interval) in [("coinbase", "BTC-USD", "1h"), ("binance", "BTCUSDT", "2h")] {
        let err = CandleSource::new(exchange, symbol, interval, "spot", http.clone(), None).unwrap_err();
        assert!(!err.retryable(), "다시 시도해도 같다: {}", err.message());
    }
}

#[test]
fn 국내는_선물_봉이_없다() {
    let err = CandleSource::new("upbit", "KRW-BTC", "1h", "futures", reqwest::Client::new(), None).unwrap_err();
    assert!(!err.retryable());
}

#[test]
fn 매크로가_쓰는_모든_간격이_국내에서_된다() {
    // 지금 표의 간격은 전부 국내에 있음. 새 간격을 더하면 여기서 먼저 걸림.
    for (interval, _) in crate::intervals::INTERVALS {
        assert!(krw_path(interval).is_ok(), "{interval}");
    }
    assert!(krw_path("2h").is_err());
}

#[tokio::test]
async fn 거래소가_거절하면_알린다() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/v3/klines"))
        .respond_with(ResponseTemplate::new(429))
        .mount(&server)
        .await;
    let err = source(&server, "binance", "1h").recent_closed(5, 10 * HOUR).await.unwrap_err();
    assert!(err.message().contains("429") && err.retryable());
}

// --- 중복 없이 흘려보내기 ---------------------------------------------------
#[tokio::test]
async fn 첫_폴은_기준만_잡고_아무것도_주지_않는다() {
    // 시작하자마자 묵은 봉의 신호로 사지 않음.
    let server = MockServer::start().await;
    mount(&server, "/api/v3/klines", json!(binance_rows(7 * HOUR, 4, HOUR))).await;
    let mut stream = ClosedCandleStream::new(source(&server, "binance", "1h"), None);
    assert!(stream.poll(3, 10 * HOUR).await.unwrap().is_empty());
    assert_eq!(stream.last_t, Some(9 * HOUR));
}

#[tokio::test]
async fn 새로_마감된_봉만_한_번씩_준다() {
    let server = MockServer::start().await;
    mount(&server, "/api/v3/klines", json!(binance_rows(7 * HOUR, 5, HOUR))).await;
    let mut stream = ClosedCandleStream::new(source(&server, "binance", "1h"), None);
    stream.poll(3, 10 * HOUR).await.unwrap(); // 기준 = 9시
    let fresh = stream.poll(3, 11 * HOUR).await.unwrap();
    assert_eq!(fresh.iter().map(|c| c.t).collect::<Vec<_>>(), vec![10 * HOUR]);
    // 지표는 받은 봉을 누적함 — 중복은 조용히 결과를 바꿈.
    assert!(stream.poll(3, 11 * HOUR).await.unwrap().is_empty());
}

#[tokio::test]
async fn 폴을_건너뛰어도_그_사이_봉을_주워_온다() {
    // 절전·네트워크 끊김으로 몇 봉을 놓쳐도 순서대로 돌려줌.
    let server = MockServer::start().await;
    mount(&server, "/api/v3/klines", json!(binance_rows(7 * HOUR, 7, HOUR))).await;
    let mut stream = ClosedCandleStream::new(source(&server, "binance", "1h"), Some(9 * HOUR));
    let fresh = stream.poll(5, 13 * HOUR).await.unwrap();
    assert_eq!(fresh.iter().map(|c| c.t).collect::<Vec<_>>(), vec![10 * HOUR, 11 * HOUR, 12 * HOUR]);
}

#[tokio::test]
async fn 웜업_뒤_첫_폴은_새_봉부터_준다() {
    // 웜업에 들어간 봉을 다시 주면 지표가 이중으로 누적됨.
    let server = MockServer::start().await;
    mount(&server, "/api/v3/klines", json!(binance_rows(0, 11, HOUR))).await;
    let mut stream = ClosedCandleStream::new(source(&server, "binance", "1h"), None);
    let warm = stream.warmup(8, 10 * HOUR).await.unwrap();
    assert_eq!(warm.len(), 8);
    assert_eq!(stream.last_t, Some(9 * HOUR));
    let fresh = stream.poll(5, 11 * HOUR).await.unwrap();
    assert_eq!(fresh.iter().map(|c| c.t).collect::<Vec<_>>(), vec![10 * HOUR]);
}

// --- 봉 경계 ---------------------------------------------------------------
#[test]
fn 다음_마감_시각은_공유_격자를_따른다() {
    let http = reqwest::Client::new();
    let now = 1_792_476_000_000; // 2026-10-20T07:00:00Z
    for (exchange, symbol) in [("binance", "BTCUSDT"), ("upbit", "KRW-BTC"), ("bithumb", "KRW-BTC")] {
        for interval in ["5m", "1h", "4h", "1d"] {
            let s = CandleSource::new(exchange, symbol, interval, "spot", http.clone(), None).unwrap();
            assert_eq!(
                s.next_close_ms(now),
                crate::intervals::next_close_ms(interval, now, exchange).unwrap(),
                "{exchange} {interval}"
            );
        }
    }
}
