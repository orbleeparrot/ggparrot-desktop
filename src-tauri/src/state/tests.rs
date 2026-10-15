use super::render::*;
use super::*;
use serde_json::json;

fn core() -> Core {
    let mut c = Core::new("https://example.test".into(), true, PathBuf::from("/tmp/x"));
    c.member_key = "m".into();
    c
}

fn run(id: &str, sym: &str, ex: &str) -> Run {
    Run::new(id.into(), json!({"symbol": sym, "exchange": ex}), id.into(), "file", "https://example.test".into())
}

#[test]
fn same_exchange_and_symbol_cannot_run_twice() {
    let mut c = core();
    c.runs.push(run("a", "BTCUSDT", "binance"));
    c.runs.push(run("b", "BTCUSDT", "binance"));
    c.runs.push(run("d", "ETHUSDT", "binance"));
    c.runs.push(run("e", "KRW-BTC", "upbit"));
    assert!(c.conflicting_run(&c.runs[1]).is_none(), "아무도 실행 중이 아니면 막지 않는다");
    c.runs[0].start_pending = true; // 시작 중도 실행 중으로 봄
    assert_eq!(c.conflicting_run(&c.runs[1]).map(|r| r.id.as_str()), Some("a"));
    assert!(c.conflicting_run(&c.runs[2]).is_none(), "다른 종목");
    assert!(c.conflicting_run(&c.runs[3]).is_none(), "다른 거래소");
    let (ok, blocker, _) = c.readiness(&c.runs[1]);
    assert!(!ok);
    assert!(blocker.contains("바이낸스 BTCUSDT"));
}

#[test]
fn starting_and_checking_runs_block_quit_and_update() {
    let mut c = core();
    c.runs.push(run("a", "BTCUSDT", "binance"));
    c.runs.push(run("b", "ETHUSDT", "binance"));
    assert_eq!(c.busy_count(), 0);
    c.runs[0].start_pending = true;
    c.runs[1].checking = true;
    assert_eq!(c.running_count(), 0, "봇은 아직 없다");
    assert_eq!(c.busy_count(), 2, "시작 중 · 확인 중도 끄면 안 된다");
    c.runs[0].cancel_requested = Some(StopMode::StopOnly);
    let dto = c.render("0.18.2");
    let phase = serde_json::to_value(&dto).unwrap()["runs"][0]["phase"].clone();
    assert_eq!(phase, "stopping", "시작 중 종료를 누르면 종료 중으로 보인다");
}

#[test]
fn run_uses_the_key_slot_for_its_mode() {
    let mut c = core();
    c.slot_mut(KeySlot::Binance).keys = KeyPair { api_key: "live".into(), api_secret: "ls".into() };
    c.slot_mut(KeySlot::BinanceTestnet).keys = KeyPair { api_key: "test".into(), api_secret: "ts".into() };
    let mut r = run("a", "BTCUSDT", "binance");
    r.mode = RunMode::Testnet;
    assert_eq!(c.run_credentials(&r).exchanges["binance"].api_key, "test");
    r.mode = RunMode::Live;
    assert_eq!(c.run_credentials(&r).exchanges["binance"].api_key, "live");
    assert_eq!(c.storage_values().exchanges["binance_testnet"].api_key, "test");
}

#[test]
fn readiness_points_to_settings() {
    let mut c = core();
    let mut r = run("a", "KRW-BTC", "upbit");
    r.mode = RunMode::Live;
    let (ok, blocker, needs) = c.readiness(&r);
    assert!(!ok);
    assert!(blocker.contains("업비트 거래소 키"));
    assert!(matches!(needs, Some(NeedsDto::Key { slot: KeySlot::Upbit })));
    r.mode = RunMode::Mock;
    assert!(c.readiness(&r).0, "모의는 키 없이 시작한다");
    c.member_key.clear();
    assert!(matches!(c.readiness(&r).2, Some(NeedsDto::Member)));
}

#[test]
fn saved_runs_carry_no_keys() {
    let mut c = core();
    c.member_key = "MEMBERSECRET".into();
    c.slot_mut(KeySlot::Upbit).keys = KeyPair { api_key: "SECRETKEYVALUE".into(), api_secret: "SECRET2".into() };
    let mut r = run("a", "KRW-BTC", "upbit");
    r.mode = RunMode::Live;
    let text = serde_json::to_string(&r.saved()).unwrap();
    assert!(!text.contains("SECRET"));
    let back = Run::from_saved(serde_json::from_str(&text).unwrap(), "https://example.test");
    assert_eq!(back.mode, RunMode::Live);
    assert_eq!(back.symbol(), "KRW-BTC");
}

#[test]
fn testnet_mode_is_dropped_for_domestic_saved_runs() {
    let mut s = run("a", "KRW-BTC", "upbit").saved();
    s.mode = RunMode::Testnet;
    assert_eq!(Run::from_saved(s, "x").mode, RunMode::Mock);
}

