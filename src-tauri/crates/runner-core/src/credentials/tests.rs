use super::*;
use serde_json::json;

fn values(api_key: &str, api_secret: &str, member_key: &str) -> Credentials {
    let mut c = Credentials { member_key: member_key.into(), ..Default::default() };
    c.exchanges.insert("binance".into(), KeyPair { api_key: api_key.into(), api_secret: api_secret.into() });
    c
}

fn pair(k: &str, s: &str) -> KeyPair {
    KeyPair { api_key: k.into(), api_secret: s.into() }
}

fn load(store: &dyn SecretStore) -> Option<Credentials> {
    load_result(store).values
}

fn load_raw(raw: &str) -> Option<Credentials> {
    load(&MemoryStore::with(raw))
}

fn load_json(v: Value) -> Option<Credentials> {
    load_raw(&serde_json::to_string(&v).unwrap())
}

fn roundtrip(c: &Credentials) -> Credentials {
    let store = MemoryStore::new();
    save(&store, c).unwrap();
    load(&store).unwrap()
}

fn obs(ip: &str, at: f64) -> IpObservation {
    IpObservation { public_ipv4: ip.into(), observed_at: at }
}

#[test]
fn save_then_load_round_trips_as_version_2() {
    let store = MemoryStore::new();
    save(&store, &values("AK", "SECRET-1", "MK")).unwrap();
    let stored: Value = serde_json::from_str(&store.value().unwrap()).unwrap();
    assert_eq!(
        stored,
        json!({"version": 2, "member_key": "MK", "exchanges": {"binance": {"api_key": "AK", "api_secret": "SECRET-1"}}})
    );
    assert!(store.value().unwrap().starts_with("{\"version\": 2, \"member_key\": \"MK\", \"exchanges\": {"));
    assert_eq!(load(&store).unwrap(), values("AK", "SECRET-1", "MK"));
}

#[test]
fn load_missing_or_corrupt_returns_none() {
    assert!(load(&MemoryStore::new()).is_none());
    assert!(load_raw("garbage").is_none());
}

#[test]
fn clear_removes_and_is_idempotent() {
    let store = MemoryStore::new();
    save(&store, &values("AK", "S", "M")).unwrap();
    clear(&store).unwrap();
    assert!(store.value().is_none());
    clear(&store).unwrap();
}

#[test]
fn only_known_fields_are_kept() {
    let loaded = load_json(json!({"version": 2, "member_key": "M", "junk": "x",
        "exchanges": {"binance": {"api_key": "AK", "api_secret": "S", "junk": "y"}}}))
    .unwrap();
    assert_eq!(loaded, values("AK", "S", "M"));
    let store = MemoryStore::new();
    save(&store, &loaded).unwrap();
    assert!(!store.value().unwrap().contains("junk"));
}

#[test]
fn each_exchange_keeps_its_own_pair() {
    let mut c = Credentials { member_key: "mk".into(), ..Default::default() };
    c.exchanges.insert("binance".into(), pair("bk", "bs"));
    c.exchanges.insert("upbit".into(), pair("uk", "us"));
    c.exchanges.insert("kraken".into(), pair("x", "y"));
    let loaded = roundtrip(&c);
    assert_eq!(loaded.exchanges["binance"].api_key, "bk");
    assert_eq!(loaded.exchanges["upbit"].api_secret, "us");
    assert_eq!(loaded.member_key, "mk");
    assert!(!loaded.exchanges.contains_key("kraken"));
}

#[test]
fn v1_file_moves_into_binance_and_keeps_member_key() {
    let loaded = load_json(json!({"api_key": "old", "api_secret": "olds", "member_key": "mk"})).unwrap();
    assert_eq!(loaded, values("old", "olds", "mk"));
    // rewritten as v2 on next save
    let store = MemoryStore::new();
    save(&store, &loaded).unwrap();
    let stored: Value = serde_json::from_str(&store.value().unwrap()).unwrap();
    assert_eq!(stored["version"], 2);
    assert!(stored.get("api_key").is_none());
    assert_eq!(stored["exchanges"]["binance"]["api_secret"], "olds");
}

