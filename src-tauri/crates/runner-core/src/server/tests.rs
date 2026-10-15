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