#[test]
fn edited_capital_shows_as_modified_and_unsupported_macros_cannot_start() {
    let mut c = core();
    let mut r = run("a", "BTCUSDT", "binance");
    r.macro_ =
        json!({"symbol": "BTCUSDT", "exchange": "binance", "rule_type": "F", "params": {"initial_capital": 800}});
    r.sig = Some(json!({"v": 4}));
    r.original_capital = Some(500.0);
    c.runs.push(r);
    let mut dca = run("b", "ETHUSDT", "binance");
    dca.macro_ = json!({"symbol": "ETHUSDT", "exchange": "binance", "rule_type": "C"});
    c.runs.push(dca);
    let v = serde_json::to_value(c.render("0.18.2")).unwrap();
    let m = &v["runs"][0]["macro"];
    assert_eq!(m["signature"], "modified", "서명된 파일의 금액을 바꾸면 수정됨");
    assert_eq!((m["capital"].as_f64(), m["capitalOriginal"].as_f64()), (Some(800.0), Some(500.0)));
    assert_eq!(m["strategy"], "RSI 조건 매매");
    assert_eq!(m["orderCap"].as_f64(), Some(runner_core::types::order_cap("USDT")));
    assert_eq!(m["capitalLocked"], "");
    let blocked = &v["runs"][1]["readiness"];
    assert_eq!(blocked["canStart"], false);
    assert!(blocked["blocker"].as_str().unwrap().contains("정기 분할매수"));
}

fn running(c: &mut Core, i: usize) -> &mut RunData {
    let sym = c.runs[i].symbol();
    c.runs[i].bot = Some(runner_core::bot::BotHandle::idle_for_tests(&sym));
    let mut d = RunData::started(now_ms(), Some(1));
    d.updated_at = now_ms();
    c.runs[i].data = Some(d);
    c.runs[i].data.as_mut().unwrap()
}

fn header(c: &Core) -> (String, String) {
    let v = serde_json::to_value(c.render("0.18.3")).unwrap();
    (v["status"]["text"].as_str().unwrap().into(), v["status"]["tone"].as_str().unwrap().into())
}

#[tokio::test]
async fn degraded_runs_show_on_the_card_and_the_header() {
    let mut c = core();
    c.runs.push(run("a", "BTCUSDT", "binance"));
    c.runs.push(run("b", "ETHUSDT", "binance"));
    c.runs.iter_mut().for_each(|r| r.mode = RunMode::Live);
    running(&mut c, 0);
    running(&mut c, 1);
    assert_eq!(c.runs[0].degraded(), None);
    assert_eq!(header(&c), ("실행 중 2개".into(), "ok".into()));

    // 서버만 끊김 — 잠깐 불안정(warn).
    let d = c.runs[1].data.as_mut().unwrap();
    d.health.online = false;
    d.health.offline_since = now_ms() as u64;
    assert_eq!(c.runs[1].degraded(), Some(("서버 재시도 중".into(), "warn")));
    assert_eq!(header(&c), ("실행 중 2개 · 연결 다시 시도하는 중".into(), "warn".into()));
    // 5분 넘게 끊김 — 확인 필요(danger).
    c.runs[1].data.as_mut().unwrap().health.offline_since = (now_ms() - 6 * 60_000) as u64;
    assert_eq!(c.runs[1].degraded().unwrap().1, "danger");

    // 포지션을 든 채 시세가 끊기면 손절을 못 보는 중 — 서버 끊김보다 앞섬.
    let d = c.runs[0].data.as_mut().unwrap();
    d.snapshot = json!({"in_position": true});
    d.health.price_stale_since = now_ms() as u64;
    assert_eq!(c.runs[0].degraded(), Some(("시세 끊김 · 손절 확인 멈춤".into(), "danger")));
    let v = serde_json::to_value(c.render("0.18.3")).unwrap();
    assert_eq!(v["runs"][0]["status"]["text"], "실행 중 · 시세 끊김 · 손절 확인 멈춤");
    assert!(v["runs"][0]["run"]["priceStaleSince"].as_i64().unwrap() > 0);
    assert_eq!(header(&c), ("실행 중 2개 · 확인 필요 2개".into(), "danger".into()));

    // 회원 키 거절·정리 실패도 확인 필요.
    let d = c.runs[0].data.as_mut().unwrap();
    d.health = runner_core::bot::RunHealth { online: true, close_failures: 2, ..Default::default() };
    assert_eq!(c.runs[0].degraded(), Some(("정리 주문 실패 2회".into(), "danger")));
}

#[tokio::test]
async fn runs_that_ended_in_error_stay_visible_until_restarted() {
    let mut c = core();
    c.runs.push(run("a", "BTCUSDT", "binance"));
    c.runs[0].mode = RunMode::Live;
    let mut d = RunData::started(now_ms(), Some(1));
    d.finished = Some(("error".into(), "청산 실패 — 포지션 남음".into(), now_ms()));
    d.snapshot = json!({"in_position": true});
    c.runs[0].data = Some(d);
    assert!(c.runs[0].needs_attention());
    assert_eq!(header(&c), ("확인 필요 1개".into(), "danger".into()));
    let why = crate::shared::leftover_warning(&c.runs[0]).unwrap();
    assert!(why.contains("포지션을 남긴 채"), "{why}");

    c.runs[0].data.as_mut().unwrap().finished = Some(("stopped".into(), "포지션 없이 종료".into(), now_ms()));
    c.runs[0].data.as_mut().unwrap().snapshot = json!({"in_position": false});
    assert!(!c.runs[0].needs_attention());
    assert!(crate::shared::leftover_warning(&c.runs[0]).is_none());
}

