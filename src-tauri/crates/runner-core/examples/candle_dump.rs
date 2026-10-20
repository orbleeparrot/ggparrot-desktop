//! 실제 거래소에서 받은 마감봉을 찍어 파이썬 서버와 대조하는 도구.
//! `cargo run -p runner-core --example candle_dump`
#[tokio::main]
async fn main() {
    let http = runner_core::net::http_client();
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis() as i64;
    for (exchange, symbol, interval) in
        [("bithumb", "KRW-BTC", "1h"), ("bithumb", "KRW-BTC", "1d"), ("upbit", "KRW-BTC", "1d"), ("binance", "BTCUSDT", "1h")]
    {
        let source =
            runner_core::candles::CandleSource::new(exchange, symbol, interval, "spot", http.clone(), None)
                .unwrap();
        match source.recent_closed(5, now).await {
            Ok(rows) => {
                for c in rows {
                    println!("{exchange},{interval},{},{},{},{},{}", c.t, c.o, c.h, c.l, c.c);
                }
            }
            Err(e) => eprintln!("{exchange},{interval},FAILED,{}", e.message()),
        }
    }
}
