use super::*;
use serde_json::json;

fn checkpoint() -> Checkpoint {
    Checkpoint {
        recovery_id: uuid::Uuid::new_v4().to_string(),
        session_id: 23,
        generation: 0,
        macro_: json!({"symbol":"BTCUSDT","rule":"A"}),
        macro_digest: "digest".into(),
        mode: RunMode::Mock,
        server_base: "http://localhost".into(),
        completed: false,
        runtime: json!({"snapshot":{"in_position":true,"position_qty":2.0}}),
        engine_version: crate::engine::ENGINE_VERSION,
        decides_locally: true,
    }
}

#[test]
fn durable_store_roundtrips_without_credentials_and_fences_older_generations() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("recovery.sqlite");
    let store = RecoveryStore::new(path.clone());
    let mut cp = checkpoint();
    store.save(&cp).unwrap();
    assert_eq!(store.load(&cp.recovery_id).unwrap().unwrap().snapshot()["position_qty"], 2.0);
    cp.generation = 1;
    store.save(&cp).unwrap();
    let mut stale = cp.clone();
    stale.generation = 0;
    assert!(store.save(&stale).is_err());
    assert_eq!(RecoveryStore::new(path).list().unwrap()[0].generation, 1);
    let raw = serde_json::to_string(&cp).unwrap();
    for key in ["member_key", "api_key", "api_secret", "runner_key"] {
        assert!(!raw.contains(key));
    }
}

#[test]
fn corrupt_storage_and_incompatible_engine_are_not_silently_replaced() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("recovery.sqlite");
    std::fs::write(&path, "broken database").unwrap();
    let store = RecoveryStore::new(path.clone());
    assert!(store.load("x").is_err());
    assert!(store.save(&checkpoint()).is_err());
    assert_eq!(std::fs::read_to_string(path).unwrap(), "broken database");
    let mut cp = checkpoint();
    cp.engine_version += 1;
    assert!(cp.validate().is_err());
}
