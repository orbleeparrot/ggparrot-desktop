use super::*;
use crate::types::KeyPair;
use std::collections::BTreeMap;

/// 기준점에서 secs초 뒤 — 음수도 쓰도록 1000초 띄운 기준점.
fn at(base: Instant, secs: f64) -> Instant {
    base + Duration::from_secs_f64(1000.0 + secs)
}

fn creds(pairs: &[(&str, &str, &str)], member: &str) -> Credentials {
    let mut c = Credentials { member_key: member.into(), ..Default::default() };
    for (ex, k, s) in pairs {
        c.exchanges.insert(ex.to_string(), KeyPair { api_key: k.to_string(), api_secret: s.to_string() });
    }
    c
}

// --- PublicIPTests ---

#[tokio::test]
async fn detects_one_global_ipv4_with_a_bounded_request() {
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"ip": "8.8.8.8"})))
        .expect(1)
        .mount(&server)
        .await;
    assert_eq!(detect_public_ipv4_from(&format!("{}/?format=json", server.uri())).await.unwrap(), "8.8.8.8");
}

#[tokio::test]
async fn rejects_redirects_bad_status_and_bad_json_without_echo() {
    use wiremock::matchers::method;
    use wiremock::{Mock, MockServer, ResponseTemplate};
    for (status, body) in [
        (301, json!({"ip": "8.8.8.8"})),
        (429, json!({})),
        (500, json!({})),
        (200, json!({"ip": "10.0.0.1"})),
        (200, json!(["8.8.8.8"])),
        (200, json!({"ip": 8})),
    ] {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .respond_with(
                ResponseTemplate::new(status).insert_header("Location", "https://evil.test/").set_body_json(body),
            )
            .expect(1)
            .mount(&server)
            .await;
        assert_eq!(detect_public_ipv4_from(&server.uri()).await.unwrap_err(), DETECT_IP_FAILURE);
    }
    // 연결 실패 — 오류 문장(주소)을 그대로 돌려주지 않음.
    let err = detect_public_ipv4_from("https://user:SECRET@127.0.0.1:9/").await.unwrap_err();
    assert_eq!(err, DETECT_IP_FAILURE);
    assert!(!err.contains("SECRET"));
}

#[test]
fn refuses_private_ipv6_and_malformed_addresses() {
    for value in [
        "192.168.1.1",
        "127.0.0.1",
        "224.0.0.1",
        "239.255.255.250",
        "::1",
        "2606:4700::1111",
        "key=secret",
        "",
        "100.64.0.1",
        "198.18.0.1",
        "192.0.0.1",
        "192.0.2.1",
        "203.0.113.9",
        "255.255.255.255",
        "240.0.0.1",
        "0.1.2.3",
        "169.254.1.1",
        "08.8.8.8",
    ] {
        assert_eq!(validate_public_ipv4(value).unwrap_err(), VALIDATE_IP_FAILURE, "{value}");
    }
    assert_eq!(validate_public_ipv4(" 8.8.8.8\n").unwrap(), "8.8.8.8");
    assert_eq!(validate_public_ipv4("192.0.0.9").unwrap(), "192.0.0.9");
}

// --- PreflightStateTests ---

fn fingerprint(macro_: Option<Value>, mode: RunMode, key: &str, secret: &str) -> String {
    preflight_fingerprint(
        &macro_.unwrap_or(json!({"exchange": "upbit", "symbol": "KRW-BTC", "params": {"initial_capital": 10000}})),
        mode,
        &creds(&[("upbit", key, secret)], ""),
    )
}

#[test]
fn setting_key_mode_and_notional_changes_invalidate_the_result() {
    let original = fingerprint(None, RunMode::Live, "key", "secret");
    assert_eq!(original, fingerprint(None, RunMode::Live, "key", "secret"));
    assert_ne!(original, fingerprint(None, RunMode::Mock, "key", "secret"));
    assert_ne!(original, fingerprint(None, RunMode::Live, "other", "secret"));
    assert_ne!(original, fingerprint(None, RunMode::Live, "key", "other"));
    assert_ne!(
        original,
        fingerprint(
            Some(json!({"exchange": "upbit", "symbol": "KRW-BTC", "params": {"initial_capital": 20000}})),
            RunMode::Live,
            "key",
            "secret"
        )
    );
    assert!(!original.contains("secret"));
    assert_eq!(original.len(), 64);
}

