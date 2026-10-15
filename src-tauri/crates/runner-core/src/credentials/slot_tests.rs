use super::*;

#[test]
fn testnet_slot_is_kept_separately() {
    let store = MemoryStore::new();
    let mut values = Credentials { member_key: "m".into(), ..Default::default() };
    values.exchanges.insert("binance".into(), KeyPair { api_key: "live-k".into(), api_secret: "live-s".into() });
    values
        .exchanges
        .insert("binance_testnet".into(), KeyPair { api_key: "test-k".into(), api_secret: "test-s".into() });
    values.exchanges.insert("binance_demo".into(), KeyPair { api_key: "demo-k".into(), api_secret: "demo-s".into() });
    apply_exchange_choice(&store, KeySlot::BinanceTestnet, true, &values).unwrap();
    apply_exchange_choice(&store, KeySlot::BinanceDemo, true, &values).unwrap();
    apply_exchange_choice(&store, KeySlot::Binance, true, &values).unwrap();
    let loaded = load_result(&store).values.unwrap();
    assert_eq!(loaded.exchanges["binance_testnet"].api_key, "test-k");
    assert_eq!(loaded.exchanges["binance_demo"].api_key, "demo-k");
    assert_eq!(loaded.exchanges["binance"].api_key, "live-k");
    // 테스트넷 칸만 지워도 실거래 칸·회원 키는 남음
    let saved = apply_exchange_choice(&store, KeySlot::BinanceTestnet, false, &values).unwrap().unwrap();
    assert!(!saved.exchanges.contains_key("binance_testnet"));
    assert_eq!(saved.exchanges["binance"].api_key, "live-k");
    assert_eq!(saved.exchanges["binance_demo"].api_key, "demo-k");
    assert_eq!(saved.member_key, "m");
}

#[test]
fn member_key_updates_only_existing_store() {
    let store = MemoryStore::new();
    assert!(update_member_key(&store, "new").unwrap().is_none());
    assert_eq!(load_result(&store).status, LoadStatus::Missing);
    let mut values = Credentials::default();
    values.exchanges.insert("upbit".into(), KeyPair { api_key: "a".repeat(20), api_secret: "b".repeat(20) });
    apply_exchange_choice(&store, KeySlot::Upbit, true, &values).unwrap();
    assert_eq!(update_member_key(&store, "new").unwrap().unwrap().member_key, "new");
    assert_eq!(load_result(&store).values.unwrap().member_key, "new");
}
