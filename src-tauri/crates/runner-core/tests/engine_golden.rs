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
    let path: PathBuf = [env!("CARGO_MANIFEST_DIR"), "tests", "fixtures", "engine_golden.json"].iter().collect();
    let raw =
        std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("판단 계약을 읽지 못했다 {}: {e}", path.display()));
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
    // 이름이 `_blocked` 로 끝나는 경우는 거래가 **없는 것**이 요점이다(필터가 막았다).
    let empty: Vec<&String> = golden
        .cases
        .iter()
        .filter(|(name, c)| c.commands.is_empty() && !name.ends_with("_blocked"))
        .map(|(k, _)| k)
        .collect();
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
        let interval = case.macro_.get("candle_interval").and_then(|v| v.as_str()).unwrap_or_default();
        assert!(runner_core::intervals::step_ms(interval).is_some(), "{name}: {interval}");
        let exchange = case.macro_.get("exchange").and_then(|v| v.as_str()).unwrap_or_default();
        assert!(matches!(exchange, "binance" | "upbit" | "bithumb"), "{name}: {exchange}");
        if !name.ends_with("_blocked") {
            assert!(case.trade_count > 0, "{name}: 거래가 없다");
        }
    }
}

// --- 엔진이 계약대로 판단하는가 ------------------------------------------
//
// 규칙을 하나 포팅할 때마다 그 규칙이 `포팅됨` 으로 넘어간다. 열하나가 전부 넘어가면
// 실행기가 서버와 같은 자리에서 사고판다는 뜻이고, 그때 서버의
// `RUNNER_LOCAL_DECISIONS` 를 켤 수 있다.

/// 계약의 봉을 서버와 **같은 순서로** 흘려보낸다 — 봉을 넣고, 그 봉의 종가로 체결을 다 꺼낸다.
fn run_case(macro_: &serde_json::Value, golden: &Golden) -> Result<(Vec<Command>, f64), String> {
    let mut engine = runner_core::engine::runner::build(macro_).map_err(|e| e.to_string())?;
    let initial = runner_core::engine::runner::session_capital(macro_);
    let to_bar = |row: &[f64; 6]| runner_core::engine::candle_sim::Bar {
        t: row[0] as i64,
        o: row[1],
        h: row[2],
        l: row[3],
        c: row[4],
        v: Some(row[5]),
    };
    for row in &golden.candles[..golden.warmup_count] {
        // 웜업은 지표만 세우고 장부는 건드리지 않는다 — 체결을 꺼내지 않는다.
        engine.on_bar(&to_bar(row));
    }
    engine.warmup_done();
    let mut out = Vec::new();
    for row in &golden.candles[golden.warmup_count..] {
        let bar = to_bar(row);
        engine.on_bar(&bar);
        while let Some(fill) = engine.tick(bar.c) {
            let cmd = runner_core::engine::runner::command_from_fill(&fill, initial);
            out.push(Command {
                candle_t: bar.t,
                action: cmd.action.to_string(),
                notional_frac: round10(cmd.notional_frac),
                qty_frac: round10(cmd.qty_frac),
                signal_price: round10(cmd.signal_price),
                reason: cmd.reason,
            });
            if out.len() > 500 {
                return Err("체결이 끝없이 나온다".into());
            }
        }
    }
    // 마지막 봉 종가 기준 자산. 명령이 같아도 장부 식이 틀리면 여기서 갈린다.
    let last_close = golden.candles.last().map(|row| row[4]).unwrap_or(0.0);
    Ok((out, round10(engine.equity(last_close))))
}

/// 계약 파일은 **유효숫자 10자리**로 적혀 있다(파이썬 `f"{v:.10g}"`). 견주기 전에 같은
/// 자리로 맞춘다 — 지수 표기에서 소수점 아래 9자리가 유효숫자 10자리다.
fn round10(v: f64) -> f64 {
    format!("{v:.9e}").parse().unwrap_or(v)
}