#[test]
fn other_exchange_keys_are_not_part_of_the_selected_check() {
    let m = json!({"exchange": "upbit", "symbol": "KRW-BTC"});
    let mut c = creds(&[("upbit", "k", "s")], "");
    let original = preflight_fingerprint(&m, RunMode::Live, &c);
    c.exchanges.insert("bithumb".into(), KeyPair { api_key: "another".into(), api_secret: "other".into() });
    assert_eq!(original, preflight_fingerprint(&m, RunMode::Live, &c));
}

#[test]
fn fingerprint_reads_the_exchange_name_like_the_session_payload() {
    // 앞뒤 공백·대문자가 섞인 거래소 이름도 세션 시작 payload와 같은 이름(upbit)의 키로 지문을 만듦.
    let m = json!({"exchange": " Upbit ", "symbol": "KRW-BTC"});
    assert_eq!(crate::types::session_payload(&m, false, RunMode::Live)["exchange"], "upbit");
    let original = preflight_fingerprint(&m, RunMode::Live, &creds(&[("upbit", "k", "s")], ""));
    assert_ne!(original, preflight_fingerprint(&m, RunMode::Live, &creds(&[("upbit", "other", "s")], "")));
    // 문자열이 아니거나 빈 이름은 payload처럼 binance로 읽음.
    for odd in [json!(5), json!(true), json!("  "), json!(["upbit"])] {
        let m = json!({"exchange": odd, "symbol": "BTCUSDT"});
        assert_eq!(crate::types::session_payload(&m, false, RunMode::Live)["exchange"], "binance");
        let with = |key: &str| preflight_fingerprint(&m, RunMode::Live, &creds(&[("binance", key, "s")], ""));
        assert_ne!(with("k"), with("other"), "{odd}");
    }
}

#[test]
fn success_is_valid_only_for_the_current_snapshot_for_two_minutes() {
    let base = Instant::now();
    let mut result = PreflightResult::new(true, "확인", "fp");
    result.checked_at = at(base, 100.0);
    assert!(result.matches_at("fp", at(base, 219.0)));
    assert!(!result.matches_at("changed", at(base, 101.0)));
    assert!(!result.matches_at("fp", at(base, 220.0)));
    assert!(!result.matches_at("fp", at(base, 99.0)));
    let mut failed = PreflightResult::new(false, "실패", "fp");
    failed.checked_at = at(base, 100.0);
    assert!(!failed.matches_at("fp", at(base, 101.0)));
}

#[test]
fn error_redaction_never_displays_selected_keys() {
    let c = creds(&[("upbit", "ABC", "XYZ")], "MEMBER");
    let safe = safe_preflight_reason("key ABC and secret XYZ failed", &c);
    assert_eq!(safe, "key [키 숨김] and secret [키 숨김] failed");
    assert!(!safe_preflight_reason("MEMBER", &c).contains("MEMBER"));
    // longest first: a key containing another key is masked whole.
    let c = creds(&[("upbit", "AB", "ABCD")], "");
    assert_eq!(safe_preflight_reason("ABCD", &c), "[키 숨김]");
    assert_eq!(safe_preflight_reason(&"가".repeat(700), &Credentials::default()).chars().count(), 600);
}

#[test]
fn signed_urls_proxy_credentials_and_jwt_are_not_rendered() {
    for reason in [
        "ProxyError https://proxy-user:proxy-password@example.test",
        "failure https://api.test?signature=SIGNED&timestamp=1",
        "Authorization: Bearer eyJtoken.payload.signature",
        "proxy-user:proxy-password@example.test refused",
    ] {
        let safe = safe_preflight_reason(reason, &Credentials::default());
        for forbidden in ["proxy-user", "proxy-password", "SIGNED", "eyJtoken", "https://"] {
            assert!(!safe.contains(forbidden));
        }
        assert_eq!(safe, GENERIC_PREFLIGHT_REASON);
    }
}

// --- PublicIPCacheTests ---

fn request_token(outcome: BeginOutcome) -> IpToken {
    match outcome {
        BeginOutcome::Request(t) => t,
        other => panic!("expected request, got {other:?}"),
    }
}