#[test]
fn v1_without_exchange_keys_keeps_member_key_and_adds_no_binance_entry() {
    for payload in [json!({"api_key": "", "api_secret": "", "member_key": "mk"}), json!({"member_key": "mk"})] {
        let loaded = load_json(payload).unwrap();
        assert_eq!(loaded, Credentials { member_key: "mk".into(), ..Default::default() });
    }
}

#[test]
fn v1_half_pair_is_migrated_as_typed_and_non_strings_become_empty() {
    let loaded = load_json(json!({"api_key": "old", "api_secret": "", "member_key": "mk"})).unwrap();
    assert_eq!(loaded.exchanges["binance"], pair("old", ""));
    let loaded = load_json(json!({"api_key": 12345, "api_secret": ["x"], "member_key": {"a": 1}})).unwrap();
    assert_eq!(loaded, Credentials::default());
}

#[test]
fn not_a_json_object_is_invalid_data() {
    for raw in ["", "not json", "[1, 2]", "null", "\"text\"", "{\"version\": 2, \"exch"] {
        let result = load_result(&MemoryStore::with(raw));
        assert_eq!(result.status, LoadStatus::InvalidData, "{raw}");
        assert!(result.values.is_none());
    }
}

#[test]
fn malformed_exchanges_never_crash_and_become_empty() {
    for bad in [Value::Null, json!("binance"), json!(["binance"]), json!(7), json!([]), json!(true)] {
        let loaded = load_json(json!({"version": 2, "member_key": "mk", "exchanges": bad})).unwrap();
        assert_eq!(loaded, Credentials { member_key: "mk".into(), ..Default::default() });
    }
    assert!(load_json(json!({"version": 2, "member_key": "mk"})).unwrap().exchanges.is_empty());
}

#[test]
fn bad_pair_is_skipped_and_non_strings_become_empty() {
    let loaded = load_json(json!({"version": 2, "member_key": "mk", "exchanges": {
        "binance": "oops", "upbit": ["k", "s"], "bithumb": {"api_key": "bk", "api_secret": "bs"}}}))
    .unwrap();
    assert_eq!(loaded.exchanges, [("bithumb".to_string(), pair("bk", "bs"))].into_iter().collect());
    let loaded = load_json(json!({"version": 2, "member_key": 5, "exchanges": {
        "upbit": {"api_key": "uk", "api_secret": 99}, "binance": {"api_key": null, "api_secret": {"x": 1}}}}))
    .unwrap();
    assert_eq!(loaded.exchanges, [("upbit".to_string(), pair("uk", ""))].into_iter().collect());
    assert_eq!(loaded.member_key, "");
}

#[test]
fn empty_pairs_are_not_stored_and_future_version_is_not_migrated() {
    let mut c = Credentials { member_key: "mk".into(), ..Default::default() };
    c.exchanges.insert("binance".into(), pair("", ""));
    c.exchanges.insert("upbit".into(), pair("uk", "us"));
    assert_eq!(roundtrip(&c).exchanges.keys().collect::<Vec<_>>(), vec!["upbit"]);
    let loaded = load_json(json!({"version": 3, "member_key": "mk", "api_key": "stray",
        "exchanges": {"upbit": {"api_key": "uk", "api_secret": "us"}}}))
    .unwrap();
    assert_eq!(loaded.exchanges.keys().collect::<Vec<_>>(), vec!["upbit"]);
}

#[test]
fn missing_unreadable_and_decrypt_statuses_are_distinct_and_hide_keys() {
    assert_eq!(load_result(&MemoryStore::new()).status, LoadStatus::Missing);
    let store = MemoryStore::with("{}");
    store.set_fail_reads(true);
    assert_eq!(load_result(&store).status, LoadStatus::ReadError);
    store.set_fail_reads(false);
    store.set_decrypt_error(true);
    assert_eq!(load_result(&store).status, LoadStatus::DecryptError);
    let result = load_result(&MemoryStore::with(
        json!({"api_key": "fixture-key", "api_secret": "fixture-secret", "member_key": "fixture-member"}).to_string(),
    ));
    assert_eq!(result.status, LoadStatus::Loaded);
    assert_eq!(result.values.as_ref().unwrap().exchanges["binance"].api_secret, "fixture-secret");
    let debug = format!("{result:?}");
    assert!(!debug.contains("fixture-secret") && !debug.contains("fixture-member"));
    assert_eq!(serde_json::to_value(LoadStatus::DecryptError).unwrap(), "decrypt_error");
}

