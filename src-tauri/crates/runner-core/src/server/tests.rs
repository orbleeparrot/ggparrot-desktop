use super::*;
use wiremock::matchers::{header, method, path};
use wiremock::{Mock, MockServer, Request, ResponseTemplate};

fn client(base: &str, dir: &tempfile::TempDir) -> ServerClient {
    let outbox = Arc::new(EventOutbox::open(Some(dir.path().join("outbox.sqlite3"))));
    let mut c = ServerClient::new("ggp_member_key_value_123456", base, outbox, reqwest::Client::new());
    Arc::get_mut(&mut c.inner).unwrap().echo_stdout = false;
    c
}

#[tokio::test]
async fn start_retries_busy_then_reports_detail() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/api/runner/start"))
        .and(header("X-Runner-Key", "ggp_member_key_value_123456"))
        .respond_with(
            ResponseTemplate::new(503)
                .insert_header("Retry-After", "0")
                .set_body_json(json!({"detail": "database_busy"})),
        )
        .expect(4)
        .mount(&server)
        .await;
    let dir = tempfile::tempdir().unwrap();
    let c = client(&server.uri(), &dir);
    let err = c.start(&json!({"symbol": "BTCUSDT"})).await.unwrap_err();
    match err {
        StartError::Server(e) => {
            assert_eq!(e.status, 503);
            assert_eq!(e.to_string(), "database_busy");
        }
        _ => panic!("expected server error"),
    }
}

#[tokio::test]
async fn start_sets_session() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/api/runner/start"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(json!({"session_id": 42, "macro_origin": "file_modified"})),
        )
        .mount(&server)
        .await;
    let dir = tempfile::tempdir().unwrap();
    let c = client(&server.uri(), &dir);
    c.start(&json!({})).await.unwrap();
    assert_eq!(c.session_id(), Some(42));
    assert_eq!(c.macro_origin(), "file_modified");
}

#[tokio::test]
async fn heartbeat_offline_keeps_acks_for_next_time() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/api/runner/start"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"session_id": 7})))
        .mount(&server)
        .await;
    let dir = tempfile::tempdir().unwrap();
    let c = client(&server.uri(), &dir);
    c.start(&json!({})).await.unwrap();
    Mock::given(method("POST"))
        .and(path("/api/runner/heartbeat"))
        .respond_with(ResponseTemplate::new(500))
        .up_to_n_times(1)
        .mount(&server)
        .await;
    let r = c.heartbeat(&json!({"in_position": false}), vec![json!({"command_id": 1, "ok": true})]).await;
    assert!(r.offline);
    assert_eq!(r.action, "continue");
    // 백오프 중에는 보내지 않고 ack만 쌓아 둠.
    let before = server.received_requests().await.unwrap().len();
    let r = c.heartbeat(&json!({}), vec![]).await;
    assert!(r.offline);
    assert_eq!(server.received_requests().await.unwrap().len(), before, "백오프 중 요청 없음");
    assert!(c.has_unsent_acks());
    c.reset_backoff();
    server.reset().await;
    Mock::given(method("POST"))
        .and(path("/api/runner/heartbeat"))
        .respond_with(|req: &Request| {
            let body: Value = serde_json::from_slice(&req.body).unwrap();
            assert_eq!(body["session_id"], 7);
            assert_eq!(body["acks"].as_array().unwrap().len(), 2, "first ack re-sent ahead of new one");
            assert_eq!(body["acks"][0]["command_id"], 1);
            ResponseTemplate::new(200).set_body_json(json!({"action": "stop_only", "commands": [{"id": 9}]}))
        })
        .mount(&server)
        .await;
    let r = c.heartbeat(&json!({}), vec![json!({"command_id": 2, "ok": true})]).await;
    assert!(!r.offline);
    assert_eq!(r.action, "stop_only");
    assert_eq!(r.commands.len(), 1);
}

#[test]
fn heartbeat_backoff_grows_and_caps() {
    assert_eq!(heartbeat_backoff(0, 0.0), Duration::ZERO);
    assert_eq!(heartbeat_backoff(1, 0.0), Duration::from_secs(5));
    assert_eq!(heartbeat_backoff(3, 0.0), Duration::from_secs(20));
    assert_eq!(heartbeat_backoff(9, 0.0), Duration::from_secs(60));
    assert!(heartbeat_backoff(9, 1.0) <= Duration::from_secs(72));
}