#[test]
fn restarted_cache_compares_a_saved_observation_without_claiming_registration() {
    let base = Instant::now();
    let mut cache = PublicIpCache::new();
    assert!(cache.seed_previous(Some(&IpObservation { public_ipv4: "8.8.8.8".into(), observed_at: 1710000000.0 })));
    let token = request_token(cache.begin_at(false, at(base, 100.0)));
    let result = cache.finish_at(token, Ok("1.1.1.1".into()), at(base, 101.0)).unwrap();
    assert!(result.changed);
    let status = public_ip_status_at(Some(&result), at(base, 102.0));
    assert!(status.contains("전에 확인한 주소와 달라요"));
    assert!(!status.contains("등록 완료"));
}

#[test]
fn invalid_saved_observations_do_not_replace_the_process_baseline() {
    let base = Instant::now();
    let mut cache = PublicIpCache::new();
    assert!(cache.seed_previous_value(&json!({"public_ipv4": "8.8.8.8", "observed_at": 1710000000})));
    for history in [
        Value::Null,
        json!({}),
        json!({"public_ipv4": "192.168.1.2", "observed_at": 1710000000}),
        json!({"public_ipv4": "1.1.1.1", "observed_at": "invalid"}),
        json!({"public_ipv4": "1.1.1.1", "observed_at": true}),
        json!({"public_ipv4": "1.1.1.1", "observed_at": 1e15}),
        json!({"public_ipv4": "1.1.1.1", "observed_at": 0}),
        json!({"public_ipv4": 16843009, "observed_at": 1710000000}),
    ] {
        assert!(!cache.seed_previous_value(&history), "{history}");
    }
    assert!(!cache.seed_previous(None));
    assert!(!cache.seed_previous(Some(&IpObservation { public_ipv4: "1.1.1.1".into(), observed_at: f64::NAN })));
    assert!(!cache.seed_previous(Some(&IpObservation { public_ipv4: "1.1.1.1".into(), observed_at: f64::INFINITY })));
    let token = request_token(cache.begin_at(false, at(base, 100.0)));
    assert!(!cache.finish_at(token, Ok("8.8.8.8".into()), at(base, 101.0)).unwrap().changed);
}

#[test]
fn duplicate_requests_share_one_inflight_token() {
    let base = Instant::now();
    let mut cache = PublicIpCache::new();
    let token = request_token(cache.begin_at(false, at(base, 100.0)));
    assert_eq!(cache.begin_at(false, at(base, 101.0)), BeginOutcome::Shared(token));
    assert!(cache.busy());
}

#[test]
fn success_is_reused_for_120_seconds_then_rechecked() {
    let base = Instant::now();
    let mut cache = PublicIpCache::new();
    let token = request_token(cache.begin_at(false, at(base, 100.0)));
    assert!(cache.finish_at(token, Ok("8.8.8.8".into()), at(base, 100.0)).unwrap().ok);
    assert_eq!(cache.begin_at(false, at(base, 219.0)), BeginOutcome::Cached);
    assert!(matches!(cache.begin_at(false, at(base, 220.0)), BeginOutcome::Request(_)));
    assert!(cache.result().is_none());
}

#[test]
fn failure_is_cached_without_retaining_a_previous_copyable_ip() {
    let base = Instant::now();
    let mut cache = PublicIpCache::new();
    let token = request_token(cache.begin_at(false, at(base, 100.0)));
    cache.finish_at(token, Ok("8.8.8.8".into()), at(base, 100.0));
    let token = request_token(cache.begin_at(true, at(base, 110.0)));
    assert!(cache.result().is_none());
    let result = cache.finish_at(token, Err("ProxyError secret".into()), at(base, 111.0)).unwrap();
    assert!(!result.ok);
    assert_eq!(result.address, "");
    assert!(!result.reason.contains("secret"));
    assert_eq!(result.reason, DETECT_IP_FAILURE);
    assert_eq!(public_ip_status_at(Some(&result), at(base, 112.0)), DETECT_IP_FAILURE);
    assert_eq!(cache.begin_at(false, at(base, 112.0)), BeginOutcome::Cached);
    assert!(matches!(cache.begin_at(true, at(base, 113.0)), BeginOutcome::Request(_)));
}

