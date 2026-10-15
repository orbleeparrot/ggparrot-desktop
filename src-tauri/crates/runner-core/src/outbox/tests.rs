use super::*;
use serde_json::json;
use std::sync::atomic::AtomicU64;

struct Fixture {
    _dir: tempfile::TempDir,
    path: PathBuf,
    now: Arc<AtomicU64>,
}

impl Fixture {
    fn new() -> Self {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("retention.sqlite3");
        Fixture { _dir: dir, path, now: Arc::new(AtomicU64::new(1_800_000_000_000)) }
    }
    fn clock(&self) -> Clock {
        let now = self.now.clone();
        Arc::new(move || now.load(Ordering::SeqCst) as f64 / 1000.0)
    }
    fn advance(&self, seconds: f64) {
        self.now.fetch_add((seconds * 1000.0) as u64, Ordering::SeqCst);
    }
    fn journal(&self) -> EventOutbox {
        self.journal_with(OutboxLimits::default())
    }
    fn journal_with(&self, limits: OutboxLimits) -> EventOutbox {
        EventOutbox::open_with(Some(self.path.clone()), self.clock(), limits)
    }
}

fn rows(n: i64) -> OutboxLimits {
    OutboxLimits { max_rows: n, ..Default::default() }
}

fn event(index: u64, text: &str) -> Value {
    json!({"event_key": format!("runner:{:032x}", index), "message": text})
}

#[test]
fn disk_ttl_and_row_quota_apply_globally_including_other_keys_and_orphans() {
    let f = Fixture::new();
    let journal = f.journal_with(rows(3));
    for index in 0..5 {
        let (scope, session) = if index < 3 { ("old-key", 0) } else { ("new-key", 7) };
        journal.append(scope, session, &event(index, "실제 기록")).unwrap();
        f.advance(1.0);
    }
    let stats = journal.loss_stats().unwrap();
    assert_eq!(stats.pending_rows, 3);
    assert_eq!(stats.disk_capacity, 2);
    f.advance((7 * 86400 + 1) as f64);
    journal.try_maintain(true).unwrap();
    let stats = journal.loss_stats().unwrap();
    assert_eq!(stats.pending_rows, 0);
    assert_eq!(stats.disk_expired, 3);
    assert_eq!(stats.disk_capacity, 2);
}

#[test]
fn utf8_payload_quota_removes_oldest_rows_without_fake_server_ack() {
    let f = Fixture::new();
    let journal = f.journal_with(OutboxLimits { max_bytes: 1024, ..Default::default() });
    for index in 0..10 {
        journal.append("same-account", 1, &event(index, &"가".repeat(100))).unwrap();
    }
    let stats = journal.loss_stats().unwrap();
    assert!(stats.pending_bytes <= 1024);
    assert_eq!(stats.disk_capacity, 10 - stats.pending_rows);
    let found = journal.peek("same-account", 1, MAX_BATCH, None).unwrap();
    let expected: Vec<Value> = ((10 - found.len() as u64)..10).map(|i| event(i, "x")["event_key"].clone()).collect();
    assert_eq!(found.iter().map(|e| e["event_key"].clone()).collect::<Vec<_>>(), expected);
    journal.close();
    let reopened = f.journal();
    assert_eq!(reopened.loss_stats().unwrap().disk_capacity, stats.disk_capacity);
}

#[test]
fn new_database_uses_incremental_compaction_and_fixed_size_loss_metadata() {
    let f = Fixture::new();
    let journal = f.journal_with(rows(2));
    for index in 0..12 {
        journal.append(&format!("rotated-account-{index}"), 0, &event(index, "실제 기록")).unwrap();
    }
    journal.try_maintain(true).unwrap();
    assert_eq!(journal.pragma_i64("PRAGMA auto_vacuum"), 2);
    assert_eq!(journal.pragma_i64("PRAGMA journal_size_limit"), 4 * 1024 * 1024);
    assert_eq!(journal.pragma_i64("SELECT COUNT(*) FROM retention_meta"), 1);
    assert_eq!(journal.loss_stats().unwrap().disk_capacity, 10);
}

