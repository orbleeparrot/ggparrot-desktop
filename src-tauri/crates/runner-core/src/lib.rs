//! 껄무새(GG Parrot) 코어. GUI 의존 없이 거래소·서버·보관함·키 저장·봇 루프를 담음.

/// 앱 버전(Cargo 패키지 버전, 예 0.18.18) — HTTP User-Agent에만 씀. 서버에는 보내지 않음.
pub const APP_VERSION: &str = env!("CARGO_PKG_VERSION");
/// 서버 계약 버전 18 — 시작·웹 연결 때 `runner_version`으로 보냄.
/// 서버는 숫자로만 된 값만 받고 최소 버전과 정수로 비교함. `0.18.18`·`v18`을 보내면 426으로 막힘.
pub const RUNNER_VERSION: &str = "18";
/// 알림 제목 등에 쓰는 앱 이름(버전 없이).
pub const APP_TITLE: &str = "껄무새";

pub mod bot;
pub mod brokers;
pub mod check;
pub mod connection;
pub mod credentials;
pub mod library;
pub mod log_kind;
pub mod macro_info;
pub mod money;
pub mod net;
pub mod outbox;
pub mod paths;
pub mod protocol;
pub mod risk;
pub mod server;
pub mod setup_progress;
pub mod types;

#[cfg(test)]
mod tests {
    #[test]
    fn runner_version_is_digits_only() {
        // 서버(`isdigit()` 검사)가 받는 모양 — 앱 버전(0.18.x)과 섞이면 시작이 전부 막힘.
        assert!(!super::RUNNER_VERSION.is_empty() && super::RUNNER_VERSION.bytes().all(|b| b.is_ascii_digit()));
        assert!(super::RUNNER_VERSION.len() <= 6);
        assert_ne!(super::RUNNER_VERSION, super::APP_VERSION);
    }
}
