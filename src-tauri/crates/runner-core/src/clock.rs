//! 벽시계(유닉스 시각). 잠자기 동안 멈추는 Instant 대신 씀 — 깨어난 뒤 공백·서버 시각과 비교할 때.
//! 시계를 읽지 못하면(1970년 이전으로 맞춘 PC 등) 0.

use std::time::{SystemTime, UNIX_EPOCH};

/// 유닉스 ms.
pub fn unix_ms() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_millis() as u64).unwrap_or(0)
}

/// 유닉스 초(소수).
pub fn unix_secs() -> f64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs_f64()).unwrap_or(0.0)
}
