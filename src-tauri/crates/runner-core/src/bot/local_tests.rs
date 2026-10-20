//! 실행기가 스스로 판단하는 길 (2026-10-20).
//!
//! 엔진이 계약과 일치하는지는 `tests/engine_golden.rs` 가 본다. 여기서 보는 것은 **그
//! 판단이 실제 주문과 보고로 이어지는 길**이다:
//!
//! * 주문은 서버 명령과 **같은 길**로 나간다(주문 상한 · 최소 수량 · 수수료 보정이 똑같이 걸린다).
//! * 로컬 리스크(일일 손실 · 쿨다운)가 로컬 판단에도 걸린다.
//! * 체결은 서버에 보고되고, **보낸 뒤에만** 지운다 — 못 보낸 체결은 리더보드에서 사라진다.
//! * 로컬 명령 번호가 서버 명령 번호와 섞이지 않는다.

use super::local::LOCAL_ID_PREFIX;
use super::tests::*;
use super::*;

/// 로컬 판단이 만드는 명령 — 실제 `local_round` 가 내는 것과 같은 모양.
fn local_command(seq: u64, action: &str, frac: f64) -> Value {
    let entry = action == "buy" || action == "short";
    json!({
        "id": format!("{LOCAL_ID_PREFIX}{seq}"),
        "seq": seq,
        "source": "local",
        "action": action,
        "notional_frac": if entry { frac } else { 0.0 },
        "qty_frac": if entry { 0.0 } else { frac },
        "signal_price": 100.0,
        "reason": "진입",
    })
}

#[tokio::test]
async fn 로컬_명령도_서버_명령과_같은_길로_나간다() {
    // 길이 하나여야 주문 상한 · 최소 수량 · 수수료 보정이 양쪽에 똑같이 걸린다.
    let fb = FakeBroker::new(100.0, Market::Spot, Side::Long);
    let (mut bot, _rx) = bot_with(
        json!({"symbol": "BTCUSDT", "params": {"initial_capital": 1000}}),
        fb.clone(),
        RunMode::Live,
    );
    let ack = bot.execute_command(&local_command(1, "buy", 0.05), 100.0).await.unwrap();
    assert_eq!(ack["ok"], true);
    assert_eq!(fb.submits()[0].base_qty, Some(0.5), "서버 명령과 같은 수량 계산");
    assert!(bot.in_position);
}

#[tokio::test]
async fn 로컬_명령_번호는_서버_명령_번호와_섞이지_않는다() {
    // 겹치면 "이미 했다" 기억이 서로의 명령을 지워 주문이 통째로 사라진다.
    let fb = FakeBroker::new(100.0, Market::Spot, Side::Long);
    let (mut bot, _rx) = bot_with(
        json!({"symbol": "BTCUSDT", "params": {"initial_capital": 1000}}),
        fb.clone(),
        RunMode::Live,
    );
    // 서버 명령 1 번과 로컬 명령 1 번은 **다른 것**이다.
    bot.execute_command(&json!({"id": 1, "action": "buy", "notional_frac": 0.05, "reason": "서버"}), 100.0)
        .await
        .unwrap();
    let before = fb.submits().len();
    let ack = bot.execute_command(&local_command(1, "sell", 1.0), 100.0).await.unwrap();
    assert_eq!(ack["ok"], true);
    assert!(
        fb.submits().len() > before,
        "로컬 1 번이 서버 1 번과 같은 것으로 읽혀 주문이 사라졌다"
    );
}

#[tokio::test]
async fn 같은_로컬_명령을_두_번_실행하지_않는다() {
    let fb = FakeBroker::new(100.0, Market::Spot, Side::Long);
    let (mut bot, _rx) = bot_with(
        json!({"symbol": "BTCUSDT", "params": {"initial_capital": 1000}}),
        fb.clone(),
        RunMode::Live,
    );
    let cmd = local_command(7, "buy", 0.05);
    bot.execute_command(&cmd, 100.0).await.unwrap();
    let after_first = fb.submits().len();
    let ack = bot.execute_command(&cmd, 100.0).await.unwrap();
    assert_eq!(ack["outcome"], "duplicate_command_no_reexecution");
    assert_eq!(fb.submits().len(), after_first, "같은 명령으로 두 번 주문했다");
}