// --- ExchangeRememberChoiceTests ---

fn previous() -> Credentials {
    let mut c = Credentials { member_key: "saved-member".into(), ..Default::default() };
    c.exchanges.insert("binance".into(), pair("bk", "bs"));
    c.exchanges.insert("upbit".into(), pair("uk", "us"));
    c.connection_history.insert("binance".into(), obs("8.8.8.8", 100.0));
    c
}

fn disk(c: &Credentials) -> MemoryStore {
    let store = MemoryStore::new();
    save(&store, c).unwrap();
    store
}

#[test]
fn selected_save_keeps_other_pairs_and_accepts_only_the_selected_input() {
    let original = previous();
    let store = disk(&original);
    let mut input = Credentials { member_key: "current-member".into(), ..Default::default() };
    input.exchanges.insert("binance".into(), pair("must-not-overwrite", "must-not-overwrite"));
    input.exchanges.insert("upbit".into(), pair("new-upbit", "new-secret"));
    input.connection_history.insert("upbit".into(), obs("1.1.1.1", 101.0));
    input.connection_history.insert("binance".into(), obs("9.9.9.9", 999.0));
    let result = apply_exchange_choice(&store, KeySlot::Upbit, true, &input).unwrap().unwrap();
    assert_eq!(result.exchanges["upbit"].api_key, "new-upbit");
    assert_eq!(result.exchanges["binance"], original.exchanges["binance"]);
    assert_eq!(result.connection_history["binance"], original.connection_history["binance"]);
    assert_eq!(result.connection_history["upbit"].public_ipv4, "1.1.1.1");
    assert_eq!(result.member_key, "current-member");
    assert_eq!(load(&store).unwrap(), result);
}

#[test]
fn selected_delete_keeps_other_pairs_and_the_saved_member_key() {
    let store = disk(&previous());
    let input = Credentials { member_key: "unrelated-current-member".into(), ..Default::default() };
    let result = apply_exchange_choice(&store, KeySlot::Upbit, false, &input).unwrap().unwrap();
    assert_eq!(result.exchanges.keys().collect::<Vec<_>>(), vec!["binance"]);
    assert_eq!(result.member_key, "saved-member");
    assert!(store.value().is_some());
}

#[test]
fn deleting_the_last_exchange_preserves_a_member_only_store() {
    let mut prev = Credentials { member_key: "saved-member".into(), ..Default::default() };
    prev.exchanges.insert("upbit".into(), pair("uk", "us"));
    prev.connection_history.insert("upbit".into(), obs("8.8.8.8", 100.0));
    let store = disk(&prev);
    let result = apply_exchange_choice(&store, KeySlot::Upbit, false, &Credentials::default()).unwrap();
    assert_eq!(result, Some(Credentials { member_key: "saved-member".into(), ..Default::default() }));
    assert_eq!(store.value().unwrap(), "{\"version\": 2, \"member_key\": \"saved-member\", \"exchanges\": {}}");
}

#[test]
fn deleting_the_last_exchange_with_no_member_removes_the_entry() {
    let mut prev = Credentials::default();
    prev.exchanges.insert("upbit".into(), pair("uk", "us"));
    let store = disk(&prev);
    assert!(apply_exchange_choice(&store, KeySlot::Upbit, false, &Credentials::default()).unwrap().is_none());
    assert!(store.value().is_none());
}

#[test]
fn member_only_store_is_preserved_when_selected_key_does_not_exist() {
    let prev = Credentials { member_key: "saved-member".into(), ..Default::default() };
    let store = disk(&prev);
    assert_eq!(apply_exchange_choice(&store, KeySlot::Upbit, false, &Credentials::default()).unwrap(), Some(prev));
    assert!(store.value().is_some());
}