#[test]
fn invalid_value_counts_as_failure() {
    let base = Instant::now();
    let mut cache = PublicIpCache::new();
    let token = request_token(cache.begin_at(false, at(base, 100.0)));
    let result = cache.finish_at(token, Ok("192.168.0.1".into()), at(base, 100.0)).unwrap();
    assert!(!result.ok && result.address.is_empty());
}

#[test]
fn stale_exchange_generation_response_is_discarded() {
    let base = Instant::now();
    let mut cache = PublicIpCache::new();
    let token = request_token(cache.begin_at(false, at(base, 100.0)));
    cache.invalidate();
    assert!(matches!(cache.begin_at(false, at(base, 101.0)), BeginOutcome::Shared(_)));
    assert!(cache.finish_at(token, Ok("8.8.8.8".into()), at(base, 102.0)).is_none());
    assert!(cache.result().is_none());
    assert!(!cache.busy());
}

#[test]
fn old_duplicate_response_cannot_overwrite_a_new_request() {
    let base = Instant::now();
    let mut cache = PublicIpCache::new();
    let old = request_token(cache.begin_at(false, at(base, 100.0)));
    cache.finish_at(old, Ok("8.8.8.8".into()), at(base, 101.0));
    let current = request_token(cache.begin_at(true, at(base, 102.0)));
    assert!(cache.finish_at(old, Ok("1.1.1.1".into()), at(base, 103.0)).is_none());
    assert!(cache.busy());
    assert_eq!(cache.finish_at(current, Ok("9.9.9.9".into()), at(base, 104.0)).unwrap().address, "9.9.9.9");
}

#[test]
fn changed_ip_compares_observations_without_claiming_registration() {
    let base = Instant::now();
    let mut cache = PublicIpCache::new();
    let token = request_token(cache.begin_at(false, at(base, 100.0)));
    assert!(!cache.finish_at(token, Ok("8.8.8.8".into()), at(base, 100.0)).unwrap().changed);
    cache.invalidate();
    let token = request_token(cache.begin_at(false, at(base, 101.0)));
    let result = cache.finish_at(token, Ok("1.1.1.1".into()), at(base, 102.0)).unwrap();
    assert!(result.changed);
    assert_eq!(result.address, "1.1.1.1");
    let status = public_ip_status_at(Some(&result), at(base, 102.0));
    assert!(!status.contains("등록 완료"));
    assert!(status.contains("달라"));
    assert!(status.starts_with("1.1.1.1 · "));
    assert!(status.contains(" 조회 당시 주소예요(2분 동안 유효). · 전에 확인한 주소와 달라요."));
}

#[test]
fn cached_expiry_and_routing_caveats_are_truthful() {
    let base = Instant::now();
    let mut result = PublicIpResult::new(true, "8.8.8.8", "확인");
    result.checked_at = at(base, 100.0);
    assert!(public_ip_status_at(Some(&result), at(base, 101.0)).contains("조회 당시"));
    assert!(public_ip_status_at(Some(&result), at(base, 220.0)).contains("만료"));
    assert_eq!(
        public_ip_status_at(None, base),
        "아직 주소를 확인하지 않았어요. 허용 IP 등록은 거래소 공식 페이지에서 직접 해요."
    );
    assert!(PublicIpResult::new(true, "8.8.8.8", "확인").fresh());
}

#[test]
fn official_api_management_pages_are_a_static_domestic_allowlist() {
    assert_eq!(api_management_url("upbit", "").unwrap(), "https://www.upbit.com/mypage/open_api_management");
    assert_eq!(
        api_management_url("bithumb", "live").unwrap(),
        "https://www.bithumb.com/react/api-support/management-api",
        "국내는 env를 보지 않음"
    );
    for ex in ["binance", "https://evil.test", "upbit?key=secret", "Upbit", "binance:live"] {
        assert_eq!(api_management_url(ex, "").unwrap_err(), "이 거래소의 공식 API 관리 페이지를 확인하지 못했어요.");
    }
}