#[test]
fn original_uuid_retry_never_renews_age_and_notice_is_a_persisted_bounded_delta() {
    let f = Fixture::new();
    let journal = f.journal();
    let e = event(1, "실제 기록");
    journal.append("scope", 1, &e).unwrap();
    f.advance((6 * 86400) as f64);
    journal.append("scope", 1, &e).unwrap();
    f.advance((86400 + 1) as f64);
    journal.try_maintain(true).unwrap();
    assert!(journal.peek("scope", 1, MAX_BATCH, None).unwrap().is_empty());
    let notice = journal.take_loss_notice();
    assert_eq!(notice["disk_expired"], 1);
    assert_eq!(notice.len(), 4);
    assert!(journal.take_loss_notice().is_empty());
    journal.close();
    let reopened = f.journal();
    assert!(reopened.take_loss_notice().is_empty());
}

#[test]
fn two_connections_share_one_capacity_limit() {
    let f = Fixture::new();
    let first = Arc::new(f.journal_with(rows(3)));
    let second = Arc::new(f.journal_with(rows(3)));
    first.try_maintain(true).unwrap();
    second.try_maintain(true).unwrap();
    let handles: Vec<_> = [(first.clone(), 0u64), (second.clone(), 20u64)]
        .into_iter()
        .map(|(journal, start)| {
            std::thread::spawn(move || -> Result<(), String> {
                for index in start..start + 20 {
                    journal.append(&format!("scope-{start}"), 0, &event(index, "실제 기록"))?;
                }
                Ok(())
            })
        })
        .collect();
    for handle in handles {
        handle.join().unwrap().unwrap();
    }
    assert_eq!(first.loss_stats().unwrap().pending_rows, 3);
    assert_eq!(first.loss_stats().unwrap().disk_capacity, 37);
}

#[test]
fn same_uuid_is_immutable_and_ack_is_account_and_session_scoped() {
    let f = Fixture::new();
    let journal = f.journal();
    let scope = namespace("https://example.invalid", "member-key-fixture");
    let e = event(9, "변하지 않는 원본");
    journal.append(&scope, 7, &e).unwrap();
    let mut changed = e.clone();
    changed["message"] = json!("다른 내용");
    assert!(journal.append(&scope, 7, &changed).unwrap_err().contains("immutable"));
    journal.append(&scope, 7, &e).unwrap(); // same payload is idempotent
    journal.append(&scope, 8, &e).unwrap();
    journal.append("different-account", 7, &e).unwrap();
    let key = e["event_key"].as_str().unwrap().to_string();
    journal.acknowledge(&scope, 7, &[key]).unwrap();
    assert!(journal.peek(&scope, 7, MAX_BATCH, None).unwrap().is_empty());
    assert_eq!(journal.peek(&scope, 8, MAX_BATCH, None).unwrap(), vec![e.clone()]);
    assert_eq!(journal.peek("different-account", 7, MAX_BATCH, None).unwrap(), vec![e]);
}

#[test]
fn invalid_identity_session_and_size_are_refused() {
    let f = Fixture::new();
    let journal = f.journal();
    assert_eq!(journal.append("s", 1, &json!({"event_key": "runner:xyz"})).unwrap_err(), "invalid event identity");
    assert_eq!(journal.append("s", 1, &json!({"event_key": 5})).unwrap_err(), "invalid event identity");
    assert_eq!(journal.append("s", -1, &event(1, "x")).unwrap_err(), "invalid session scope");
    assert_eq!(journal.append("s", 1, &event(1, &"x".repeat(17000))).unwrap_err(), "event too large");
}