#[tokio::test]
async fn a_run_interrupted_by_an_app_crash_is_flagged_on_the_next_launch() {
    let mut c = core();
    c.runs.push(run("a", "BTCUSDT", "binance"));
    c.runs[0].mode = RunMode::Live;
    running(&mut c, 0);
    let saved = c.runs[0].saved();
    assert!(saved.active_since.is_some(), "실행 중에 저장하면 표시가 남는다");
    let text = serde_json::to_string(&saved).unwrap();
    let back: SavedRun = serde_json::from_str(&text).unwrap();
    let restored = Run::from_saved(back, "https://example.test");
    let (st, note, _) = restored.data.as_ref().unwrap().finished.clone().unwrap();
    assert_eq!((st.as_str(), note.as_str()), ("error", INTERRUPTED_NOTE));
    assert!(restored.needs_attention());
    assert!(crate::shared::leftover_warning(&restored).unwrap().contains("정상적으로 끝나지 않아"));

    c.runs[0].bot = None;
    assert!(c.runs[0].saved().active_since.is_none(), "끝난 실행은 표시를 지운다");
}

#[tokio::test]
async fn mock_runs_are_flagged_softly_because_no_real_account_is_at_risk() {
    let mut c = core();
    c.runs.push(run("a", "BTCUSDT", "binance"));
    c.runs[0].mode = RunMode::Mock;
    let d = running(&mut c, 0);
    d.snapshot = json!({"in_position": true});
    d.health.price_stale_since = now_ms() as u64;
    assert_eq!(c.runs[0].degraded(), Some(("시세 끊김 · 손절 확인 멈춤".into(), "warn")));
    assert!(!c.runs[0].needs_attention());
    c.runs[0].bot = None;
    c.runs[0].data.as_mut().unwrap().finished = Some(("error".into(), "x".into(), now_ms()));
    assert!(!c.runs[0].needs_attention(), "모의가 오류로 끝나도 빨간 '확인 필요'로 올리지 않는다");
    assert_eq!(header(&c).1, "muted");
}

#[test]
#[ignore = "측정용 — cargo test -p ggparrot-desktop render_cost -- --ignored --nocapture"]
fn render_cost() {
    let mut c = core();
    for i in 0..10 {
        let mut r = run(&format!("r{i}"), &format!("S{i}USDT"), "binance");
        for k in 0..200 {
            r.activity.push(ActivityDto {
                id: k,
                run_id: r.id.clone(),
                at: 0,
                kind: "signal".into(),
                title: "매수 신호".into(),
                detail: "RSI 30 이탈 반등".into(),
                raw: "x".repeat(80),
                dev: false,
                tone: "muted".into(),
            });
        }
        c.runs.push(r);
    }
    for i in 0..300 {
        c.library.push(runner_core::library::LibraryEntry {
            file_name: format!("binance_BTCUSDT_rsi_long_{i}.ggm.json"),
            valid: true,
            error: String::new(),
            title: format!("macro {i}"),
            exchange_name: "binance".into(),
            exchange_label: "바이낸스".into(),
            symbol: "BTCUSDT".into(),
            side: "long".into(),
            strategy: "RSI 조건 매매".into(),
            unsupported: String::new(),
            summary: "RSI 14가 30 아래로 내려갔다가 다시 올라오면 사고, 70을 넘으면 팝니다.".into(),
            signed: true,
            modified_ms: 0,
        });
    }
    let t = std::time::Instant::now();
    let mut bytes = 0;
    for _ in 0..100 {
        bytes = serde_json::to_string(&c.render("0.18.7")).unwrap().len();
    }
    let full = t.elapsed() / 100;
    let t = std::time::Instant::now();
    let mut slim = 0;
    for _ in 0..100 {
        slim = serde_json::to_string(&c.render_with("0.18.7", false)).unwrap().len();
    }
    println!("전체 {full:?}/회 {bytes} bytes · 목록 뺀 판 {:?}/회 {slim} bytes", t.elapsed() / 100);
}

#[test]
fn the_macro_folder_list_is_left_out_unless_asked() {
    let c = core();
    let full = serde_json::to_value(c.render("0.18.7")).unwrap();
    let slim = serde_json::to_value(c.render_with("0.18.7", false)).unwrap();
    assert!(full.get("library").is_some(), "get_state 는 늘 목록을 싣는다");
    assert!(slim.get("library").is_none(), "바뀌지 않았으면 싣지 않는다(화면은 지난 목록을 쓴다)");
    assert_eq!(full["runs"], slim["runs"]);
}
