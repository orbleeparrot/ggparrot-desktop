//! 판단 계약 — 실행기 엔진이 **서버와 같은 답**을 내는가.
//!
//! 판단이 실행기로 내려오면 엔진이 두 언어로 존재한다(서버는 파이썬 `backend/engine/`,
//! 여기는 Rust). "같은 코드를 쓴다" 는 말이 성립하지 않으므로, 할 수 있는 것은 하나다 —
//! **같은 입력에 같은 답**을 글로 박아 두고 양쪽을 재는 것.
//!
//! `tests/fixtures/engine_golden.json` 이 그 계약이다. 서버 쪽에서
//! `gg_parrot/backend/tests/test_engine_golden.py --regolden` 으로 만들고 여기로 복사한다.
//! 고정된 90개 봉에 여섯 규칙(D · E · F · G · H · I)을 돌려 나온 주문 명령 · 거래 수 ·
//! 끝 자산이 들어 있다.
//!
//! **복사본이 낡아도 조용히 돌지 않는다.** 계약에는 `engine_version` 이 함께 들어 있고,
//! 실행기는 그 세대를 세션 시작에 서버로 보낸다. 서버가 그보다 높은 세대를 요구하면
//! 426 으로 막는다(서버 `engine/version.py`). 그래서 세대만 맞춰 두면 낡은 복사본으로
//! 실거래가 도는 일은 없다.
//!
//! 지금은 엔진이 아직 없다 — 각 규칙을 구현할 때마다 그 규칙의 시험이 하나씩 켜진다.

use serde::Deserialize;
use std::path::PathBuf;

#[derive(Debug, Deserialize)]
struct Golden {
    engine_version: u32,
    interval: String,
    warmup_count: usize,
    candles: Vec<[f64; 6]>,
    cases: std::collections::BTreeMap<String, Case>,
}

#[derive(Debug, Deserialize)]
struct Case {
    #[serde(rename = "macro")]
    macro_: serde_json::Value,
    commands: Vec<Command>,
    trade_count: u32,
    #[allow(dead_code)]
    equity: f64,
}

#[derive(Debug, Deserialize, PartialEq)]
struct Command {
    candle_t: i64,
    action: String,
    notional_frac: f64,
    qty_frac: f64,
    signal_price: f64,
    reason: String,
}

fn load() -> Golden {
    let path: PathBuf =
        [env!("CARGO_MANIFEST_DIR"), "tests", "fixtures", "engine_golden.json"].iter().collect();
    let raw = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("판단 계약을 읽지 못했다 {}: {e}", path.display()));
    serde_json::from_str(&raw).expect("판단 계약 모양이 예상과 다르다")
}

/// 실행기가 서버에 알리는 엔진 세대. 계약과 **같아야** 한다 — 계약을 새로 받아 왔는데
/// 세대를 안 올리면 서버가 막지 못하고, 세대만 올리고 계약을 안 받아 오면 여기서 깨진다.
const IMPLEMENTED_ENGINE_VERSION: u32 = 1;

#[test]
fn 계약과_실행기_엔진_세대가_같다() {
    assert_eq!(
        load().engine_version,
        IMPLEMENTED_ENGINE_VERSION,
        "계약(engine_golden.json)의 세대와 실행기가 구현한 세대가 다르다 — \
         서버에서 --regolden 으로 다시 만들어 복사했는지, ENGINE_VERSION 을 함께 올렸는지 보라"
    );
}

#[test]
fn 계약이_실제로_사고파는_것을_담고_있다() {
    // 아무것도 안 사는 계약은 아무것도 보장하지 않는다.
    let golden = load();
    let empty: Vec<&String> =
        golden.cases.iter().filter(|(_, c)| c.commands.is_empty()).map(|(k, _)| k).collect();
    assert!(empty.is_empty(), "주문이 하나도 없는 규칙: {empty:?}");
    let total: usize = golden.cases.values().map(|c| c.commands.len()).sum();
    assert!(total >= 20, "거래가 너무 적다: {total}");
    let sells: usize = golden
        .cases
        .values()
        .flat_map(|c| &c.commands)
        .filter(|cmd| cmd.action == "sell" || cmd.action == "cover")
        .count();
    assert!(sells >= 3, "청산 명령이 거의 없다 — 청산 비율 계산이 대조되지 않는다");
}

#[test]
fn 계약의_봉이_실행기가_읽을_수_있는_모양이다() {
    let golden = load();
    assert_eq!(golden.interval, "1h");
    assert!(golden.candles.len() > golden.warmup_count);
    let step = runner_core::intervals::step_ms(&golden.interval).expect("간격");
    // 봉은 (t, o, h, l, c, v) 이고 간격만큼 떨어져 있어야 한다 — 아니면 계약을 잘못 읽은 것이다.
    for pair in golden.candles.windows(2) {
        assert_eq!(pair[1][0] as i64 - pair[0][0] as i64, step);
    }
    for row in &golden.candles {
        let (o, h, l, c) = (row[1], row[2], row[3], row[4]);
        assert!(h >= o.max(c) && l <= o.min(c), "고가 · 저가가 시가 · 종가를 담지 못한다: {row:?}");
    }
}

#[test]
fn 계약의_매크로를_실행기가_알아본다() {
    // 규칙 · 간격 · 거래소를 못 읽으면 엔진을 붙여도 그 매크로는 못 돈다.
    let golden = load();
    for (name, case) in &golden.cases {
        let rule = case.macro_.get("rule_type").and_then(|v| v.as_str()).unwrap_or("");
        assert!(!rule.is_empty(), "{name}: rule_type 이 없다");
        let interval =
            case.macro_.get("candle_interval").and_then(|v| v.as_str()).unwrap_or_default();
        assert!(runner_core::intervals::step_ms(interval).is_some(), "{name}: {interval}");
        let exchange = case.macro_.get("exchange").and_then(|v| v.as_str()).unwrap_or_default();
        assert!(matches!(exchange, "binance" | "upbit" | "bithumb"), "{name}: {exchange}");
        assert!(case.trade_count > 0, "{name}: 거래가 없다");
    }
}

// --- 엔진이 붙으면 켜지는 시험 ----------------------------------------------
//
// 규칙 하나를 구현할 때마다 아래 `ignore` 를 떼고 그 규칙을 목록에 넣는다. 전부 켜지면
// 실행기가 서버와 같은 자리에서 사고판다는 뜻이고, 그때 서버의
// `RUNNER_LOCAL_DECISIONS` 를 켤 수 있다.
#[test]
#[ignore = "엔진 미구현 — 규칙을 포팅하면서 하나씩 켠다"]
fn 엔진이_계약대로_판단한다() {
    let golden = load();
    let warmup = &golden.candles[..golden.warmup_count];
    let live = &golden.candles[golden.warmup_count..];
    let _ = (warmup, live);
    for (name, case) in &golden.cases {
        // TODO(engine): 드라이버를 세우고 warmup → 봉마다 push → 틱으로 체결을 꺼내
        //               case.commands 와 글자까지 대조한다.
        let _ = (name, &case.commands);
        unimplemented!("전략 엔진 포팅 전");
    }
}
