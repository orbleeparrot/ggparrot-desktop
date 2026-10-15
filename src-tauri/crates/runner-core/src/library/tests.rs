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