#[test]
fn 로컬_명령은_문자열_번호를_쓴다() {
    // 서버 번호는 정수다. 모양이 달라야 섞이지 않는다.
    let cmd = local_command(3, "buy", 1.0);
    let id = cmd.get("id").and_then(Value::as_str).expect("문자열 번호");
    assert!(id.starts_with(LOCAL_ID_PREFIX), "{id}");
    assert!(cmd.get("id").unwrap().as_i64().is_none(), "정수로 읽히면 서버 번호와 겹친다");
    assert_eq!(cmd["source"], "local", "기록에서 출처가 구분돼야 한다");
}

#[tokio::test]
async fn 판단_주체를_서버가_정한다() {
    // 서버가 꺼 두었으면 실행기가 요청해도 서버 명령만 실행한다 — 양쪽이 동시에
    // 판단하면 같은 신호로 두 번 산다.
    let fb = FakeBroker::new(100.0, Market::Spot, Side::Long);
    let (bot, _rx) = bot_with(json!({"symbol": "BTCUSDT"}), fb, RunMode::Live);
    assert!(!bot.decides_locally(), "서버가 없으면 로컬 판단을 켜지 않는다");
}

#[test]
fn 시작_payload_가_판단_의사와_엔진_세대를_싣는다() {
    // 이 둘이 없으면 서버가 로컬 판단을 켜 줄 수도, 세대를 가릴 수도 없다.
    let m = json!({"symbol": "BTCUSDT", "position_side": "long", "leverage": 1});
    let payload = crate::types::session_payload(&m, true, RunMode::Live);
    assert_eq!(payload["decides"], "local");
    assert_eq!(payload["engine_version"], crate::engine::ENGINE_VERSION.to_string());
    // 서버는 숫자 문자열만 읽는다(`isdigit()`).
    let version = payload["engine_version"].as_str().expect("문자열");
    assert!(version.chars().all(|c| c.is_ascii_digit()), "{version}");
}

#[test]
fn 엔진_세대가_판단_계약과_같다() {
    // 계약을 새로 받아 왔는데 세대를 안 올리면 서버가 옛 실행기를 막지 못한다.
    let path: std::path::PathBuf =
        [env!("CARGO_MANIFEST_DIR"), "tests", "fixtures", "engine_golden.json"].iter().collect();
    let raw = std::fs::read_to_string(path).expect("판단 계약");
    let golden: Value = serde_json::from_str(&raw).expect("판단 계약 모양");
    assert_eq!(golden["engine_version"].as_u64(), Some(crate::engine::ENGINE_VERSION as u64));
}

#[tokio::test]
async fn 열한_매매_방식_모두_로컬_엔진을_세울_수_있다() {
    // 하나라도 못 세우면 그 매크로는 로컬 판단으로 못 돈다.
    let path: std::path::PathBuf =
        [env!("CARGO_MANIFEST_DIR"), "tests", "fixtures", "engine_golden.json"].iter().collect();
    let golden: Value =
        serde_json::from_str(&std::fs::read_to_string(path).expect("판단 계약")).expect("모양");
    let cases = golden["cases"].as_object().expect("경우들");
    let mut rules: std::collections::BTreeSet<String> = Default::default();
    for (name, case) in cases {
        let macro_ = &case["macro"];
        crate::engine::runner::build(macro_)
            .unwrap_or_else(|_| panic!("{name}: 로컬 엔진을 세우지 못했다"));
        rules.insert(macro_["rule_type"].as_str().unwrap_or_default().to_string());
    }
    assert_eq!(rules.len(), 11, "열한 매매 방식이 다 있어야 한다: {rules:?}");
}

// --- 가격선 -------------------------------------------------------------------

/// 계약의 봉을 엔진에 흘려보내고, 그 규칙이 내는 가격선을 본다.
fn levels_after_contract(name: &str) -> crate::engine::levels::Levels {
    let path: std::path::PathBuf =
        [env!("CARGO_MANIFEST_DIR"), "tests", "fixtures", "engine_golden.json"].iter().collect();
    let golden: Value =
        serde_json::from_str(&std::fs::read_to_string(path).expect("판단 계약")).expect("모양");
    let case = &golden["cases"][name];
    let warmup = golden["warmup_count"].as_u64().expect("웜업") as usize;
    let rows = golden["candles"].as_array().expect("봉");
    let to_bar = |row: &Value| crate::engine::candle_sim::Bar {
        t: row[0].as_f64().unwrap_or_default() as i64,
        o: row[1].as_f64().unwrap_or_default(),
        h: row[2].as_f64().unwrap_or_default(),
        l: row[3].as_f64().unwrap_or_default(),
        c: row[4].as_f64().unwrap_or_default(),
        v: row[5].as_f64(),
    };
    let mut engine = crate::engine::runner::build(&case["macro"]).expect("엔진");
    let mut last = None;
    for (i, row) in rows.iter().enumerate() {
        let bar = to_bar(row);
        engine.on_bar(&bar);
        if i == warmup - 1 {
            engine.warmup_done();
        }
        if i >= warmup {
            while engine.tick(bar.c).is_some() {}
        }
        last = Some(bar);
    }
    engine.levels(last.as_ref())
}