#[tokio::test]
async fn auth_failure_and_session_end_are_told_apart() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/api/runner/start"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"session_id": 7})))
        .mount(&server)
        .await;
    let dir = tempfile::tempdir().unwrap();
    let c = client(&server.uri(), &dir);
    c.start(&json!({})).await.unwrap();
    Mock::given(method("POST"))
        .and(path("/api/runner/heartbeat"))
        .respond_with(ResponseTemplate::new(401))
        .up_to_n_times(1)
        .mount(&server)
        .await;
    let r = c.heartbeat(&json!({}), vec![]).await;
    assert!(r.offline && r.auth_failed);
    c.reset_backoff();
    Mock::given(method("POST"))
        .and(path("/api/runner/heartbeat"))
        .respond_with(ResponseTemplate::new(410))
        .mount(&server)
        .await;
    let r = c.heartbeat(&json!({}), vec![json!({"command_id": 3, "ok": true})]).await;
    assert_eq!(r.action, "stop_only", "서버가 끝낸 세션이면 멈춘다");
    assert!(c.has_unsent_acks(), "명령 결과는 종료 보고 전까지 남긴다");
}

#[tokio::test]
async fn events_are_deleted_only_after_server_accepts() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/api/runner/start"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"session_id": 3})))
        .mount(&server)
        .await;
    let dir = tempfile::tempdir().unwrap();
    let c = client(&server.uri(), &dir);
    c.start(&json!({})).await.unwrap();
    let ev = c.push_event("info", "hello ggp_member_key_value_123456", "INFO", "runner.message", "", &Map::new());
    assert!(!ev["message"].as_str().unwrap().contains("ggp_member_key_value_123456"));
    let key = ev["event_key"].as_str().unwrap().to_string();
    // 200이지만 저장 확인 없음 → 남아 있음
    Mock::given(method("POST"))
        .and(path("/api/runner/heartbeat"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"action": "continue"})))
        .up_to_n_times(1)
        .mount(&server)
        .await;
    c.heartbeat(&json!({}), vec![]).await;
    assert_eq!(c.peek_events(None).len(), 1);
    server.reset().await;
    let k2 = key.clone();
    Mock::given(method("POST"))
        .and(path("/api/runner/heartbeat"))
        .respond_with(move |_: &Request| {
            ResponseTemplate::new(200).set_body_json(json!({"action": "continue", "accepted_event_keys": [k2]}))
        })
        .mount(&server)
        .await;
    c.heartbeat(&json!({}), vec![]).await;
    assert!(c.peek_events(None).is_empty());
}

#[tokio::test]
async fn stopped_gives_up_on_client_error() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/api/runner/start"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"session_id": 3})))
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(path("/api/runner/stopped"))
        .respond_with(ResponseTemplate::new(404))
        .expect(1)
        .mount(&server)
        .await;
    let dir = tempfile::tempdir().unwrap();
    let c = client(&server.uri(), &dir);
    assert!(c.stopped("stopped", "", None).await, "no session yet → true");
    c.start(&json!({})).await.unwrap();
    assert!(!c.stopped("stopped", "종료", None).await);
}

#[tokio::test]
async fn claim_validates_payload() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/api/runner/launch-tickets/claim"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "macro": {"symbol": "KRW-BTC", "exchange": "upbit"}, "runner_key": " k ", "user_macro_id": 5,
            "launch_id": 11, "name": "내 매크로"})))
        .mount(&server)
        .await;
    let c = claim_ticket(&reqwest::Client::new(), &server.uri(), "t").await.unwrap();
    assert_eq!(c.runner_key, "k");
    assert_eq!(c.user_macro_id, Some(5));
    assert_eq!(c.launch_id, Some(11));
    assert_eq!(c.name.as_deref(), Some("내 매크로"));

    server.reset().await;
    Mock::given(method("POST"))
        .and(path("/api/runner/launch-tickets/claim"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"macro": {"symbol": ""}, "runner_key": "k"})))
        .mount(&server)
        .await;
    assert!(claim_ticket(&reqwest::Client::new(), &server.uri(), "t").await.is_err());
}