#[test]
fn disk_is_authoritative_when_another_runner_changed_saved_keys() {
    let mut on_disk = Credentials { member_key: "disk-member".into(), ..Default::default() };
    on_disk.exchanges.insert("binance".into(), pair("latest-binance", "latest-secret"));
    on_disk.exchanges.insert("bithumb".into(), pair("new-bithumb", "new-secret"));
    on_disk.connection_history.insert("bithumb".into(), obs("1.1.1.1", 100.0));
    let store = disk(&on_disk);
    let mut input = Credentials::default();
    input.exchanges.insert("upbit".into(), pair("chosen", "chosen-secret"));
    let result = apply_exchange_choice(&store, KeySlot::Upbit, true, &input).unwrap().unwrap();
    assert_eq!(result.exchanges["binance"], on_disk.exchanges["binance"]);
    assert_eq!(result.exchanges["bithumb"], on_disk.exchanges["bithumb"]);
    assert_eq!(result.member_key, "disk-member");
    assert_eq!(result.connection_history, on_disk.connection_history);
}

#[test]
fn save_failure_is_reported() {
    let store = disk(&previous());
    store.set_fail_writes(true);
    assert_eq!(apply_exchange_choice(&store, KeySlot::Upbit, true, &previous()).unwrap_err(), SAVE_FAILED);
    let mut last = Credentials::default();
    last.exchanges.insert("upbit".into(), pair("uk", "us"));
    let store = disk(&last);
    store.set_fail_writes(true);
    assert_eq!(
        apply_exchange_choice(&store, KeySlot::Upbit, false, &Credentials::default()).unwrap_err(),
        CLEAR_FAILED
    );
    assert!(store.value().is_some());
}

#[test]
fn missing_disk_does_not_resurrect_an_externally_deleted_cache() {
    let store = MemoryStore::new();
    let mut input = Credentials::default();
    input.exchanges.insert("upbit".into(), pair("chosen", "chosen-secret"));
    let result = apply_exchange_choice(&store, KeySlot::Upbit, true, &input).unwrap().unwrap();
    let mut expected = Credentials::default();
    expected.exchanges.insert("upbit".into(), pair("chosen", "chosen-secret"));
    assert_eq!(result, expected);
}

#[test]
fn incomplete_selected_keys_cannot_write() {
    for (k, s) in [("uk", ""), ("uk", "  "), ("", "")] {
        let store = MemoryStore::new();
        let mut input = Credentials::default();
        input.exchanges.insert("upbit".into(), pair(k, s));
        assert_eq!(apply_exchange_choice(&store, KeySlot::Upbit, true, &input).unwrap_err(), INCOMPLETE_PAIR);
        assert!(store.value().is_none());
    }
}

#[test]
fn unknown_existing_store_is_never_overwritten_on_load_failure() {
    let mut input = Credentials::default();
    input.exchanges.insert("upbit".into(), pair("uk", "us"));
    let invalid = MemoryStore::with("not json");
    let unreadable = MemoryStore::with("{}");
    unreadable.set_fail_reads(true);
    for store in [&invalid, &unreadable] {
        assert_eq!(apply_exchange_choice(store, KeySlot::Upbit, true, &input).unwrap_err(), UNREADABLE_NOT_CHANGED);
    }
    assert_eq!(invalid.value().unwrap(), "not json");
    let store = MemoryStore::with("{}");
    store.set_decrypt_error(true);
    assert_eq!(apply_exchange_choice(&store, KeySlot::Upbit, false, &input).unwrap_err(), UNREADABLE_NOT_CHANGED);
    assert_eq!(store.value().unwrap(), "{}");
}

// --- ConnectionHistoryStorageTests ---