#[test]
fn unassigned_binding_peek_order_and_sessions() {
    let f = Fixture::new();
    let journal = f.journal();
    let mine = event(1, "내 것");
    let orphan = event(2, "남의 것");
    journal.append("scope", 0, &mine).unwrap();
    journal.append("scope", 0, &orphan).unwrap();
    let mine_key = mine["event_key"].as_str().unwrap().to_string();
    let orphan_key = orphan["event_key"].as_str().unwrap().to_string();
    assert_eq!(
        journal.existing_unassigned_keys("scope", &[mine_key.clone(), "runner:nope".into()]).unwrap(),
        vec![mine_key.clone()]
    );
    assert_eq!(
        journal.peek("scope", 0, MAX_BATCH, Some(&[orphan_key.clone(), mine_key.clone()])).unwrap(),
        vec![mine.clone(), orphan.clone()]
    );
    journal.bind_unassigned("scope", 0, std::slice::from_ref(&mine_key)).unwrap();
    journal.bind_unassigned("scope", 42, &[mine_key.clone(), "bad".into()]).unwrap();
    assert_eq!(journal.peek("scope", 42, MAX_BATCH, None).unwrap(), vec![mine]);
    assert_eq!(journal.peek("scope", 0, MAX_BATCH, None).unwrap(), vec![orphan]);
    journal.append("scope", 5, &event(3, "a")).unwrap();
    assert_eq!(journal.sessions("scope").unwrap(), vec![42, 5]);
    assert_eq!(journal.sessions("other").unwrap(), Vec::<i64>::new());
}

#[test]
fn memory_loss_is_recorded_and_noticed() {
    let f = Fixture::new();
    let journal = f.journal();
    let mut counts = BTreeMap::new();
    counts.insert("memory_capacity".to_string(), 5u64);
    counts.insert("disk_expired".to_string(), 9u64); // ignored
    journal.try_record_memory_loss(&counts).unwrap();
    let stats = journal.loss_stats().unwrap();
    assert_eq!((stats.memory_capacity, stats.disk_expired), (5, 0));
    assert_eq!(journal.take_loss_notice()["memory_capacity"], 5);
}

#[test]
fn unavailable_outbox_returns_errors_and_recovers() {
    let dir = tempfile::tempdir().unwrap();
    let blocker = dir.path().join("file");
    std::fs::write(&blocker, b"x").unwrap();
    let journal = EventOutbox::open(Some(blocker.join("sub").join("outbox.sqlite3")));
    assert!(!journal.is_available());
    assert!(journal.append("s", 0, &event(1, "x")).is_err());
    let f = Fixture::new();
    let journal = f.journal();
    journal.set_fault(OutboxFault::Append);
    assert!(journal.append("s", 0, &event(1, "x")).is_err());
    assert!(journal.peek("s", 0, 10, None).unwrap().is_empty());
    journal.set_fault(OutboxFault::All);
    assert!(journal.peek("s", 0, 10, None).is_err());
    journal.set_fault(OutboxFault::None);
    journal.append("s", 0, &event(1, "x")).unwrap();
}

#[test]
fn append_at_preserves_original_age() {
    let f = Fixture::new();
    let journal = f.journal();
    let created = journal.now_ms();
    f.advance((6 * 86400) as f64);
    journal.append_at("s", 7, &event(1, "x"), Some(created)).unwrap();
    f.advance((86400 + 1) as f64);
    journal.try_maintain(true).unwrap();
    assert_eq!(journal.loss_stats().unwrap().disk_expired, 1);
}

#[test]
fn redaction_patterns() {
    let text = redact(
        "member-key-fixture api_key=example-secret Bearer private-access-token password=example-password \
         https://provider.example/order?signature=private-signature",
        "member-key-fixture",
    );
    for secret in
        ["member-key-fixture", "example-secret", "private-access-token", "example-password", "private-signature"]
    {
        assert!(!text.contains(secret), "{text}");
    }
    assert!(text.contains("[키 숨김]"));
    assert!(text.contains("https://provider.example/order"));
    assert_eq!(redact("see https://u:p@host/x?y", ""), "see [authenticated-url]");
    assert_eq!(redact("see HTTPS://Host/x#frag", ""), "see https://Host/x");
    assert_eq!(redact("see http://[bad/x", ""), "see [url]");
    assert_eq!(redact("db postgres://u:p@h/db", ""), "db [connection]");
    assert_eq!(redact("sk-abcdefghijklmnop", ""), "[redacted]");
    assert_eq!(redact("eyJabcdefghi.abcdefghij.abcdefghij", ""), "[redacted]");
    assert_eq!(redact("Authorization: Bearer abc.def", ""), "Authorization: [redacted]");
    assert_eq!(redact("\"token\": \"abc\"", ""), "\"token\": [redacted]\"");
}