#[test]
fn binance_env_follows_mode_and_market() {
    assert_eq!(binance_env(RunMode::Testnet, Market::Spot), "testnet");
    assert_eq!(binance_env(RunMode::Mock, Market::Spot), "testnet", "모의도 연습 환경");
    assert_eq!(binance_env(RunMode::Testnet, Market::Futures), "demo");
    assert_eq!(binance_env(RunMode::Mock, Market::Futures), "demo");
    assert_eq!(binance_env(RunMode::Live, Market::Futures), "live");
    assert_eq!(binance_env(RunMode::Live, Market::Spot), "live");
}

#[test]
fn each_binance_env_opens_its_own_official_page() {
    assert_eq!(api_management_url("binance", "testnet").unwrap(), "https://testnet.binance.vision/");
    assert_eq!(
        api_management_url("binance", "demo").unwrap(),
        "https://demo.binance.com/en/my/settings/api-management"
    );
    assert_eq!(api_management_url("binance", "live").unwrap(), "https://www.binance.com/en/my/settings/api-management");
    assert_eq!(exchange_home_url("binance", "testnet").unwrap(), "https://testnet.binance.vision/");
    assert_eq!(exchange_home_url("binance", "demo").unwrap(), "https://demo.binance.com/en/futures/BTCUSDT");
    assert_eq!(exchange_home_url("binance", "live").unwrap(), "https://www.binance.com/en");
    for env in ["", "mainnet", "LIVE", "live/../x"] {
        assert!(api_management_url("binance", env).is_err(), "{env}");
        assert!(exchange_home_url("binance", env).is_err(), "{env}");
    }
}

// --- ConnectionHistoryTests ---

struct History {
    base: Instant,
    m: Value,
    saved: Credentials,
    ip: PublicIpResult,
    check: PreflightResult,
}

impl History {
    fn new() -> Self {
        let base = Instant::now();
        let m = json!({"exchange": "upbit", "symbol": "KRW-BTC"});
        let mut saved = creds(
            &[("upbit", "fixture-key", "fixture-secret"), ("bithumb", "other-key", "other-secret")],
            "fixture-member",
        );
        saved
            .connection_history
            .insert("bithumb".into(), IpObservation { public_ipv4: "9.9.9.9".into(), observed_at: 1700000000.0 });
        let mut ip = PublicIpResult::new(true, "8.8.8.8", "조회");
        ip.checked_at = at(base, 100.0);
        ip.observed_at = 1710000000.0;
        let mut check = PreflightResult::new(true, "검사", preflight_fingerprint(&m, RunMode::Live, &saved));
        check.checked_at = at(base, 100.0);
        History { base, m, saved, ip, check }
    }
    fn updated(
        &self,
        saved: Option<Credentials>,
        current: Option<Credentials>,
        ip: Option<PublicIpResult>,
        check: Option<PreflightResult>,
        mode: RunMode,
    ) -> Option<Credentials> {
        with_successful_ip_history_at(
            saved.as_ref().unwrap_or(&self.saved),
            &self.m,
            mode,
            current.as_ref().unwrap_or(&self.saved),
            ip.as_ref().unwrap_or(&self.ip),
            check.as_ref().unwrap_or(&self.check),
            at(self.base, 101.0),
        )
    }
    fn ip_at(&self, ok: bool, address: &str, checked: f64, observed: f64) -> PublicIpResult {
        let mut ip = PublicIpResult::new(ok, address, "x");
        ip.checked_at = at(self.base, checked);
        ip.observed_at = observed;
        ip
    }
    fn check_at(&self, ok: bool, checked: f64) -> PreflightResult {
        let mut check = PreflightResult::new(ok, "x", self.check.fingerprint.clone());
        check.checked_at = at(self.base, checked);
        check
    }
}

#[test]
fn success_adds_only_an_observation_preserving_all_saved_keys_and_other_history() {
    let h = History::new();
    let updated = h.updated(None, None, None, None, RunMode::Live).unwrap();
    assert_eq!(updated.exchanges, h.saved.exchanges);
    assert_eq!(updated.member_key, "fixture-member");
    assert_eq!(updated.connection_history["bithumb"], h.saved.connection_history["bithumb"]);
    assert_eq!(
        updated.connection_history["upbit"],
        IpObservation { public_ipv4: "8.8.8.8".into(), observed_at: 1710000000.0 }
    );
    assert!(!h.saved.connection_history.contains_key("upbit"));
    // same observation again → nothing to save
    assert!(h.updated(Some(updated.clone()), Some(updated), None, None, RunMode::Live).is_none());
}