#[tokio::test]
async fn an_outdated_app_hears_the_servers_update_notice() {
    let server = MockServer::start().await;
    let detail = "바이낸스 빠른 실행은 실행기 v19 이상이 필요해요(연결 도우미). 실행기를 업데이트해 주세요.";
    Mock::given(method("POST"))
        .and(path("/api/runner/launch-tickets/claim"))
        .respond_with(ResponseTemplate::new(426).set_body_json(json!({"detail": detail})))
        .mount(&server)
        .await;
    let err = claim_ticket(&reqwest::Client::new(), &server.uri(), "t").await.unwrap_err();
    assert_eq!(err, ClaimError::UpdateRequired(detail.into()));

    // 주소가 섞인 안내·읽을 수 없는 본문은 비워 앱의 기본 안내를 씀. 다른 거절은 이유를 숨김.
    for (status, body) in [
        (426, json!({"detail": "https://evil.example 에서 받으세요"})),
        (426, json!({"detail": {"code": 1}})),
        (409, json!({"detail": "이미 쓴 티켓"})),
    ] {
        server.reset().await;
        Mock::given(method("POST"))
            .and(path("/api/runner/launch-tickets/claim"))
            .respond_with(ResponseTemplate::new(status).set_body_json(body))
            .mount(&server)
            .await;
        let err = claim_ticket(&reqwest::Client::new(), &server.uri(), "t").await.unwrap_err();
        let expected = if status == 426 { ClaimError::UpdateRequired(String::new()) } else { ClaimError::Failed };
        assert_eq!(err, expected, "{status}");
    }
}

#[tokio::test]
async fn permanent_rejections_and_stop_reasons_are_told_apart() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/api/runner/start"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"session_id": 8})))
        .mount(&server)
        .await;
    let dir = tempfile::tempdir().unwrap();
    let c = client(&server.uri(), &dir);
    c.start(&json!({})).await.unwrap();
    Mock::given(method("POST"))
        .and(path("/api/runner/heartbeat"))
        .respond_with(ResponseTemplate::new(422))
        .up_to_n_times(1)
        .mount(&server)
        .await;
    let r = c.heartbeat(&json!({}), vec![]).await;
    assert!(r.offline && !r.auth_failed);
    assert_eq!(r.rejected, 422, "기다려도 풀리지 않을 거절은 따로 알린다");
    c.reset_backoff();
    Mock::given(method("POST"))
        .and(path("/api/runner/heartbeat"))
        .respond_with(ResponseTemplate::new(503))
        .up_to_n_times(1)
        .mount(&server)
        .await;
    let r = c.heartbeat(&json!({}), vec![]).await;
    assert!(r.offline);
    assert_eq!(r.rejected, 0, "503 은 일시적인 끊김");
    c.reset_backoff();
    Mock::given(method("POST"))
        .and(path("/api/runner/heartbeat"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(json!({"action": "stop_only", "reason": "세션을 찾을 수 없어요.", "commands": []})),
        )
        .mount(&server)
        .await;
    let r = c.heartbeat(&json!({}), vec![]).await;
    assert_eq!((r.action.as_str(), r.reason.as_str()), ("stop_only", "세션을 찾을 수 없어요."));
}

#[tokio::test]
async fn a_start_without_a_session_id_is_refused() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/api/runner/start"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"ok": true})))
        .mount(&server)
        .await;
    let dir = tempfile::tempdir().unwrap();
    let c = client(&server.uri(), &dir);
    match c.start(&json!({})).await {
        Err(StartError::Server(e)) => assert!(e.detail.contains("세션 번호"), "{}", e.detail),
        other => panic!("세션 번호가 없으면 시작하지 않는다: {other:?}"),
    }
    assert_eq!(c.session_id(), None);
}

