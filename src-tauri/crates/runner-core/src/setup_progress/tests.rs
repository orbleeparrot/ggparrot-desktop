use super::*;
use std::sync::atomic::{AtomicU64, Ordering};
use wiremock::matchers::{header, method, path};
use wiremock::{Mock, MockServer, Request, Respond, ResponseTemplate};

#[test]
fn payload_has_only_the_whitelisted_fields() {
    let payload = progress_payload(
        "keys",
        "upbit",
        "",
        "new",
        (true, true),
        Some((Check::Ok, Check::Bad, Check::Wait)),
        "ip",
        false,
    )
    .unwrap();
    let keys: Vec<&String> = payload.as_object().unwrap().keys().collect();
    let mut expected = vec!["checks", "connected", "exchange", "path", "problem", "slots", "stage"];
    expected.sort();
    assert_eq!(keys, expected);
    assert_eq!(payload["slots"], json!({"access": true, "secret": true}));
    assert_eq!(payload["checks"], json!({"auth": "ok", "ip": "bad", "order": "wait"}));
    let default = progress_payload("ip", "upbit", "", "", (false, false), None, "", false).unwrap();
    assert_eq!(default["checks"], json!({"auth": "wait", "ip": "wait", "order": "wait"}));
    assert_eq!(default["slots"], json!({"access": false, "secret": false}));
}

#[test]
fn unknown_values_are_refused_not_sent() {
    let ok = |stage: &str, ex: &str, p: &str, problem: &str| {
        progress_payload(stage, ex, "", p, (false, false), None, problem, false)
    };
    for result in [
        ok("typed-key", "upbit", "", ""),
        ok("ip", "kraken", "", ""),
        ok("ip", "upbit", "web", ""),
        ok("ip", "upbit", "", "reason text"),
    ] {
        assert_eq!(result.unwrap_err(), "unknown progress field");
    }
}

#[test]
fn env_is_sent_only_for_binance_and_only_when_set() {
    let domestic = progress_payload("keys", "upbit", "", "new", (false, false), None, "", false).unwrap();
    assert!(domestic.get("env").is_none(), "빈 env는 키도 싣지 않음");
    let binance = progress_payload("perm", "binance", "", "new", (false, false), None, "", false).unwrap();
    assert!(binance.get("env").is_none());
    for env in ["testnet", "demo", "live"] {
        let p = progress_payload("perm", "binance", env, "new", (false, false), None, "", false).unwrap();
        assert_eq!(p["env"], env);
    }
    assert_eq!(
        progress_payload("perm", "upbit", "live", "new", (false, false), None, "", false).unwrap_err(),
        "env is binance-only"
    );
    assert_eq!(
        progress_payload("perm", "binance", "mainnet", "new", (false, false), None, "", false).unwrap_err(),
        "unknown progress field"
    );
}

struct Recorder {
    stages: Arc<Mutex<Vec<String>>>,
    status: u16,
}

impl Respond for Recorder {
    fn respond(&self, request: &Request) -> ResponseTemplate {
        let body: Value = serde_json::from_slice(&request.body).unwrap();
        self.stages.lock().unwrap().push(body["stage"].as_str().unwrap().to_string());
        ResponseTemplate::new(self.status)
    }
}

async fn server(status: u16) -> (MockServer, Arc<Mutex<Vec<String>>>) {
    let server = MockServer::start().await;
    let stages = Arc::new(Mutex::new(Vec::new()));
    Mock::given(method("POST"))
        .and(path("/api/runner/launch-tickets/41/progress"))
        .and(header("X-Runner-Key", "ggp_member"))
        .respond_with(Recorder { stages: stages.clone(), status })
        .mount(&server)
        .await;
    (server, stages)
}

fn payload(stage: &str, ex: &str) -> Value {
    progress_payload(stage, ex, "", "new", (false, false), None, "", false).unwrap()
}

