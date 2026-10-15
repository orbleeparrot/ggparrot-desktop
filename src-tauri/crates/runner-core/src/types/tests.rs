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

/// 하나로 합치기 전의 세 갈래(기초 자산·바이낸스 견적 통화·바이낸스 체결 수수료용 기초 자산) — 지원하는 심볼에서
/// 결과가 같은지 비교하는 기준.
mod before_split {
    pub fn base_asset(symbol: &str) -> String {
        let s = symbol.trim().to_uppercase();
        if let Some(rest) = s.strip_prefix("KRW-") {
            return rest.to_string();
        }
        for q in ["USDT", "BUSD", "USDC", "FDUSD"] {
            if let Some(b) = s.strip_suffix(q) {
                if !b.is_empty() {
                    return b.to_string();
                }
            }
        }
        s
    }
    pub fn binance_quote(symbol: &str) -> &'static str {
        ["FDUSD", "USDT", "USDC", "BUSD", "BTC", "ETH"]
            .into_iter()
            .find(|q| symbol.len() > q.len() && symbol.to_uppercase().ends_with(q))
            .unwrap_or("")
    }
    pub fn fill_base(symbol: &str) -> String {
        if symbol.ends_with("USDT") || symbol.ends_with("USDC") {
            symbol[..symbol.len() - 4].to_string()
        } else {
            String::new()
        }
    }
}

#[test]
fn supported_symbols_split_as_before() {
    for s in ["BTCUSDT", "ETHUSDC", "1000PEPEUSDT", "USDCUSDT", "SOLUSDC", "BTCUSDT "] {
        let (base, quote) = split_symbol(s);
        assert_eq!(base, before_split::base_asset(s), "{s}");
        assert_eq!(quote, before_split::binance_quote(s.trim()), "{s}");
        assert_eq!(base, before_split::fill_base(s.trim()), "{s}");
        assert_eq!(base_asset(s), before_split::base_asset(s), "{s}");
    }
    for s in ["KRW-BTC", "krw-eth", " KRW-XRP ", "KRW-USDT"] {
        assert_eq!(split_symbol(s).0, before_split::base_asset(s), "{s}");
        assert_eq!(split_symbol(s).1, "KRW", "{s}");
    }
}

#[test]
fn other_quotes_split_by_the_same_rule() {
    // USDT·USDC 밖의 바이낸스 견적도 끝에서 찾음 — 예전 기초 자산 읽기는 심볼 전체를 돌려줬음.
    assert_eq!(split_symbol("ETHBTC"), ("ETH".to_string(), "BTC"));
    assert_eq!(split_symbol("BNBETH"), ("BNB".to_string(), "ETH"));
    assert_eq!(split_symbol("BTCFDUSD"), ("BTC".to_string(), "FDUSD"));
    assert_eq!(split_symbol("BTCBUSD"), ("BTC".to_string(), "BUSD"));
    // 견적 통화를 모르거나 기초 자산이 비면 심볼 전체와 빈 견적.
    assert_eq!(split_symbol("BTCKRW"), ("BTCKRW".to_string(), ""));
    assert_eq!(split_symbol("USDT"), ("USDT".to_string(), ""));
    assert_eq!(split_symbol(""), (String::new(), ""));
}