#[tokio::test]
async fn recovery_state_is_shared_and_skips_other_live_sessions() {
    let server = MockServer::start().await;
    let ids = Arc::new(std::sync::atomic::AtomicI64::new(20));
    let ids2 = ids.clone();
    Mock::given(method("POST"))
        .and(path("/api/runner/start"))
        .respond_with(move |_: &Request| {
            let id = ids2.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            ResponseTemplate::new(200).set_body_json(json!({"session_id": id}))
        })
        .mount(&server)
        .await;
    let dir = tempfile::tempdir().unwrap();
    let outbox = Arc::new(EventOutbox::open(Some(dir.path().join("outbox.sqlite3"))));
    let a = ServerClient::new("ggp_member_key_value_123456", &server.uri(), outbox.clone(), reqwest::Client::new());
    let b = ServerClient::new("ggp_member_key_value_123456", &server.uri(), outbox.clone(), reqwest::Client::new());
    a.start(&json!({})).await.unwrap();
    b.start(&json!({})).await.unwrap();
    let (sa, sb) = (a.session_id().unwrap(), b.session_id().unwrap());
    {
        let rec = outbox.recovery.lock().unwrap();
        assert!(rec.live.contains(&sa) && rec.live.contains(&sb), "두 실행의 세션이 같은 저장함에 보인다");
    }
    // 끝난 세션은 살아 있는 목록에서 빠짐(종료 보고가 실패해도).
    let _ = b.stopped("stopped", "", None).await;
    let rec = outbox.recovery.lock().unwrap();
    assert!(rec.live.contains(&sa) && !rec.live.contains(&sb));
}

