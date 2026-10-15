use super::*;

const MACRO: &str = r#"{"symbol":"BTCUSDT","exchange":"binance","rule_type":"F","position_side":"long","human_summary":"RSI","_sig":{"v":1}}"#;

#[test]
fn scan_lists_only_ggm_files_and_counts_the_rest() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join("rsi.ggm.json"), MACRO).unwrap();
    fs::write(dir.path().join("broken.ggm.json"), "{not json").unwrap();
    fs::write(dir.path().join("package.json"), "{}").unwrap();
    fs::write(dir.path().join("notes.txt"), "hi").unwrap();
    fs::write(dir.path().join(".hidden.ggm.json"), MACRO).unwrap();
    let scan = scan(dir.path()).unwrap();
    assert_eq!(scan.files.len(), 2);
    assert_eq!(scan.skipped, 1, "package.json 은 세기만");
    let ok = scan.files.iter().find(|e| e.file_name == "rsi.ggm.json").unwrap();
    assert!(ok.valid && ok.signed && ok.unsupported.is_empty());
    assert_eq!((ok.title.as_str(), ok.symbol.as_str(), ok.strategy.as_str()), ("rsi", "BTCUSDT", "RSI 조건 매매"));
    assert_eq!(ok.exchange_label, "바이낸스");
    let bad = scan.files.iter().find(|e| e.file_name == "broken.ggm.json").unwrap();
    assert!(!bad.valid);
    assert_eq!(bad.error, NOT_A_MACRO);
}

