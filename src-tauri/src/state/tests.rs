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
    c.slot_mut(KeySlot::BinanceDemo).keys = KeyPair { api_key: "demo".into(), api_secret: "ds".into() };
    let mut r = run("a", "BTCUSDT", "binance");
    r.mode = RunMode::Testnet;
    assert_eq!(c.run_credentials(&r).exchanges["binance"].api_key, "test");
    r.mode = RunMode::Live;
    assert_eq!(c.run_credentials(&r).exchanges["binance"].api_key, "live");
    r.mode = RunMode::Mock;
    assert_eq!(c.run_credentials(&r).exchanges["binance"].api_key, "live", "모의는 실거래 칸");
    let mut f = with_macro("f", json!({"symbol": "BTCUSDT", "exchange": "binance", "leverage": 3}), RunMode::Testnet);
    assert_eq!(f.key_slot(), Some(KeySlot::BinanceDemo));
    assert_eq!(c.run_credentials(&f).exchanges["binance"].api_key, "demo", "선물 연습은 데모 칸");
    f.mode = RunMode::Live;
    assert_eq!(c.run_credentials(&f).exchanges["binance"].api_key, "live");
    let stored = c.storage_values();
    assert_eq!(
        (stored.exchanges["binance_testnet"].api_key.as_str(), stored.exchanges["binance_demo"].api_key.as_str()),
        ("test", "demo")
    );
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
fn support_text_has_environment_but_no_keys() {
    let mut c = core();
    c.member_key = "MEMBERSECRET".into();
    c.slot_mut(KeySlot::Upbit).keys = KeyPair { api_key: "SECRETKEYVALUE".into(), api_secret: "SECRET2".into() };
    let mut r = run("a", "KRW-BTC", "upbit");
    r.mode = RunMode::Live;
    c.runs.push(r);
    let now = chrono::Local::now();
    let text = c.support_text("0.19.0", "macOS 15.6 (arm64)", now);
    assert!(!text.contains("SECRET"), "키·회원 키 값은 넣지 않음: {text}");
    for want in
        ["앱 버전: v0.19.0", "서버 연동 레벨: 19", "OS: macOS 15.6 (arm64)", "공인 IP: 확인 전", "회원 키: 입력됨"]
    {
        assert!(text.contains(want), "{want:?} 없음: {text}");
    }
    assert!(text.contains("https://example.test (기본 서버 아님)"));
    assert!(text.contains("- 업비트 KRW-BTC · 실전 · "), "{text}");
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

fn with_macro(id: &str, m: serde_json::Value, mode: RunMode) -> Run {
    let mut r = Run::new(id.into(), m, id.into(), "web", "https://example.test".into());
    r.mode = mode;
    r
}

#[test]
fn binance_wizard_target_follows_mode_and_market() {
    let spot = json!({"symbol": "BTCUSDT", "exchange": "binance"});
    let futures = json!({"symbol": "BTCUSDT", "exchange": "binance", "position_side": "short"});
    let cases = [
        (spot.clone(), RunMode::Testnet, KeySlot::BinanceTestnet, "testnet", Market::Spot),
        (futures.clone(), RunMode::Testnet, KeySlot::BinanceDemo, "demo", Market::Futures),
        (futures, RunMode::Live, KeySlot::Binance, "live", Market::Futures),
        (spot, RunMode::Live, KeySlot::Binance, "live", Market::Spot),
        (json!({"symbol": "KRW-BTC", "exchange": "upbit"}), RunMode::Live, KeySlot::Upbit, "", Market::Spot),
    ];
    for (m, mode, slot, env, market) in cases {
        let r = with_macro("a", m, mode);
        assert_eq!(r.wizard_target(), Some(WizardTarget { slot, market }), "{mode:?} {env}");
        assert_eq!(r.wizard_target().unwrap().env(), env, "사이트는 칸이 정함");
        assert_eq!(r.key_slot(), Some(slot), "도우미 칸과 실행 키 칸이 같음");
        assert_eq!(r.guide_target(), r.wizard_target(), "실전·연습은 웹 가이드도 같은 칸");
    }
}

#[test]
fn mock_runs_open_no_wizard_but_the_web_guide_shows_the_practice_site() {
    let futures = json!({"symbol": "BTCUSDT", "exchange": "binance", "leverage": 3});
    let r = with_macro("m", futures, RunMode::Mock);
    assert_eq!(r.key_slot(), Some(KeySlot::Binance), "모의가 키를 쓰면 실거래 칸");
    assert_eq!(r.wizard_target(), None, "모의는 키가 필요 없어 도우미로 데려가지 않음");
    let g = r.guide_target().unwrap();
    assert_eq!((g.slot, g.env(), g.market), (KeySlot::BinanceDemo, "demo", Market::Futures));
    assert_eq!(g.env(), r.binance_env(), "웹 가이드의 사이트는 binance_env와 같음");
    let spot = with_macro("s", json!({"symbol": "BTCUSDT", "exchange": "binance"}), RunMode::Mock);
    assert_eq!(spot.guide_target().map(|t| (t.slot, t.env())), Some((KeySlot::BinanceTestnet, "testnet")));
    let up = with_macro("u", json!({"symbol": "KRW-BTC", "exchange": "upbit"}), RunMode::Mock);
    assert_eq!(up.guide_target().map(|t| (t.slot, t.env())), Some((KeySlot::Upbit, "")));
}

#[test]
fn practice_slots_have_a_fixed_site_and_market() {
    let mut c = core();
    let lev = json!({"symbol": "BTCUSDT", "exchange": "binance", "leverage": 3});
    c.runs.push(with_macro("t", lev, RunMode::Testnet));
    let t = c.wizard_target_for_slot(KeySlot::BinanceTestnet);
    assert_eq!((t.env(), t.market), ("testnet", Market::Spot), "선물 실행이 있어도 테스트넷 칸은 현물");
    let t = c.wizard_target_for_slot(KeySlot::BinanceDemo);
    assert_eq!((t.env(), t.market), ("demo", Market::Futures));
    assert_eq!(
        c.wizard_target_for_slot(KeySlot::Bithumb),
        WizardTarget { slot: KeySlot::Bithumb, market: Market::Spot }
    );
    assert_eq!(c.wizard_target_for_slot(KeySlot::Bithumb).env(), "");
}

#[test]
fn the_live_slot_is_checked_in_the_market_of_the_first_run_on_it() {
    let mut c = core();
    assert_eq!(c.probe_market(KeySlot::Binance), Market::Spot, "실행이 없으면 현물");
    let futures = json!({"symbol": "BTCUSDT", "exchange": "binance", "position_side": "short"});
    // 웹 연결로 들어온 실행(모의) — 모의도 키가 있으면 실거래 칸을 써서 바로 따름.
    c.runs.push(with_macro("web", futures, RunMode::Mock));
    assert_eq!(c.probe_market(KeySlot::Binance), Market::Futures);
    c.runs.push(with_macro("spot", json!({"symbol": "ETHUSDT", "exchange": "binance"}), RunMode::Live));
    assert_eq!(c.probe_market(KeySlot::Binance), Market::Futures, "목록 맨 앞 실행을 따름");
    assert_eq!(c.wizard_target_for_slot(KeySlot::Binance).market, Market::Futures);
    // 앞 실행을 연습으로 바꾸면 그 실행은 데모 칸을 써서 다음 실행의 시장으로.
    c.runs[0].mode = RunMode::Testnet;
    assert_eq!(c.probe_market(KeySlot::Binance), Market::Spot);
    assert_eq!(c.probe_market(KeySlot::BinanceDemo), Market::Futures);
    // 열린 도우미가 이 칸을 보면 도우미의 시장.
    c.wizard = Some(wizard(KeySlot::Binance, Market::Futures));
    assert_eq!(c.probe_market(KeySlot::Binance), Market::Futures);
    c.wizard = None;
    assert_eq!(c.probe_market(KeySlot::Binance), Market::Spot);
}

#[test]
fn binance_slot_checks_use_the_slot_market() {
    let mut c = core();
    c.slot_mut(KeySlot::BinanceTestnet).keys = KeyPair { api_key: "k".into(), api_secret: "s".into() };
    c.slot_mut(KeySlot::BinanceDemo).keys = KeyPair { api_key: "k".into(), api_secret: "s".into() };
    let (spot, mode, _, spot_fp) = c.slot_check_snapshot(KeySlot::BinanceTestnet);
    assert_eq!((spot["leverage"].clone(), mode), (json!(1), RunMode::Testnet));
    let (futures, mode, _, futures_fp) = c.slot_check_snapshot(KeySlot::BinanceDemo);
    assert_eq!(market_of("binance", macro_side(&futures), macro_leverage(&futures).unwrap()), Market::Futures);
    assert_eq!(mode, RunMode::Testnet);
    assert_ne!(spot_fp, futures_fp, "같은 키라도 시장이 다르면 다른 확인");
    c.slot_mut(KeySlot::Binance).keys = KeyPair { api_key: "k".into(), api_secret: "s".into() };
    let before = c.slot_check_snapshot(KeySlot::Binance).3;
    c.runs.push(with_macro("f", json!({"symbol": "BTCUSDT", "exchange": "binance", "leverage": 2}), RunMode::Live));
    assert_ne!(
        c.slot_check_snapshot(KeySlot::Binance).3,
        before,
        "실행이 늘어 시장이 바뀌면 지난 확인은 지금 키의 결과가 아님"
    );
}

fn wizard(slot: KeySlot, market: Market) -> Wizard {
    Wizard {
        target: WizardTarget { slot, market },
        stage: "perm".into(),
        path: "new".into(),
        got_access: false,
        got_secret: false,
        advance_on_return: String::new(),
        clip_seen: String::new(),
        generation: 1,
    }
}

fn reporter() -> Arc<SetupProgressReporter> {
    Arc::new(SetupProgressReporter::new("http://127.0.0.1:9", "k", 7, reqwest::Client::new()).unwrap())
}

#[test]
fn progress_carries_the_binance_site_but_never_for_domestic_runs() {
    let mut c = core();
    let mut demo =
        with_macro("demo", json!({"symbol": "BTCUSDT", "exchange": "binance", "leverage": 5}), RunMode::Mock);
    demo.reporter = Some(reporter());
    let mut live = with_macro("live", json!({"symbol": "ETHUSDT", "exchange": "binance"}), RunMode::Live);
    live.reporter = Some(reporter());
    let mut upbit = with_macro("up", json!({"symbol": "KRW-BTC", "exchange": "upbit"}), RunMode::Live);
    upbit.reporter = Some(reporter());
    c.runs = vec![demo, live, upbit];
    let sent: Vec<Value> = c.progress_payloads().into_iter().map(|(_, p)| p).collect();
    assert_eq!(sent.len(), 3);
    assert_eq!((sent[0]["env"].clone(), sent[0]["stage"].clone()), (json!("demo"), json!("ready")));
    assert_eq!(sent[1]["env"], "live");
    assert!(sent[2].get("env").is_none(), "국내에는 env를 싣지 않음(서버가 422로 거절)");
    for r in &c.runs {
        if let Some(rep) = &r.reporter {
            rep.close();
        }
    }
}

#[test]
fn the_wizard_is_named_after_the_binance_site() {
    let mut c = core();
    c.wizard = Some(wizard(KeySlot::BinanceDemo, Market::Futures));
    let w = serde_json::to_value(c.render_wizard()).unwrap();
    assert_eq!(
        (&w["exchange"], &w["slot"], &w["env"], &w["market"], &w["label"], &w["homeLabel"]),
        (
            &json!("binance"),
            &json!("binance_demo"),
            &json!("demo"),
            &json!("futures"),
            &json!("바이낸스 데모"),
            &json!("데모")
        )
    );
    assert_eq!(w["notes"], json!({"auth": "", "ip": "", "order": ""}));
    assert_eq!(w["keysReady"], false);
    let settings = serde_json::to_value(c.render("0.18.2")).unwrap();
    assert!(settings["settings"]["slots"].as_array().unwrap().iter().all(|s| s["wizardAvailable"] == true));
}

#[test]
fn settings_list_every_slot_with_the_demo_slot_after_the_testnet_slot() {
    let c = core();
    let dto = serde_json::to_value(c.render("0.18.2")).unwrap();
    let slots: Vec<_> = dto["settings"]["slots"].as_array().unwrap().iter().map(|s| s["slot"].clone()).collect();
    assert_eq!(
        slots,
        [json!("binance"), json!("binance_testnet"), json!("binance_demo"), json!("upbit"), json!("bithumb")]
    );
    let demo = &dto["settings"]["slots"][2];
    assert_eq!((&demo["label"], &demo["exchange"]), (&json!("바이낸스 데모"), &json!("binance")));
}

#[test]
fn progress_follows_only_the_wizard_on_the_slot_of_that_site() {
    let mut c = core();
    let mut r = with_macro("m", json!({"symbol": "BTCUSDT", "exchange": "binance", "leverage": 5}), RunMode::Mock);
    r.reporter = Some(reporter());
    c.runs = vec![r];
    c.wizard = Some(wizard(KeySlot::BinanceTestnet, Market::Spot));
    let sent = c.progress_payloads();
    assert_eq!(sent[0].1["stage"], "ready", "현물 테스트넷 도우미는 선물 데모 가이드와 무관");
    c.wizard = Some(wizard(KeySlot::BinanceDemo, Market::Futures));
    let sent = c.progress_payloads();
    assert_eq!((sent[0].1["stage"].clone(), sent[0].1["env"].clone()), (json!("perm"), json!("demo")));
    c.runs[0].reporter.take().unwrap().close();
}

#[test]
fn activity_fields_are_clipped() {
    let mut c = core();
    c.runs.push(run("a", "KRW-BTC", "upbit"));
    let long = "가".repeat(ACTIVITY_RAW_CHARS + 50);
    c.push_activity("a", "error", &long, &long, &long, true, "danger");
    let a = c.run("a").unwrap().activity.last().unwrap();
    assert_eq!(a.title.chars().count(), ACTIVITY_TITLE_CHARS + 1, "한도 + '…'");
    assert_eq!(a.detail.chars().count(), ACTIVITY_DETAIL_CHARS + 1);
    assert_eq!(a.raw.chars().count(), ACTIVITY_RAW_CHARS + 1);
    assert!(a.raw.ends_with('…'));
    assert_eq!(clip_chars("짧음", 10), "짧음");
}

#[test]
fn activity_copy_keeps_recent_lines_within_limit() {
    let mut c = core();
    c.runs.push(run("a", "KRW-BTC", "upbit"));
    for i in 0..ACTIVITY_LIMIT + 10 {
        c.push_activity("a", "info", &format!("줄 {i}"), "", &"x".repeat(150), true, "muted");
    }
    let items = &c.run("a").unwrap().activity;
    assert_eq!(items.len(), ACTIVITY_LIMIT, "메모리에는 최근 1,000줄만");
    let text = activity_copy_text("[머리말]", items, 5_000);
    assert!(text.chars().count() <= 5_000, "한도 안: {}", text.chars().count());
    assert!(text.starts_with("[머리말]\n(앞의 "), "뺀 수를 적음");
    assert!(text.contains(&format!("줄 {}", ACTIVITY_LIMIT + 9)), "가장 최근 줄은 남음");
    assert!(!text.contains("줄 10\n"), "오래된 줄은 빠짐");
    let all = activity_copy_text("[머리말]", &items[..3], ACTIVITY_COPY_CHARS);
    assert!(!all.contains("뺐어요"), "한도 안이면 다 담음");
    // 날짜까지 붙음(MM-DD HH:MM:SS)
    let line = all.lines().nth(1).unwrap();
    assert!(line.as_bytes()[2] == b'-' && line.as_bytes()[5] == b' ', "{line}");
}

#[test]
fn removing_or_closing_hands_the_guide_a_closed_stage() {
    let mut r = with_macro("demo", json!({"symbol": "BTCUSDT", "exchange": "binance", "leverage": 5}), RunMode::Mock);
    assert!(r.take_closing_report().is_none(), "웹으로 들어오지 않은 실행은 보낼 것이 없음");
    r.reporter = Some(reporter());
    let (rep, payload) = r.take_closing_report().unwrap();
    assert!(r.reporter.is_none(), "한 번만 보냄");
    let p = payload.unwrap();
    assert_eq!((&p["stage"], &p["exchange"], &p["env"]), (&json!("closed"), &json!("binance"), &json!("demo")));
    rep.close();
    // 국내에는 env를 싣지 않음(앱 종료 때와 같은 모양).
    let mut up = with_macro("up", json!({"symbol": "KRW-BTC", "exchange": "upbit"}), RunMode::Live);
    up.reporter = Some(reporter());
    let (rep, payload) = up.take_closing_report().unwrap();
    assert!(payload.unwrap().get("env").is_none());
    rep.close();
}

#[test]
fn the_web_run_page_needs_a_session_on_the_default_server() {
    let m = json!({"symbol": "BTCUSDT", "exchange": "binance"});
    let mut r = Run::new("a".into(), m, "a".into(), "file", runner_core::types::DEFAULT_SERVER_BASE.into());
    assert_eq!(r.web_run_url(), None, "시작 전에는 세션이 없음");
    r.data = Some(RunData::started(1, Some(4821)));
    assert_eq!(r.web_run_url().as_deref(), Some("https://gg-parrot.vercel.app/agents?session=4821"));
    let c = core();
    assert_eq!(serde_json::to_value(c.render_run(&r)).unwrap()["webRunPage"], true);
    r.data = Some(RunData::started(1, None));
    assert_eq!(r.web_run_url(), None);
    // 다른 서버(개발용 등)에서 만든 세션은 껄무새 웹에 없음.
    let mut other = run("b", "BTCUSDT", "binance");
    other.data = Some(RunData::started(1, Some(7)));
    assert_eq!(other.web_run_url(), None);
}

#[test]
fn run_status_carries_fees_day_change_and_macro_origin_for_the_screen() {
    let mut d = RunData::started(1, Some(1));
    d.macro_origin = "원본 파일(서명 확인)".into();
    d.snapshot = json!({"realized_pnl": 5.0, "fees_paid": 0.105, "day_change": -0.0123});
    let v = serde_json::to_value(render_run_data(&d, Side::Long)).unwrap();
    assert_eq!(v["realized"], 5.0, "서버 값 그대로");
    assert_eq!(v["fees"], 0.105);
    assert!((v["dayChangePct"].as_f64().unwrap() + 1.23).abs() < 1e-9);
    assert_eq!(v["macroOrigin"], "원본 파일(서명 확인)");
    // 모르는 수수료·등락률은 null.
    d.snapshot = json!({"realized_pnl": 5.0, "fees_paid": null, "day_change": null});
    let v = serde_json::to_value(render_run_data(&d, Side::Long)).unwrap();
    assert!(v["fees"].is_null() && v["dayChangePct"].is_null());
}