#[test]
fn only_public_ipv4_and_valid_observation_time_are_roundtripped() {
    let raw = json!({"version": 2, "member_key": "fixture-member",
        "exchanges": {"binance": {"api_key": "fixture-key", "api_secret": "fixture-secret"}},
        "connection_history": {
            "upbit": {"public_ipv4": "8.8.8.8", "observed_at": 100, "registered": true, "api_secret": "drop-me"},
            "unknown": {"public_ipv4": "1.1.1.1", "observed_at": 100},
            "bithumb": {"public_ipv4": "192.168.0.1", "observed_at": 100}}});
    let loaded = load_json(raw).unwrap();
    assert_eq!(loaded.connection_history, [("upbit".to_string(), obs("8.8.8.8", 100.0))].into_iter().collect());
    let store = MemoryStore::new();
    save(&store, &loaded).unwrap();
    let text = store.value().unwrap();
    assert!(
        text.ends_with(", \"connection_history\": {\"upbit\": {\"public_ipv4\": \"8.8.8.8\", \"observed_at\": 100}}}")
    );
    assert!(!text.contains("drop-me"));
}

#[test]
fn bad_address_time_and_future_observations_are_dropped() {
    let future = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs_f64() + 600.0;
    for (address, observed) in [
        (json!("192.168.0.1"), json!(100)),
        (json!("224.0.0.1"), json!(100)),
        (json!("::1"), json!(100)),
        (json!("8.8.8.8"), json!(0)),
        (json!("8.8.8.8"), json!(true)),
        (json!("8.8.8.8"), json!("100")),
        (json!("8.8.8.8"), json!(future)),
        (json!(134744072), json!(100)),
    ] {
        let loaded = load_json(json!({"version": 2, "member_key": "m",
            "connection_history": {"upbit": {"public_ipv4": address, "observed_at": observed}}}))
        .unwrap();
        assert!(loaded.connection_history.is_empty());
    }
    for observed in [f64::NAN, f64::INFINITY, future] {
        let mut c = values("k", "s", "m");
        c.connection_history.insert("upbit".into(), obs("8.8.8.8", observed));
        assert!(roundtrip(&c).connection_history.is_empty());
        assert!(!to_json(&c).contains("connection_history"));
    }
}

#[test]
fn failed_write_never_replaces_existing_value() {
    let store = disk(&values("old", "old-secret", "old-member"));
    let original = store.value();
    store.set_fail_writes(true);
    assert_eq!(save(&store, &values("new", "new-secret", "new-member")).unwrap_err(), SAVE_FAILED);
    assert_eq!(store.value(), original);
}

#[test]
fn reset_unreadable_only_for_broken_entries() {
    for raw in ["not json", "[]"] {
        let store = MemoryStore::with(raw);
        reset_unreadable(&store).unwrap();
        assert!(store.value().is_none());
    }
    let store = MemoryStore::with("x");
    store.set_decrypt_error(true);
    reset_unreadable(&store).unwrap();
    assert!(store.value().is_none());
    let store = disk(&previous());
    assert_eq!(reset_unreadable(&store).unwrap_err(), STATE_CHANGED);
    assert!(store.value().is_some());
    assert_eq!(reset_unreadable(&MemoryStore::new()).unwrap_err(), STATE_CHANGED);
    // 키체인 접근 거부·잠김은 지우지 않음 — 항목은 멀쩡할 수 있음.
    let store = disk(&previous());
    store.set_fail_reads(true);
    assert_eq!(reset_unreadable(&store).unwrap_err(), STATE_CHANGED);
    store.set_fail_reads(false);
    assert!(store.value().is_some());
}

#[test]
fn special_characters_survive_roundtrip() {
    let c = values("k\"\\\n가", "s", "회원");
    assert_eq!(roundtrip(&c), c);
    assert!(to_json(&c).contains("회원"));
}

#[test]
#[ignore = "touches the real OS keychain"]
fn keyring_store_roundtrip() {
    let store = KeyringStore::new();
    let before = store.read().unwrap();
    save(&store, &values("fixture-key", "fixture-secret", "fixture-member")).unwrap();
    assert_eq!(load(&store).unwrap(), values("fixture-key", "fixture-secret", "fixture-member"));
    clear(&store).unwrap();
    assert_eq!(load_result(&store).status, LoadStatus::Missing);
    clear(&store).unwrap();
    if let Some(value) = before {
        store.write(&value).unwrap();
    }
}
