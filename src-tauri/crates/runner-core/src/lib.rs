//! 껄무새(GGParrot) 코어. GUI 의존 없이 거래소·서버·보관함·키 저장·봇 루프를 담음.

/// 앱 버전(Cargo 패키지 버전, 예 0.19.0) — HTTP User-Agent에만 씀. 서버에는 보내지 않음.
/// 모양은 `0.<서버 연동 레벨>.<패치>` — 마이너가 SERVER_LINK_LEVEL과 같아야 함(아래 테스트).
pub const APP_VERSION: &str = env!("CARGO_PKG_VERSION");
/// 서버 연동 레벨(server link level) — 시작·웹 연결 때 서버가 정한 필드 `runner_version`에 담아 보냄.
/// - 서버는 기능마다 최소 레벨(연결 6·서버 신호 8·국내 거래소 10·바이낸스 연결 도우미 19)과 이상인지만 비교해
///   그 기능을 열거나 426('실행기 vN 이상으로 업데이트')으로 막음.
/// - 번호의 출처는 Python 실행기(Windows exe, 배포 태그 `runner-vN`)의 배포 번호. 이 앱에서는 '그 레벨의 서버 연동
///   기능을 모두 갖춤'이라는 신고라서 기능을 갖춘 뒤에만 올림.
/// - 숫자로만 보냄. `0.19.0`·`v19`를 보내면 426으로 막힘.
/// - `protocol` 모듈(`ggparrot://` 딥링크, v=1·v=2)의 번호와는 다른 것.
pub const SERVER_LINK_LEVEL: &str = "19";
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
        // 서버(`isdigit()` 검사)가 받는 모양 — 앱 버전(0.19.x)과 섞이면 시작이 전부 막힘.
        assert!(!super::SERVER_LINK_LEVEL.is_empty() && super::SERVER_LINK_LEVEL.bytes().all(|b| b.is_ascii_digit()));
        assert!(super::SERVER_LINK_LEVEL.len() <= 6);
        assert_ne!(super::SERVER_LINK_LEVEL, super::APP_VERSION);
    }

    #[test]
    fn app_minor_follows_server_link_level() {
        // 서버 연동 레벨을 올리면 앱 마이너도 같이 올림(0.<레벨>.<패치>) — 서버의 '실행기 vN 이상' 안내와 앱 버전이 바로 맞물리게.
        let mut parts = super::APP_VERSION.split('.');
        assert_eq!(parts.next(), Some("0"), "앱 버전은 0.<서버 연동 레벨>.<패치>: {}", super::APP_VERSION);
        assert_eq!(
            parts.next(),
            Some(super::SERVER_LINK_LEVEL),
            "앱 마이너가 서버 연동 레벨과 다름: {}",
            super::APP_VERSION
        );
    }
}