#[test]
fn content_needs_a_rule_type_and_server_refusals_are_flagged() {
    assert!(parse_macro(r#"{"symbol":"BTCUSDT"}"#).is_err(), "유형 없는 JSON 은 매크로가 아니다");
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join("dca.ggm.json"), r#"{"symbol":"BTCUSDT","rule_type":"C"}"#).unwrap();
    fs::write(
        dir.path().join("multi.ggm.json"),
        r#"{"symbol":"BTCUSDT","rule_type":"A","symbols":["BTCUSDT","ETHUSDT"]}"#,
    )
    .unwrap();
    fs::write(dir.path().join("odd.ggm.json"), r#"{"symbol":"BTCUSDT","rule_type":"A","exchange":"kraken"}"#).unwrap();
    let scan = scan(dir.path()).unwrap();
    assert_eq!(scan.files.len(), 3);
    for f in &scan.files {
        assert!(f.valid, "{}", f.file_name);
        assert!(!f.unsupported.is_empty(), "{} 는 서버가 거절한다", f.file_name);
    }
}

#[test]
fn unchanged_files_are_not_read_again() {
    let dir = tempfile::tempdir().unwrap();
    let p = dir.path().join("rsi.ggm.json");
    fs::write(&p, MACRO).unwrap();
    let mut cache = LibraryCache::default();
    assert!(scan_cached(dir.path(), &mut cache).unwrap().files[0].valid);
    // 캐시가 쓰였는지: 캐시 값을 바꿔 두면 파일이 그대로인 한 그 값이 나옴.
    cache.0.get_mut("rsi.ggm.json").unwrap().2.summary = "cached".into();
    assert_eq!(scan_cached(dir.path(), &mut cache).unwrap().files[0].summary, "cached");
    // 파일이 바뀌면(크기) 다시 읽음.
    fs::write(&p, MACRO.replace("RSI", "RSI 14")).unwrap();
    assert_eq!(scan_cached(dir.path(), &mut cache).unwrap().files[0].summary, "RSI 14");
    // 사라진 파일은 캐시에서도 빠짐.
    fs::remove_file(&p).unwrap();
    assert!(scan_cached(dir.path(), &mut cache).unwrap().files.is_empty());
    assert!(cache.0.is_empty());
}

#[test]
fn too_big_files_are_not_read() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join("big.ggm.json"), " ".repeat(MAX_FILE_BYTES as usize + 1)).unwrap();
    let scan = scan(dir.path()).unwrap();
    assert!(!scan.files[0].valid);
    assert!(scan.files[0].error.contains("256KB"));
}

#[test]
fn only_the_newest_files_are_listed() {
    let dir = tempfile::tempdir().unwrap();
    for i in 0..(MAX_LISTED + 3) {
        fs::write(dir.path().join(format!("m{i:04}.ggm.json")), MACRO).unwrap();
    }
    let scan = scan(dir.path()).unwrap();
    assert_eq!(scan.files.len(), MAX_LISTED);
    assert_eq!(scan.truncated, 3);
}

#[test]
fn import_renames_json_to_ggm_and_never_overwrites() {
    let lib = tempfile::tempdir().unwrap();
    let src = tempfile::tempdir().unwrap();
    let plain = src.path().join("rsi.json");
    fs::write(&plain, MACRO).unwrap();
    assert_eq!(import(lib.path(), &plain).unwrap(), "rsi.ggm.json");
    assert_eq!(import(lib.path(), &plain).unwrap(), "rsi (2).ggm.json");
    assert!(plain.exists(), "원본은 그대로");
    // 이미 폴더 안에 있는 파일은 복사하지 않음
    assert_eq!(import(lib.path(), &lib.path().join("rsi.ggm.json")).unwrap(), "rsi.ggm.json");
    assert_eq!(scan(lib.path()).unwrap().files.len(), 2);
}

#[test]
fn import_rejects_non_macros() {
    let lib = tempfile::tempdir().unwrap();
    let src = tempfile::tempdir().unwrap();
    let bad = src.path().join("x.json");
    fs::write(&bad, r#"{"hello":1}"#).unwrap();
    assert_eq!(import(lib.path(), &bad).unwrap_err(), NOT_A_MACRO);
    let txt = src.path().join("x.txt");
    fs::write(&txt, MACRO).unwrap();
    assert!(import(lib.path(), &txt).is_err());
    assert!(scan(lib.path()).unwrap().files.is_empty());
}

#[test]
fn read_refuses_paths_and_non_ggm_names() {
    let lib = tempfile::tempdir().unwrap();
    fs::write(lib.path().join("a.ggm.json"), MACRO).unwrap();
    fs::write(lib.path().join("b.json"), MACRO).unwrap();
    assert!(read(lib.path(), "a.ggm.json").is_ok());
    assert!(read(lib.path(), "b.json").is_err(), "목록에 없는 이름은 읽지 않는다");
    assert!(read(lib.path(), "../a.ggm.json").is_err());
    assert!(read(lib.path(), "/etc/passwd").is_err());
}

fn at(rfc3339: &str) -> i64 {
    chrono::DateTime::parse_from_rfc3339(rfc3339).unwrap().timestamp_millis()
}

fn json(text: &str) -> Value {
    serde_json::from_str(text).unwrap()
}

#[test]
fn file_names_follow_the_web_download_rule() {
    // 웹 frontend/tests/macroFileName.test.js 와 같은 기대값.
    let now = at("2026-10-13T06:00:00Z"); // KST 10/13 15:00
    let name = |t: &str| macro_file_name(&json(t), now);
    assert_eq!(
        name(r#"{"exchange":"bithumb","symbol":"KRW-BTR","rule_type":"F","position_side":"long"}"#),
        "bithumb_KRW-BTR_rsi_long_20261013.ggm.json"
    );
    assert_eq!(
        name(r#"{"symbol":"BTCUSDT","rule_type":"A","position_side":"short"}"#),
        "binance_BTCUSDT_tp-sl-reentry_short_20261013.ggm.json"
    );
    assert_eq!(
        name(r#"{"exchange":"upbit","symbols":["KRW-XRP","KRW-ETH","KRW-SOL"],"rule_type":"D"}"#),
        "upbit_KRW-XRP-plus2_grid_long_20261013.ggm.json"
    );
    assert_eq!(
        macro_file_name(&json(r#"{"symbol":"BTCUSDT","rule_type":"C"}"#), at("2026-10-13T15:30:00Z")),
        "binance_BTCUSDT_dca_long_20261014.ggm.json",
        "KST 자정을 넘기면 다음 날짜"
    );
}

#[test]
fn file_names_keep_only_safe_ascii() {
    let now = at("2026-10-13T06:00:00Z");
    let name = macro_file_name(
        &json(r#"{"exchange":"모름","symbol":"비트/코인:*","rule_type":"Z","position_side":"long"}"#),
        now,
    );
    assert_eq!(name, "binance_macro_z_long_20261013.ggm.json");
    let odd = macro_file_name(
        &json(r#"{"exchange":"UPBIT","symbol":"../..\\KRW-BTC <>|?\u0000","rule_type":"","position_side":"SHORT"}"#),
        now,
    );
    assert_eq!(odd, "upbit_KRW-BTC_custom_long_20261013.ggm.json");
    let long = macro_file_name(&json(&format!(r#"{{"symbol":"{}","rule_type":"F"}}"#, "A".repeat(500))), now);
    assert!(long.len() < 120, "이상하게 긴 종목은 잘림: {long}");
    for n in [name, odd, long] {
        assert!(n.chars().all(|c| c.is_ascii_alphanumeric() || "._-".contains(c)), "{n}");
        assert!(is_macro_file(&n));
    }
}

#[test]
fn saved_macros_read_back_as_valid_library_entries() {
    let lib = tempfile::tempdir().unwrap();
    let m = json(r#"{"symbol":"KRW-BTC","exchange":"upbit","rule_type":"F","position_side":"long","leverage":1}"#);
    let saved = save_macro(lib.path(), &m, at("2026-10-13T06:00:00Z")).unwrap();
    assert_eq!(saved, SavedMacro { file_name: "upbit_KRW-BTC_rsi_long_20261013.ggm.json".into(), reused: false });
    assert_eq!(read(lib.path(), &saved.file_name).unwrap(), m, "파일 내용은 매크로 JSON 그대로");
    let scan = scan(lib.path()).unwrap();
    let e = &scan.files[0];
    assert!(e.valid && e.unsupported.is_empty(), "{e:?}");
    assert_eq!((e.symbol.as_str(), e.exchange_label.as_str()), ("KRW-BTC", "업비트"));
}

#[test]
fn saving_the_same_macro_again_reuses_the_file() {
    let lib = tempfile::tempdir().unwrap();
    let now = at("2026-10-13T06:00:00Z");
    let m = json(
        r#"{"symbol":"BTCUSDT","exchange":"binance","rule_type":"F","position_side":"long","rsi":{"len":14,"lo":30}}"#,
    );
    let first = save_macro(lib.path(), &m, now).unwrap();
    // 키 순서가 달라도 같은 매크로.
    let reordered = json(
        r#"{"rsi":{"lo":30,"len":14},"position_side":"long","rule_type":"F","exchange":"binance","symbol":"BTCUSDT"}"#,
    );
    let again = save_macro(lib.path(), &reordered, now + 86_400_000).unwrap();
    assert_eq!(again, SavedMacro { file_name: first.file_name.clone(), reused: true });
    assert_eq!(scan(lib.path()).unwrap().files.len(), 1);
    // 웹에서 내려받은 파일(서명·설명이 붙음)도 같은 매크로로 봄 — 이름이 달라도.
    let lib2 = tempfile::tempdir().unwrap();
    let mut downloaded = m.clone();
    downloaded["_sig"] = json(r#"{"v":1}"#);
    downloaded["human_summary"] = "RSI 30 아래에서 사요".into();
    fs::write(lib2.path().join("내가 받은 파일.ggm.json"), serde_json::to_string_pretty(&downloaded).unwrap()).unwrap();
    let linked = save_macro(lib2.path(), &m, now).unwrap();
    assert_eq!(linked, SavedMacro { file_name: "내가 받은 파일.ggm.json".into(), reused: true });
    // 값이 하나라도 다르면 새 파일.
    let mut other = m.clone();
    other["rsi"]["lo"] = 25.into();
    assert!(!save_macro(lib2.path(), &other, now).unwrap().reused);
    assert_eq!(scan(lib2.path()).unwrap().files.len(), 2);
}

#[test]
fn saving_never_overwrites_an_existing_file() {
    let lib = tempfile::tempdir().unwrap();
    let now = at("2026-10-13T06:00:00Z");
    let taken = "binance_BTCUSDT_rsi_long_20261013.ggm.json";
    fs::write(lib.path().join(taken), "{not json").unwrap();
    let m = json(r#"{"symbol":"BTCUSDT","rule_type":"F"}"#);
    let saved = save_macro(lib.path(), &m, now).unwrap();
    assert_eq!(saved.file_name, "binance_BTCUSDT_rsi_long_20261013 (2).ggm.json");
    assert_eq!(fs::read_to_string(lib.path().join(taken)).unwrap(), "{not json", "있던 파일은 그대로");
    assert!(read(lib.path(), &saved.file_name).is_ok());
}

#[test]
fn saving_refuses_what_the_folder_could_not_read_back() {
    let lib = tempfile::tempdir().unwrap();
    let now = at("2026-10-13T06:00:00Z");
    assert_eq!(save_macro(lib.path(), &json(r#"{"symbol":"BTCUSDT"}"#), now).unwrap_err(), NOT_A_MACRO);
    assert!(save_macro(lib.path(), &json("[1,2]"), now).is_err());
    assert!(scan(lib.path()).unwrap().files.is_empty(), "아무것도 쓰지 않음");
    // 폴더를 만들 수 없는 자리(파일 아래)면 이유와 함께 실패.
    let blocker = lib.path().join("file");
    fs::write(&blocker, "x").unwrap();
    let err = save_macro(&blocker.join("macros"), &json(r#"{"symbol":"BTCUSDT","rule_type":"F"}"#), now).unwrap_err();
    assert!(err.contains("매크로 폴더"), "{err}");
}