#[tokio::test]
async fn a_claimed_member_key_must_look_like_a_key() {
    let server = MockServer::start().await;
    let dir = tempfile::tempdir().unwrap();
    let _ = dir;
    for (key, ok) in [("ggp_ok_key_123", true), ("bad key with space", false), ("", false)] {
        Mock::given(method("POST"))
            .respond_with(
                ResponseTemplate::new(200).set_body_json(json!({"macro": {"symbol": "BTCUSDT"}, "runner_key": key})),
            )
            .up_to_n_times(1)
            .mount(&server)
            .await;
        let r = claim_ticket(&reqwest::Client::new(), &server.uri(), "t").await;
        assert_eq!(r.is_ok(), ok, "{key:?}");
        if let Ok(c) = r {
            assert!(!format!("{c:?}").contains(key), "디버그 출력에 회원 키가 남지 않는다");
        }
    }
    let long = "k".repeat(crate::credentials::MAX_KEY_CHARS + 1);
    Mock::given(method("POST"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(json!({"macro": {"symbol": "BTCUSDT"}, "runner_key": long})),
        )
        .mount(&server)
        .await;
    assert!(claim_ticket(&reqwest::Client::new(), &server.uri(), "t").await.is_err());
}

// --- 메모리 보관·손실 알림(maintain_locked) 경계 ---------------------------------

struct Memo {
    _dir: tempfile::TempDir,
    now: Arc<std::sync::atomic::AtomicI64>,
    client: ServerClient,
    warnings: Arc<Mutex<Vec<String>>>,
}

impl Memo {
    fn new(fault: crate::outbox::OutboxFault) -> Self {
        use std::sync::atomic::{AtomicI64, Ordering};
        let dir = tempfile::tempdir().unwrap();
        let now = Arc::new(AtomicI64::new(1_800_000_000_000));
        let tick = now.clone();
        let clock: crate::outbox::Clock = Arc::new(move || tick.load(Ordering::SeqCst) as f64 / 1000.0);
        let outbox = Arc::new(EventOutbox::open_with(
            Some(dir.path().join("outbox.sqlite3")),
            clock,
            crate::outbox::OutboxLimits::default(),
        ));
        outbox.set_fault(fault);
        let mut client =
            ServerClient::new("ggp_member_key_value_123456", "http://127.0.0.1:9", outbox, reqwest::Client::new());
        Arc::get_mut(&mut client.inner).unwrap().echo_stdout = false;
        let warnings: Arc<Mutex<Vec<String>>> = Arc::default();
        let sink = warnings.clone();
        client.set_event_warning(Arc::new(move |m: String| sink.lock().unwrap().push(m)));
        Memo { _dir: dir, now, client, warnings }
    }
    fn push(&self, n: usize) {
        for _ in 0..n {
            self.client.push_event("info", "x", "INFO", "runner.message", "", &Map::new());
        }
    }
    fn advance_secs(&self, s: i64) {
        self.now.fetch_add(s * 1000, std::sync::atomic::Ordering::SeqCst);
    }
    fn take_warnings(&self) -> Vec<String> {
        std::mem::take(&mut *self.warnings.lock().unwrap())
    }
    fn memory_len(&self) -> usize {
        self.client.inner.state.lock().unwrap().events.len()
    }
    fn loss(&self, reason: &str) -> u64 {
        self.client.inner.state.lock().unwrap().memory_loss_counts[reason]
    }
}

const STORAGE_WARNING: &str = "활동 기록을 이 기기에 저장하지 못했어요";

#[test]
fn a_working_disk_keeps_nothing_in_memory_and_warns_nothing() {
    let m = Memo::new(crate::outbox::OutboxFault::None);
    m.push(3);
    assert_eq!(m.memory_len(), 0);
    assert!(m.take_warnings().is_empty());
}

#[test]
fn memory_overflow_is_recorded_on_disk_and_noticed_once_a_minute() {
    // 쓰기만 막힘 — 메모리 보관 한도를 넘긴 수는 디스크 손실 기록으로 옮기고, 디스크 알림으로 알림.
    let m = Memo::new(crate::outbox::OutboxFault::Append);
    m.push(outbox::MEMORY_MAX_ROWS);
    let w = m.take_warnings();
    assert_eq!(w.len(), 1, "{w:?}");
    assert!(w[0].starts_with(STORAGE_WARNING), "저장 실패 경고는 한 번만");
    m.push(1);
    assert_eq!(m.memory_len(), outbox::MEMORY_MAX_ROWS);
    assert_eq!(m.loss("memory_capacity"), 1);
    let w = m.take_warnings();
    assert_eq!(w.len(), 1, "{w:?}");
    assert!(w[0].contains("활동 기록 1건") && w[0].contains("7일 지남 0건 · 용량 초과 1건"), "{}", w[0]);
    {
        let st = m.client.inner.state.lock().unwrap();
        assert_eq!(st.memory_flushed_loss_counts["memory_capacity"], 1, "디스크에 옮긴 수");
    }
    // 1분 안의 손실은 다음 알림으로 미룸.
    m.push(1);
    assert!(m.take_warnings().is_empty());
    m.advance_secs(61);
    m.push(1);
    let w = m.take_warnings();
    assert_eq!(w.len(), 1, "{w:?}");
    assert!(w[0].contains("활동 기록 2건") && w[0].contains("용량 초과 2건"), "{}", w[0]);
}

#[test]
fn expired_memory_records_are_counted_as_expired() {
    let m = Memo::new(crate::outbox::OutboxFault::Append);
    m.push(3);
    m.take_warnings();
    m.advance_secs(outbox::PENDING_TTL_SECONDS);
    m.push(1);
    assert_eq!(m.memory_len(), 1, "기한이 지난 3건은 지우고 새 1건만");
    assert_eq!(m.loss("memory_expired"), 3);
    let w = m.take_warnings();
    assert_eq!(w.len(), 1, "{w:?}");
    assert!(w[0].contains("활동 기록 3건") && w[0].contains("7일 지남 3건 · 용량 초과 0건"), "{}", w[0]);
}

#[test]
fn without_any_disk_the_memory_counts_are_noticed_once_a_minute() {
    // 디스크 전체가 막힘 — 손실 수를 디스크에 옮기지 못하고 메모리의 수로 알림.
    let m = Memo::new(crate::outbox::OutboxFault::All);
    m.push(outbox::MEMORY_MAX_ROWS + 1);
    let w = m.take_warnings();
    assert_eq!(w.len(), 2, "{w:?}");
    assert!(w[0].starts_with(STORAGE_WARNING));
    assert!(w[1].contains("활동 기록 1건") && w[1].contains("용량 초과 1건"), "{}", w[1]);
    {
        let st = m.client.inner.state.lock().unwrap();
        assert_eq!(st.memory_flushed_loss_counts["memory_capacity"], 0, "디스크에 옮기지 못함");
        assert_eq!(st.memory_noticed_loss_counts["memory_capacity"], 1);
    }
    m.push(2);
    assert!(m.take_warnings().is_empty(), "1분 안에는 다시 알리지 않음");
    m.advance_secs(outbox::MAINTENANCE_SECONDS);
    m.push(1);
    let w = m.take_warnings();
    assert_eq!(w.len(), 1, "{w:?}");
    assert!(w[0].contains("활동 기록 3건") && w[0].contains("용량 초과 3건"), "{}", w[0]);
}

#[tokio::test]
async fn heartbeat_carries_entry_wait_only_when_it_is_an_object() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/api/runner/start"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"session_id": 7})))
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(path("/api/runner/heartbeat"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "action": "continue",
            "entry_wait": {"rule": "I", "sentence": "가격이 102.00 위로 올라가면 사요.", "gap": "+1.0%", "last_check_ms": 1}
        })))
        .up_to_n_times(1)
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(path("/api/runner/heartbeat"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"action": "continue", "entry_wait": "oops"})))
        .mount(&server)
        .await;
    let dir = tempfile::tempdir().unwrap();
    let c = client(&server.uri(), &dir);
    c.start(&json!({})).await.unwrap();
    let r = c.heartbeat(&json!({}), vec![]).await;
    assert_eq!(r.entry_wait.as_ref().unwrap()["gap"], "+1.0%");
    let r = c.heartbeat(&json!({}), vec![]).await;
    assert!(r.entry_wait.is_none(), "객체가 아니면 버림");
}

