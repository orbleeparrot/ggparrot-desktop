//! 껄무새(GGParrot) 코어. GUI 의존 없이 거래소·서버·보관함·키 저장·봇 루프를 담음.

/// 앱 버전(Cargo 패키지 버전, 예 0.20.0) — HTTP User-Agent에만 씀. 서버에는 보내지 않음.
/// 모양은 `0.<서버 연동 레벨>.<패치>` — 원본은 `src-tauri/Cargo.toml`의 `[workspace.package] version` 한 곳.
pub const APP_VERSION: &str = env!("CARGO_PKG_VERSION");
/// 서버 연동 레벨(server link level) — 시작·웹 연결 때 서버가 정한 필드 `runner_version`에 담아 보냄.
/// - 서버는 기능마다 최소 레벨(연결 6·서버 신호 8·국내 거래소 10·바이낸스 연결 도우미 19)과 이상인지만 비교해
///   그 기능을 열거나 426('실행기 vN 이상으로 업데이트')으로 막음.
/// - 번호의 출처는 Python 실행기(Windows exe, 배포 태그 `runner-vN`)의 배포 번호. 이 앱에서는 '그 레벨의 서버 연동
///   기능을 모두 갖춤'이라는 신고라서 기능을 갖춘 뒤에만 올림.
/// - 숫자로만 보냄. `0.20.0`·`v20`을 보내면 426으로 막힘.
/// - `protocol` 모듈(`ggparrot://` 딥링크, v=1·v=2)의 번호와는 다른 것.
/// - 따로 적지 않고 앱 버전의 가운데 숫자(마이너)를 빌드 때 그대로 씀 — 레벨을 올리려면 Cargo.toml 버전 한 곳만
///   `0.<새 레벨>.0`으로 바꾸면 됨.
pub const SERVER_LINK_LEVEL: &str = env!("CARGO_PKG_VERSION_MINOR");
/// 알림 제목 등에 쓰는 앱 이름(버전 없이).
pub const APP_TITLE: &str = "껄무새";

pub mod bot;
pub mod brokers;
pub mod check;
pub mod clock;
pub mod connection;
pub mod credentials;
pub mod exits;
pub mod library;
pub mod log_kind;
pub mod macro_info;
pub mod market_page;
pub mod money;
pub mod net;
pub mod outbox;
pub mod paths;
pub mod pnl;
pub mod protocol;
pub mod risk;
pub mod server;
pub mod setup_progress;
pub mod types;

#[cfg(test)]
mod tests {
    #[test]
    fn server_link_level_is_digits_only() {
        // 서버(`isdigit()` 검사)가 받는 모양 — 앱 버전(0.20.x)과 섞이면 시작이 전부 막힘.
        assert!(!super::SERVER_LINK_LEVEL.is_empty() && super::SERVER_LINK_LEVEL.bytes().all(|b| b.is_ascii_digit()));
        assert!(super::SERVER_LINK_LEVEL.len() <= 6);
        assert_ne!(super::SERVER_LINK_LEVEL, super::APP_VERSION);
    }

    #[test]
    fn app_version_is_zero_level_patch() {
        // 앱 버전은 0.<서버 연동 레벨>.<패치> — 레벨은 마이너에서 오므로 모양만 봄.
        let parts: Vec<&str> = super::APP_VERSION.split('.').collect();
        assert_eq!(parts.len(), 3, "앱 버전 모양: {}", super::APP_VERSION);
        assert_eq!(parts[0], "0", "앱 버전은 0.<서버 연동 레벨>.<패치>: {}", super::APP_VERSION);
        assert_eq!(parts[1], super::SERVER_LINK_LEVEL);
        // 레벨을 실수로 낮추면(예 0.2.0) 서버가 기능을 막음 — 이 앱이 갖춘 바이낸스 연결 도우미 레벨(19) 아래로 내려가지 않게.
        assert!(super::SERVER_LINK_LEVEL.parse::<u32>().unwrap() >= 19);
    }
}