#[test]
fn padded_exchange_name_records_history_under_the_exchange() {
    let mut h = History::new();
    h.m = json!({"exchange": " UPBIT", "symbol": "KRW-BTC"});
    h.check = PreflightResult::new(true, "검사", preflight_fingerprint(&h.m, RunMode::Live, &h.saved));
    h.check.checked_at = at(h.base, 100.0);
    let updated = h.updated(None, None, None, None, RunMode::Live).unwrap();
    assert_eq!(updated.connection_history["upbit"].public_ipv4, "8.8.8.8");
}

#[test]
fn unsaved_edited_mock_failed_or_stale_checks_cannot_create_history() {
    let h = History::new();
    assert!(h.updated(Some(Credentials::default()), None, None, None, RunMode::Live).is_none());
    assert!(h.updated(None, Some(creds(&[("upbit", "new", "new")], "")), None, None, RunMode::Live).is_none());
    assert!(h.updated(None, None, None, None, RunMode::Mock).is_none());
    assert!(h.updated(None, None, None, Some(h.check_at(false, 100.0)), RunMode::Live).is_none());
    assert!(h.updated(None, None, None, Some(h.check_at(true, -100.0)), RunMode::Live).is_none());
    assert!(h.updated(None, None, Some(h.ip_at(true, "8.8.8.8", -100.0, 1710000000.0)), None, RunMode::Live).is_none());
    assert!(h.updated(None, None, Some(h.ip_at(false, "", 100.0, 1710000000.0)), None, RunMode::Live).is_none());
    assert!(
        h.updated(None, None, Some(h.ip_at(true, "192.168.1.2", 100.0, 1710000000.0)), None, RunMode::Live).is_none()
    );
    assert!(h.updated(None, None, Some(h.ip_at(true, "8.8.8.8", 100.0, f64::NAN)), None, RunMode::Live).is_none());
    // binance is not a domestic exchange with an allow-list page
    let mut binance = History::new();
    binance.m = json!({"exchange": "binance"});
    assert!(binance.updated(None, None, None, None, RunMode::Live).is_none());
}

// --- connection views ---

#[test]
fn success_marks_every_row_but_order_permission_only_where_it_can_be_proven() {
    let upbit = check_view("upbit", true, "");
    assert_eq!((upbit.auth, upbit.ip, upbit.order), (Check::Ok, Check::Ok, Check::Ok));
    for ex in ["bithumb", "binance"] {
        let v = check_view(ex, true, "");
        assert_eq!((v.auth, v.ip, v.order), (Check::Ok, Check::Ok, Check::Na));
        assert!(v.connected);
        assert_eq!(v.headline, "연결 완료");
        assert_eq!(v.problem, "");
    }
}

#[test]
fn each_exchange_error_points_at_the_row_that_blocked() {
    use Check::*;
    let cases: BTreeMap<&str, (Check, Check, Check, &str, &str)> = [
        ("no_authorization_ip", (Ok, Bad, Wait, "ip", "허용 IP 확인 필요")),
        ("NotAllowIP", (Ok, Bad, Wait, "ip", "허용 IP 확인 필요")),
        ("invalid_access_key", (Bad, Wait, Wait, "auth", "키 확인 필요")),
        ("jwt_verification", (Bad, Wait, Wait, "auth", "키 확인 필요")),
        ("expired_access_key", (Bad, Wait, Wait, "auth", "키 확인 필요")),
        ("no_authorization_token", (Bad, Wait, Wait, "auth", "키 확인 필요")),
        ("out_of_scope", (Ok, Ok, Bad, "order", "주문 권한 확인 필요")),
        ("network", (Wait, Wait, Wait, "network", "연결 확인 필요")),
        ("something_new", (Wait, Wait, Wait, "other", "연결 확인 필요")),
    ]
    .into_iter()
    .collect();
    for (name, (auth, ip, order, problem, headline)) in cases {
        let v = check_view("upbit", false, name);
        assert_eq!((v.auth, v.ip, v.order), (auth, ip, order), "{name}");
        assert_eq!((v.problem.as_str(), v.headline.as_str()), (problem, headline));
        assert!(!v.connected);
    }
}