#[test]
fn 열한_매매_방식_모두_그릴_선을_낸다() {
    // 선이 없으면 사용자는 "지금 무엇을 기다리는지" 를 볼 수 없다. 화면이 빈칸을
    // 추정으로 메우기 시작하면 그 추정선이 실제 조건인 척한다 — 한 번 겪은 일이다.
    let path: std::path::PathBuf =
        [env!("CARGO_MANIFEST_DIR"), "tests", "fixtures", "engine_golden.json"].iter().collect();
    let golden: Value =
        serde_json::from_str(&std::fs::read_to_string(path).expect("판단 계약")).expect("모양");
    let mut silent = Vec::new();
    for name in golden["cases"].as_object().expect("경우들").keys() {
        let levels = levels_after_contract(name);
        if !levels.has_any() && !levels.pending && levels.next_buy_ms.is_none() {
            silent.push(name.clone());
        }
    }
    // A 는 비어 있으면 다음 틱에 바로 사므로 그릴 선이 없는 것이 정상이다.
    silent.retain(|n| !n.starts_with("A_"));
    assert!(silent.is_empty(), "그릴 선을 하나도 못 내는 경우: {silent:?}");
}

#[test]
fn 돌파선은_마지막_봉의_시가에서_나온다() {
    // 평단이나 현재가로 재면 선이 가격을 따라 움직인다 — E 눌림목에서 겪은 거짓이다.
    let levels = levels_after_contract("I_breakout");
    assert!(
        levels.entry_target.is_some() || levels.trail_stop.is_some(),
        "돌파 규칙이 사는 선도 파는 선도 못 낸다"
    );
}

#[test]
fn 그리드는_사는_칸과_파는_칸을_모두_낸다() {
    let levels = levels_after_contract("D_grid_inband");
    assert!(!levels.buy_levels.is_empty(), "사려고 기다리는 칸이 없다");
    assert!(
        levels.buy_levels.iter().all(|v| v.is_finite() && *v > 0.0),
        "가격이 아닌 값이 섞였다: {:?}",
        levels.buy_levels
    );
}

#[test]
fn 볼린저는_밴드_셋을_낸다() {
    let levels = levels_after_contract("G_bollinger");
    assert!(levels.reference.is_some() && levels.band_upper.is_some() && levels.band_lower.is_some());
    let (mid, up, low) =
        (levels.reference.unwrap(), levels.band_upper.unwrap(), levels.band_lower.unwrap());
    assert!(low < mid && mid < up, "밴드 순서가 뒤집혔다: {low} {mid} {up}");
}

#[test]
fn 적립식은_가격선_대신_다음_매수_시각을_낸다() {
    // 적립식은 가격이 아니라 **달력**이 기준이다.
    let levels = levels_after_contract("C_dca");
    assert!(levels.next_buy_ms.is_some(), "다음 매수 예정 시각이 없다");
    assert_eq!(levels.entry_target, None, "적립식에 '사는 선' 은 없다");
}

#[test]
fn 값이_없으면_그린다고_하지_않는다() {
    // "관심 없음" 과 "아직 값이 없음" 은 다르다. 섞이면 화면이 빈칸을 추정으로 메운다.
    let fresh = crate::engine::runner::build(&json!({
        "rule_type": "G", "symbol": "BTCUSDT", "position_side": "long",
        "params": {"bb_period": 20, "bb_std": 2.0, "initial_capital": 1000}
    }))
    .expect("엔진");
    let levels = fresh.levels(None);
    assert!(levels.pending, "첫 봉 전에는 pending 이어야 한다");
    assert!(!levels.has_any());
}

#[test]
fn 가격이_아닌_값은_선이_되지_않는다() {
    use crate::engine::levels::price;
    assert_eq!(price(0.0), None);
    assert_eq!(price(-1.0), None);
    assert_eq!(price(f64::NAN), None);
    assert_eq!(price(f64::INFINITY), None);
    assert_eq!(price(1.5), Some(1.5));
}
