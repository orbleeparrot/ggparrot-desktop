use super::*;

#[test]
fn key_slots_follow_mode() {
    assert_eq!(KeySlot::for_run(Exchange::Binance, RunMode::Testnet), KeySlot::BinanceTestnet);
    assert_eq!(KeySlot::for_run(Exchange::Binance, RunMode::Live), KeySlot::Binance);
    assert_eq!(KeySlot::for_run(Exchange::Binance, RunMode::Mock), KeySlot::Binance);
    assert_eq!(KeySlot::for_run(Exchange::Upbit, RunMode::Live), KeySlot::Upbit);
    assert_eq!(KeySlot::parse("binance_testnet"), Some(KeySlot::BinanceTestnet));
    assert_eq!(KeySlot::BinanceTestnet.check_mode(), RunMode::Testnet);
    assert_eq!(KeySlot::Upbit.probe_macro()["symbol"], "KRW-BTC");
    assert_eq!(base_asset("BTCUSDT"), "BTC");
    assert_eq!(base_asset("krw-eth"), "ETH");
}

#[test]
fn mode_parse_defaults_to_mock() {
    assert_eq!(RunMode::parse_or_mock("LIVE"), RunMode::Live);
    assert_eq!(RunMode::parse_or_mock("bogus"), RunMode::Mock);
    assert_eq!(RunMode::parse_or_mock(""), RunMode::Mock);
}

#[test]
fn market_rules() {
    assert_eq!(market_of("binance", Side::Long, 1), Market::Spot);
    assert_eq!(market_of("binance", Side::Short, 1), Market::Futures);
    assert_eq!(market_of("binance", Side::Long, 3), Market::Futures);
    assert_eq!(market_of("upbit", Side::Long, 3), Market::Spot);
}

#[test]
fn quote_and_cap() {
    assert_eq!(quote_of("KRW-BTC"), "KRW");
    assert_eq!(quote_of("BTCUSDT"), "USDT");
    assert_eq!(order_cap("KRW"), 150_000.0);
    assert_eq!(order_cap("USDT"), 100.0);
}

#[test]
fn pair_requires_both() {
    let mut c = Credentials::default();
    c.exchanges.insert("upbit".into(), KeyPair { api_key: "a".into(), api_secret: " ".into() });
    assert!(c.pair(Exchange::Upbit).is_none());
    c.exchanges.insert("upbit".into(), KeyPair { api_key: " a ".into(), api_secret: "b".into() });
    assert_eq!(c.pair(Exchange::Upbit).unwrap().api_key, "a");
}

#[test]
fn payload_has_no_secrets_and_version() {
    let m = json!({"symbol": "btcusdt", "position_side": "short", "leverage": 2});
    let p = session_payload(&m, true, RunMode::Testnet);
    assert_eq!(p["symbol"], "BTCUSDT");
    assert_eq!(p["market"], "futures");
    assert_eq!(p["runner_version"], "18");
    assert_eq!(p["mode"], "testnet");
    assert!(p.get("api_key").is_none());
}

#[test]
fn leverage_parsing() {
    assert_eq!(macro_leverage(&json!({})).unwrap(), 1);
    assert_eq!(macro_leverage(&json!({"leverage": 0})).unwrap(), 1);
    assert_eq!(macro_leverage(&json!({"leverage": "5"})).unwrap(), 5);
    assert!(macro_leverage(&json!({"leverage": "x"})).is_err());
}