#[test]
fn namespace_matches_python_hash() {
    // hashlib.sha256(("ggp-event-outbox-v1\0" + "https://a.b" + "\0" + "k").encode()).hexdigest()
    let expected = hex::encode(Sha256::digest(b"ggp-event-outbox-v1\0https://a.b\0k"));
    assert_eq!(namespace("https://a.b//", "k"), expected);
    assert_ne!(namespace("https://a.b", "k"), namespace("https://a.b", "other"));
}

#[test]
fn python_json_encoding() {
    let v = json!({"b": [1, 2.5, null, true], "a": "가\"\n\u{7f}", "c": 1e16, "d": 0.00001, "e": 100.0});
    assert_eq!(
        py_dumps(&v, true, true, false),
        r#"{"a": "BSuac00\"\n\u007f", "b": [1, 2.5, null, true], "c": 1e+16, "d": 1e-05, "e": 100.0}"#
            .replace("BS", "\\")
    );
    assert_eq!(py_dumps(&json!({"a": "가"}), false, true, true), "{\"a\":\"가\"}");
    assert_eq!(py_dumps(&json!("😀"), true, false, false), r#""BSud83dBSude00""#.replace("BS", "\\"));
    assert_eq!(py_float(0.0001), "0.0001");
    assert_eq!(py_float(-0.0), "-0.0");
    assert_eq!(py_float(1234567890123456.0), "1234567890123456.0");
    assert_eq!(py_float(1.2345678901234568e17), "1.2345678901234568e+17");
}

#[test]
fn maximum_utf8_journal_record_still_fits_an_ascii_wire_batch() {
    let e = json!({"event_key": format!("runner:{}", "a".repeat(32)), "message": "é".repeat(8100)});
    assert!(py_dumps(&e, false, false, false).len() <= 16384);
    assert_eq!(wire_batch(vec![e.clone()], &json!({})), vec![e.clone()]);
    assert!(py_dumps(&json!({"events": [e]}), true, false, false).len() < MAX_WIRE_BODY_BYTES);
}

#[test]
fn wire_batch_respects_count_and_byte_budget_including_body() {
    let events: Vec<Value> = (0..150).map(|i| event(i, "x")).collect();
    assert_eq!(wire_batch(events.clone(), &Value::Null).len(), MAX_BATCH);
    let big: Vec<Value> = (0..100).map(|i| event(i, &"확인".repeat(400))).collect();
    let body = json!({"note": "상태 설명".repeat(100), "events": big.clone()});
    let batch = wire_batch(big.clone(), &body);
    assert!(!batch.is_empty() && batch.len() < big.len());
    assert_eq!(batch[..], big[..batch.len()]);
    let mut wire = body.clone();
    wire["events"] = Value::Array(batch.clone());
    assert!(py_dumps(&wire, true, false, false).len() <= MAX_WIRE_BODY_BYTES);
    let mut over = body;
    over["events"] = Value::Array(big[..batch.len() + 1].to_vec());
    assert!(py_dumps(&over, true, false, false).len() > MAX_WIRE_BODY_BYTES);
}

#[test]
fn default_path_is_under_the_app_data_activity_folder() {
    let path = default_path();
    assert!(
        path.ends_with("ggparrot-desktop/activity/outbox.sqlite3")
            || path.ends_with("ggparrot-desktop\\activity\\outbox.sqlite3")
    );
    #[cfg(target_os = "macos")]
    assert!(path.to_string_lossy().contains("Library/Application Support"));
}