#[test]
fn binance_views_split_key_ip_and_permission() {
    use Check::*;
    // (검사 통과, 키 인증, 허용 IP, 주문 권한, problem, 제목) — 통과한 것만 connected.
    type Row = (bool, Check, Check, Check, &'static str, &'static str);
    let cases: [(&str, Row); 12] = [
        ("binance_verified", (true, Ok, Ok, Ok, "", "연결 완료")),
        ("binance_verified_open", (true, Ok, Na, Ok, "", "연결 완료")),
        ("binance_test_ok", (true, Ok, Na, Na, "", "연결 완료")),
        ("binance_key", (false, Bad, Wait, Wait, "auth", "키 확인 필요")),
        ("binance_access_test", (false, Bad, Na, Wait, "auth", "키 확인 필요")),
        ("binance_access", (false, Wait, Bad, Wait, "ip", "허용 IP 확인 필요")),
        ("binance_ip_unrestricted", (false, Ok, Bad, Bad, "ip", "허용 IP 등록 필요")),
        ("binance_no_trade", (false, Ok, Ok, Bad, "order", "주문 권한 확인 필요")),
        ("binance_no_futures", (false, Ok, Ok, Bad, "order", "주문 권한 확인 필요")),
        // 잔고(증거금) 부족 — 연결은 됐고 국내 잔고 부족과 같은 'order_size'.
        ("binance_funds", (false, Ok, Ok, Ok, "order_size", "연결 완료")),
        ("binance_funds_open", (false, Ok, Na, Ok, "order_size", "연결 완료")),
        ("binance_funds_test", (false, Ok, Na, Na, "order_size", "연결 완료")),
    ];
    for (name, (ok, auth, ip, order, problem, headline)) in cases {
        let connected = ok || problem == "order_size";
        let v = check_view("binance", ok, name);
        assert_eq!((v.auth, v.ip, v.order, v.connected), (auth, ip, order, connected), "{name}");
        assert_eq!((v.problem.as_str(), v.headline.as_str()), (problem, headline), "{name}");
        assert!(crate::setup_progress::PROBLEMS.contains(&v.problem.as_str()), "{name}");
    }
    assert_eq!(check_view("binance", true, "binance_test_ok").notes.ip, "필요 없음");
    assert_eq!(check_view("binance", false, "binance_access").notes.ip, "IP 또는 키 불일치");
    assert!(check_view("binance", false, "binance_no_futures").notes.order.contains("Enable Futures"));
    // 국내 이름은 바이낸스 판정을 타지 않고, 국내에서 바이낸스 이름은 모르는 오류임.
    assert_eq!(check_view("binance", false, "under_min_total").problem, "order_size");
    assert_eq!(check_view("upbit", false, "binance_key").problem, "other");
    assert_eq!(check_view("upbit", false, "out_of_scope").notes, CheckNotes::default());
}

#[test]
fn order_size_failures_still_mean_the_keys_are_connected() {
    for name in ["insufficient_funds_bid", "under_min_total", "under_min_total_market_bid", "over_krw_funds_bid"] {
        let v = check_view("upbit", false, name);
        assert!(v.connected);
        assert_eq!(v.problem, "order_size");
    }
    assert_eq!(serde_json::to_value(check_view("bithumb", false, "under_min_total")).unwrap()["order"], "na");
}

#[test]
fn exchange_keys_are_accepted_as_copied() {
    let upbit = [format!("a8Kq3Zt0{}", "x".repeat(32)), "6b2c78fd".repeat(4), format!("  {}9\n", "K".repeat(39))];
    for value in upbit {
        assert_eq!(clipboard_key("upbit", &value), value.trim());
    }
    // 빗썸: 발급마다 길이가 다름 — Access 46자 · 48자(소문자 16진), Secret 82자 + "==" · 84자(패딩 없음).
    let access = "a1".repeat(23);
    let secret = format!("{}Ab==", "Ab1/".repeat(20));
    let access_48 = "3f9a0c7e".repeat(6);
    let secret_84 = format!("{}OWM0", "OWM0ZTIxYjdkMDVh".repeat(5));
    assert_eq!((access.len(), secret.len(), access_48.len(), secret_84.len()), (46, 84, 48, 84));
    // 실패 메시지에는 키 값 대신 경우 이름만 남김(가짜 값이라도 로그에 키를 쓰지 않게).
    let cases = [
        ("access 46", &access, false),
        ("secret 82+==", &secret, true),
        ("access 48", &access_48, false),
        ("secret 84", &secret_84, true),
    ];
    for (case, key, is_secret) in cases {
        assert!(clipboard_key("bithumb", key) == *key, "{case}");
        assert!(clipboard_key("bithumb", &format!(" {key}\n")) == *key, "{case}");
        // 모양으로 칸을 가림(시크릿을 먼저 복사해도 맞는 칸에).
        assert!(clipboard_key_is_secret("bithumb", key) == Some(is_secret), "{case}");
    }
    assert_eq!(clipboard_key_is_secret("upbit", &access), None, "업비트는 복사 순서");
}

