//! 실행 복구 보관함. 키 없이 같은 세션의 마지막 완결 상태를 원자적으로 저장함.

use std::path::PathBuf;

use rusqlite::{Connection, OptionalExtension, params};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::types::RunMode;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Checkpoint {
    pub recovery_id: String,
    pub session_id: i64,
    pub generation: u64,
    #[serde(rename = "macro")]
    pub macro_: Value,
    pub macro_digest: String,
    pub mode: RunMode,
    pub server_base: String,
    pub completed: bool,
    pub runtime: Value,
    pub engine_version: u32,
    pub decides_locally: bool,
}

impl Checkpoint {
    pub fn snapshot(&self) -> Value {
        self.runtime.get("snapshot").cloned().unwrap_or(Value::Null)
    }

    pub fn validate(&self) -> Result<(), String> {
        if uuid::Uuid::parse_str(&self.recovery_id).is_err()
            || self.session_id <= 0
            || !self.macro_.is_object()
            || !self.runtime.is_object()
            || self.engine_version != crate::engine::ENGINE_VERSION
        {
            return Err(
                "저장된 실행 상태를 읽을 수 없어요. 실행기를 업데이트하고 거래소에서 주문과 보유를 확인하세요.".into(),
            );
        }
        if self.completed {
            return Err("이미 종료한 실행이에요. 웹에서 실행 기록을 확인하세요.".into());
        }
        Ok(())
    }
}

#[derive(Clone, Debug)]
pub struct RecoveryStore {
    path: PathBuf,
}

#[derive(Clone, Debug)]
pub struct RecoveryContext {
    pub store: RecoveryStore,
    pub recovery_id: String,
}

impl RecoveryStore {
    pub fn new(path: PathBuf) -> Self {
        Self { path }
    }

    fn connection(&self) -> Result<Connection, String> {
        if let Some(parent) = self.path.parent() {
            std::fs::create_dir_all(parent).map_err(|_| storage_error())?;
        }
        let conn = Connection::open(&self.path).map_err(|_| storage_error())?;
        conn.busy_timeout(std::time::Duration::from_secs(5)).map_err(|_| storage_error())?;
        conn.execute_batch("PRAGMA synchronous=FULL; CREATE TABLE IF NOT EXISTS checkpoints (recovery_id TEXT PRIMARY KEY, generation INTEGER NOT NULL, body TEXT NOT NULL);").map_err(|_| storage_error())?;
        Ok(conn)
    }

    pub fn load(&self, recovery_id: &str) -> Result<Option<Checkpoint>, String> {
        let body: Option<String> = self
            .connection()?
            .query_row("SELECT body FROM checkpoints WHERE recovery_id=?1", [recovery_id], |r| r.get(0))
            .optional()
            .map_err(|_| storage_error())?;
        body.map(|body| {
            serde_json::from_str(&body)
                .map_err(|_| "저장된 실행 상태가 손상됐어요. 거래소에서 주문과 보유를 확인하세요.".into())
        })
        .transpose()
    }

    pub fn list(&self) -> Result<Vec<Checkpoint>, String> {
        let conn = self.connection()?;
        let mut stmt =
            conn.prepare("SELECT body FROM checkpoints ORDER BY recovery_id").map_err(|_| storage_error())?;
        let rows = stmt.query_map([], |r| r.get::<_, String>(0)).map_err(|_| storage_error())?;
        rows.map(|row| serde_json::from_str(&row.map_err(|_| storage_error())?).map_err(|_| storage_error())).collect()
    }

    pub fn save(&self, checkpoint: &Checkpoint) -> Result<(), String> {
        let body = serde_json::to_string(checkpoint).map_err(|_| storage_error())?;
        let generation = i64::try_from(checkpoint.generation).map_err(|_| storage_error())?;
        let changed = self.connection()?.execute("INSERT INTO checkpoints(recovery_id,generation,body) VALUES(?1,?2,?3) ON CONFLICT(recovery_id) DO UPDATE SET generation=excluded.generation, body=excluded.body WHERE checkpoints.generation<=excluded.generation",params![checkpoint.recovery_id, generation, body]).map_err(|_| storage_error())?;
        if changed != 1 {
            return Err("다른 실행기가 이 실행을 이어받았어요. 웹에서 실행 상태를 확인하세요.".into());
        }
        Ok(())
    }
}

fn storage_error() -> String {
    "실행 복구 상태를 저장하지 못해 주문을 멈췄어요. PC의 저장 공간과 파일 접근 권한을 확인하세요.".into()
}

#[cfg(test)]
mod tests;