#[test]
fn 포팅한_규칙은_계약대로_판단한다() {
    let golden = load();
    let mut ported = Vec::new();
    let mut waiting = Vec::new();
    let mut wrong = Vec::new();
    for (name, case) in &golden.cases {
        match run_case(&case.macro_, &golden) {
            Err(why) if why.contains("아직 직접 판단하지 못해요") => waiting.push(name.clone()),
            Err(why) => wrong.push(format!("{name}: {why}")),
            Ok((got, equity)) => {
                if got != case.commands {
                    wrong.push(format!(
                        "{name}: 판단이 계약과 다르다
  서버: {:#?}
  실행기: {:#?}",
                        case.commands, got
                    ));
                } else if (equity - case.equity).abs() > 1e-9 * case.equity.abs().max(1.0) {
                    // 명령이 같아도 장부가 다르면 다음 봉부터 갈린다.
                    wrong.push(format!(
                        "{name}: 명령은 같은데 끝 자산이 다르다 — 서버 {}, 실행기 {equity}",
                        case.equity
                    ));
                } else {
                    ported.push(name.clone());
                }
            }
        }
    }
    println!("포팅됨 {}/{}: {ported:?}", ported.len(), golden.cases.len());
    println!("남음: {waiting:?}");
    assert!(
        wrong.is_empty(),
        "{}",
        wrong.join(
            "

"
        )
    );
}

#[test]
fn 열한_규칙이_모두_포팅됐다() {
    // 하나라도 빠지면 그 매크로는 로컬 판단으로 못 돈다. 계약의 규칙 수와 맞춘다.
    let golden = load();
    let missing: Vec<&String> = golden
        .cases
        .iter()
        .filter(|(_, case)| runner_core::engine::runner::build(&case.macro_).is_err())
        .map(|(name, _)| name)
        .collect();
    assert!(missing.is_empty(), "아직 못 세우는 규칙: {missing:?}");
    let rules: std::collections::BTreeSet<&str> =
        golden.cases.values().filter_map(|c| c.macro_.get("rule_type").and_then(|v| v.as_str())).collect();
    assert_eq!(rules.len(), 11, "계약이 열한 규칙을 다 담고 있어야 한다: {rules:?}");
}

#[test]
fn 모르는_규칙은_로컬_판단을_열지_않는다() {
    // 조용히 다르게 사는 것보다 안 여는 쪽이 낫다.
    let golden = load();
    let mut unknown = golden.cases["E_trailing"].macro_.clone();
    unknown["rule_type"] = serde_json::json!("Z");
    let text = match runner_core::engine::runner::build(&unknown) {
        Err(err) => err.to_string(),
        Ok(_) => panic!("모르는 규칙인데 엔진을 세웠다"),
    };
    assert!(text.contains("아직") && text.contains("Z"), "거절 문구가 무엇이 문제인지 말하지 않는다: {text}");
}

#[test]
fn 필수_설정이_없으면_세우지_않는다() {
    // 기본값으로 때우면 사용자가 안 정한 값으로 실제 주문이 나간다.
    let golden = load();
    for (name, key) in [
        ("E_trailing", "trail_percent"),
        ("D_grid", "lower_price"),
        ("H_martingale", "take_profit"),
        ("A_tp_sl", "take_profit_pct"),
        ("B_band", "buy_price"),
        ("C_dca", "amount_per_buy"),
    ] {
        let mut broken = golden.cases[name].macro_.clone();
        broken["params"].as_object_mut().expect("params").remove(key).expect("있던 칸이어야 한다");
        let text = match runner_core::engine::runner::build(&broken) {
            Err(err) => err.to_string(),
            Ok(_) => panic!("{name}: {key} 없이도 세워졌다"),
        };
        assert!(text.contains(key), "{name}: 어느 칸이 없는지 말해야 한다 — {text}");
    }
}