async fn wait_for(stages: &Arc<Mutex<Vec<String>>>, n: usize) {
    for _ in 0..200 {
        if stages.lock().unwrap().len() >= n {
            return;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn latest_state_is_posted_with_the_member_key_header_and_duplicates_are_skipped() {
    let (server, stages) = server(200).await;
    let reporter =
        SetupProgressReporter::new(&format!("{}/", server.uri()), "ggp_member", 41, reqwest::Client::new()).unwrap();
    assert_eq!(reporter.url(), format!("{}/api/runner/launch-tickets/41/progress", server.uri()));
    let first = payload("ip", "upbit");
    reporter.report(first.clone());
    wait_for(&stages, 1).await;
    reporter.report(first); // 같은 상태는 다시 보내지 않음
    reporter.report(payload("perm", "upbit"));
    wait_for(&stages, 2).await;
    reporter.close();
    tokio::time::sleep(Duration::from_millis(50)).await;
    assert_eq!(*stages.lock().unwrap(), vec!["ip", "perm"]);
    assert!(reporter.is_finished());
}

#[tokio::test(flavor = "multi_thread")]
async fn failures_are_silent_and_closing_stops_further_reports() {
    let (server, stages) = server(500).await;
    let reporter = SetupProgressReporter::new(&server.uri(), "ggp_member", 41, reqwest::Client::new()).unwrap();
    reporter.report(payload("ask", "bithumb"));
    reporter.close();
    for _ in 0..200 {
        if reporter.is_finished() {
            break;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    assert!(reporter.is_finished());
    reporter.report(payload("ip", "bithumb"));
    tokio::time::sleep(Duration::from_millis(50)).await;
    assert!(stages.lock().unwrap().len() <= 1);
    // a failed send is not remembered as sent: unreachable host also stays silent
    let dead = SetupProgressReporter::new("http://127.0.0.1:9", "k", 7, reqwest::Client::new()).unwrap();
    dead.report(payload("ask", "upbit"));
    dead.finish(payload("closed", "upbit"), Duration::from_secs(2)).await;
}

#[tokio::test(flavor = "multi_thread")]
async fn finish_sends_the_closed_stage_before_the_window_goes() {
    let (server, stages) = server(200).await;
    let reporter = SetupProgressReporter::new(&server.uri(), "ggp_member", 41, reqwest::Client::new()).unwrap();
    reporter.report(payload("ready", "bithumb"));
    reporter.finish(payload("closed", "bithumb"), Duration::from_secs(2)).await;
    assert_eq!(stages.lock().unwrap().last().unwrap(), "closed");
    reporter.report(payload("ask", "bithumb")); // 닫힌 뒤에는 보내지 않음
    tokio::time::sleep(Duration::from_millis(50)).await;
    assert_eq!(stages.lock().unwrap().last().unwrap(), "closed");
}

#[tokio::test(flavor = "multi_thread")]
async fn reports_stop_after_the_window() {
    let (server, stages) = server(200).await;
    let now = Arc::new(AtomicU64::new(0));
    let clock_now = now.clone();
    let clock: ProgressClock = Arc::new(move || clock_now.load(Ordering::SeqCst) as f64);
    let reporter =
        SetupProgressReporter::with_clock(&server.uri(), "ggp_member", 41, reqwest::Client::new(), clock).unwrap();
    now.store(REPORT_WINDOW_SECONDS as u64 + 1, Ordering::SeqCst);
    reporter.report(payload("ask", "upbit"));
    reporter.finish(payload("closed", "upbit"), Duration::from_secs(1)).await;
    assert!(reporter.is_finished());
    assert!(stages.lock().unwrap().is_empty());
}

#[test]
fn bad_launch_ids_are_refused() {
    for bad in [0, -1] {
        assert_eq!(
            SetupProgressReporter::new("https://api.example", "k", bad, reqwest::Client::new()).err().unwrap(),
            "launch_id must be a positive integer"
        );
    }
}

#[test]
fn works_outside_a_runtime() {
    let reporter = SetupProgressReporter::new("http://127.0.0.1:9", "k", 7, reqwest::Client::new()).unwrap();
    reporter.report(payload("ask", "upbit"));
    reporter.close();
    for _ in 0..500 {
        if reporter.is_finished() {
            break;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    assert!(reporter.is_finished());
}
