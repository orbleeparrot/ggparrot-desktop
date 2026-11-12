use runner_core::engine::candle_sim::Bar;
use runner_core::engine::runner::{Engine, build};
use serde_json::Value;

fn fixture() -> Value {
    serde_json::from_str(include_str!("fixtures/engine_golden.json")).unwrap()
}

fn bars(fixture: &Value) -> Vec<Bar> {
    fixture["candles"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| Bar {
            t: r[0].as_f64().unwrap() as i64,
            o: r[1].as_f64().unwrap(),
            h: r[2].as_f64().unwrap(),
            l: r[3].as_f64().unwrap(),
            c: r[4].as_f64().unwrap(),
            v: r.get(5).and_then(Value::as_f64),
        })
        .collect()
}

#[test]
fn every_engine_restores_the_same_indicators_queue_lots_and_next_decisions() {
    let fixture = fixture();
    let history = bars(&fixture);
    for (name, case) in fixture["cases"].as_object().unwrap() {
        let mut original = build(&case["macro"]).unwrap();
        for bar in &history[..45] {
            original.on_bar(bar);
            for _ in 0..50 {
                if original.tick(bar.c).is_none() {
                    break;
                }
            }
        }
        let raw = serde_json::to_string(&original).unwrap();
        let mut restored: Engine = serde_json::from_str(&raw).unwrap_or_else(|e| panic!("{name}: {e}"));
        for bar in &history[45..] {
            original.on_bar(bar);
            restored.on_bar(bar);
            for _ in 0..50 {
                let expected = original.tick(bar.c);
                let actual = restored.tick(bar.c);
                assert_eq!(expected, actual, "{name} at {}", bar.t);
                if expected.is_none() {
                    break;
                }
            }
        }
        assert_eq!(serde_json::to_value(&original).unwrap(), serde_json::to_value(&restored).unwrap(), "{name}");
    }
}

#[test]
fn multi_hour_gap_refreshes_indicators_without_mutating_position_risk_or_replaying_orders() {
    let fixture = fixture();
    let history = bars(&fixture);
    for (name, case) in fixture["cases"].as_object().unwrap() {
        let macro_ = &case["macro"];
        if matches!(macro_["rule_type"].as_str(), Some("A" | "B" | "C")) {
            continue;
        }
        let mut engine = build(macro_).unwrap();
        for bar in &history[..35] {
            engine.on_bar(bar);
            for _ in 0..50 {
                if engine.tick(bar.c).is_none() {
                    break;
                }
            }
        }
        let before = serde_json::to_value(&engine).unwrap();
        engine.refresh_market(macro_, &history).unwrap();
        let after = serde_json::to_value(&engine).unwrap();
        let variant = before.as_object().unwrap().keys().next().unwrap();
        for field in [
            "ledger",
            "risk",
            "day",
            "day_start_equity",
            "halted_today",
            "cooldown_until_ms",
            "entry_time_ms",
            "stopped",
        ] {
            assert_eq!(before[variant][field], after[variant][field], "{name} {field}");
        }
        assert!(engine.tick(history.last().unwrap().c).is_none(), "{name}: outage order replay");
    }
}
