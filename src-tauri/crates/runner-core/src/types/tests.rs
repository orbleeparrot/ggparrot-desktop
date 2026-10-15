use super::*;

#[test]
fn key_slots_follow_mode() {
    let (spot, futures) = (Market::Spot, Market::Futures);
    assert_eq!(KeySlot::for_run(Exchange::Binance, RunMode::Testnet, spot), KeySlot::BinanceTestnet);
    assert_eq!(KeySlot::for_run(Exchange::Binance, RunMode::Testnet, futures), KeySlot::BinanceDemo);
    assert_eq!(KeySlot::for_run(Exchange::Binance, RunMode::Live, futures), KeySlot::Binance);
    assert_eq!(KeySlot::for_run(Exchange::Binance, RunMode::Mock, futures), KeySlot::Binance);
    assert_eq!(KeySlot::for_run(Exchange::Upbit, RunMode::Live, spot), KeySlot::Upbit);
    assert_eq!(KeySlot::parse("binance_testnet"), Some(KeySlot::BinanceTestnet));
    assert_eq!(KeySlot::parse("binance_demo"), Some(KeySlot::BinanceDemo));
    assert_eq!(KeySlot::BinanceTestnet.check_mode(), RunMode::Testnet);
    assert_eq!(KeySlot::BinanceDemo.check_mode(), RunMode::Testnet);
    assert_eq!(KeySlot::Binance.check_mode(), RunMode::Live);
    assert_eq!(KeySlot::Upbit.probe_macro()["symbol"], "KRW-BTC");
    assert_eq!(base_asset("BTCUSDT"), "BTC");
    assert_eq!(base_asset("krw-eth"), "ETH");
}

#[test]
fn every_slot_round_trips_and_practice_slots_fix_the_market() {
    for s in KeySlot::ALL {
        assert_eq!(KeySlot::parse(s.as_str()), Some(s));
        assert_eq!(serde_json::to_value(s).unwrap(), s.as_str());
    }
    assert_eq!(KeySlot::BinanceDemo.label(), "바이낸스 데모");
    assert_eq!(KeySlot::BinanceDemo.as_str(), "binance_demo");
    assert_eq!(KeySlot::BinanceTestnet.fixed_market(), Some(Market::Spot));
    assert_eq!(KeySlot::BinanceDemo.fixed_market(), Some(Market::Futures));
    assert_eq!(KeySlot::Binance.fixed_market(), None, "실거래 칸은 쓰는 실행에 따름");
    assert_eq!(KeySlot::Upbit.fixed_market(), Some(Market::Spot));
}

#[test]
fn binance_env_and_slot_agree_both_ways() {
    for env in crate::connection::BINANCE_ENVS {
        assert_eq!(KeySlot::for_binance_env(env).binance_env(), env);
    }
    assert_eq!(KeySlot::Upbit.binance_env(), "");
    // 연습 실행의 칸은 도우미가 안내하는 사이트의 칸과 같음.
    for market in [Market::Spot, Market::Futures] {
        for mode in [RunMode::Testnet, RunMode::Live] {
            let env = crate::connection::binance_env(mode, market);
            assert_eq!(KeySlot::for_run(Exchange::Binance, mode, market), KeySlot::for_binance_env(env));
        }
    }
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
    assert_eq!(p["runner_version"], "19");
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