#[test]
fn binance_keys_are_taken_in_copy_order() {
    let api = "0lOHb8TL".repeat(8);
    let secret = "Zq9x".repeat(16);
    assert_eq!((api.len(), secret.len()), (64, 64));
    assert_eq!(clipboard_key("binance", &format!(" {api}\n")), api);
    assert_eq!(clipboard_key("binance", &secret), secret);
    assert_eq!(clipboard_key_is_secret("binance", &api), None, "두 칸이 같은 모양이라 복사 순서");
    for value in
        ["Passw0rdManagerGen3rated".to_string(), "a1".repeat(23), format!("{}+/", "a1".repeat(31)), "K".repeat(64)]
    {
        assert_eq!(clipboard_key("binance", &value), "", "{value}");
    }
}

#[test]
fn bithumb_capture_ignores_values_that_are_not_key_shaped() {
    for value in [
        "Passw0rdManagerGen3rated".to_string(), // 비밀번호 관리자가 만든 영문·숫자 비밀번호
        "a1".repeat(19),                        // 38자 — Access보다 짧음
        "a1".repeat(31),                        // 62자 — Access보다 길고 Secret보다 짧음
        format!("{}Ab=", "Ab1/".repeat(20)),    // 83자 — base64 길이(4의 배수)가 아님
        format!("{}+/", "a1".repeat(30)),       // 62자 — '+/'가 섞였는데 Secret보다 짧음
        "a1".repeat(51),                        // 102자 — Secret보다 김
        "ZDkyNGRkYWY0NTg2NDk0ZDk5YzE0NDRjYzE4YjJhZDc=".into(),
    ] {
        assert_eq!(clipboard_key("bithumb", &value), "", "{value}");
    }
}

#[test]
fn addresses_urls_sentences_and_short_values_are_ignored() {
    for value in [
        "121.134.55.20".to_string(),
        "https://upbit.com/mypage".into(),
        "API Key 복사".into(),
        "short".into(),
        String::new(),
        "a b".repeat(20),
        "가".repeat(30),
        "x".repeat(201),
        "K".repeat(40),
        // 다른 서비스 토큰·JWT 조각·기호 섞인 비밀번호
        // 토큰 모양만 흉내 냄(비밀 검사 도구가 실제 토큰으로 오인하지 않게 실행 중에 붙임).
        format!("{}_{}", "ghp", "1a2B3c4D5e6F7g8H9i0J1k2L3m4N5o6P7q8R"),
        "eyJhbGciOiJIUzI1NiJ9-abc_def123456789".into(),
    ] {
        assert_eq!(clipboard_key("upbit", &value), "");
        assert_eq!(clipboard_key("bithumb", &value), "");
    }
    assert_eq!(clipboard_key("upbit", "ZDkyNGRkYWY0NTg2NDk0ZDk5YzE0NDRjYzE4YjJhZDc="), "", "업비트 키에 = 는 없다");
}

#[test]
fn shortcuts_are_a_static_official_allowlist() {
    assert_eq!(exchange_home_url("upbit", "").unwrap(), "https://upbit.com/");
    assert_eq!(exchange_home_url("bithumb", "").unwrap(), "https://www.bithumb.com/");
    for bad in ["binance", "https://evil.example", ""] {
        assert_eq!(exchange_home_url(bad, "").unwrap_err(), "이 거래소의 공식 홈페이지를 확인하지 못했어요.");
    }
}