#[tokio::test]
async fn entry_preview_sends_macro_and_market_with_member_key() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/api/runner/entry-preview"))
        .and(header("X-Runner-Key", "ggp_member_key_value_123456"))
        .respond_with(|req: &Request| {
            let body: Value = serde_json::from_slice(&req.body).unwrap();
            assert_eq!(body["macro"]["rule_type"], "F");
            assert_eq!(body["market"], "spot");
            ResponseTemplate::new(200)
                .set_body_json(json!({"waiting": true, "sentence": "RSI(14)가 30 아래로 내려가면 사요."}))
        })
        .mount(&server)
        .await;
    let m = json!({"rule_type": "F", "exchange": "upbit", "symbol": "KRW-BTC", "params": {}});
    let v = entry_preview(&reqwest::Client::new(), &server.uri(), "ggp_member_key_value_123456", &m).await.unwrap();
    assert_eq!(v["waiting"], true);
    // 서버가 실패하면 안내만 건너뜀
    assert!(entry_preview(&reqwest::Client::new(), &server.uri(), "wrong", &m).await.is_none());
}

#[test]
fn only_condition_rules_wait_for_entry() {
    for rule in ["B", "F", "G", "I", "J"] {
        assert!(is_waiting_macro(&json!({"rule_type": rule})), "{rule}");
    }
    assert!(!is_waiting_macro(&json!({"rule_type": "A"})));
    assert!(!is_waiting_macro(&json!({"rule_type": "E", "params": {"entry_mode": "immediate"}})));
    assert!(is_waiting_macro(&json!({"rule_type": "E", "params": {"entry_mode": "dip"}})));
}

#[tokio::test]
async fn local_fills_ride_with_acks_and_are_resent_until_delivered() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/api/runner/start"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"session_id": 8})))
        .mount(&server)
        .await;
    let dir = tempfile::tempdir().unwrap();
    let c = client(&server.uri(), &dir);
    c.start(&json!({})).await.unwrap();
    Mock::given(method("POST"))
        .and(path("/api/runner/heartbeat"))
        .respond_with(ResponseTemplate::new(503))
        .up_to_n_times(1)
        .mount(&server)
        .await;
    let fill = json!({"client_order_id": "ggp-local-1", "side": "sell", "executed_qty": 1.0, "reason": "stop_loss"});
    let r = c.heartbeat_with(&json!({}), vec![json!({"command_id": 1, "ok": true})], vec![fill.clone()]).await;
    assert!(r.offline);
    assert!(c.has_unsent_acks(), "로컬 체결도 남겨 둔다");
    c.reset_backoff();
    server.reset().await;
    Mock::given(method("POST"))
        .and(path("/api/runner/heartbeat"))
        .respond_with(|req: &Request| {
            let body: Value = serde_json::from_slice(&req.body).unwrap();
            assert_eq!(body["acks"][0]["command_id"], 1);
            assert_eq!(body["local_fills"].as_array().unwrap().len(), 2, "앞서 못 보낸 것을 먼저");
            assert_eq!(body["local_fills"][0]["client_order_id"], "ggp-local-1");
            assert_eq!(body["local_fills"][1]["client_order_id"], "ggp-local-2");
            ResponseTemplate::new(200).set_body_json(json!({"action": "continue", "commands": []}))
        })
        .mount(&server)
        .await;
    let second = json!({"client_order_id": "ggp-local-2", "side": "sell", "executed_qty": 0.5, "reason": "forced"});
    let r = c.heartbeat_with(&json!({}), vec![], vec![second]).await;
    assert!(!r.offline);
    assert!(!c.has_unsent_acks(), "보낸 뒤에는 비운다");
}
