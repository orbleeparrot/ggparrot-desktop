//! 업비트·빗썸 오류 이름 분류. 같은 이름을 두 곳이 서로 다르게 나누므로 한 파일에 나란히 둠.
//!
//! - 오류 문장(`explain_domestic_error`)·접근 오류 판정(`is_access_error`): 사람이 다음에 할 일 기준.
//!   invalid_access_key·no_authorization_token은 허용 IP가 막혀도 오는 이름이라 문장이 IP부터 확인하게 함.
//! - 연결 도우미 세 줄(`connection::check_view`): 막힌 줄 하나를 고르는 기준. 한 줄만 표시하므로
//!   이름이 가리키는 그대로 IP 줄·키 줄·주문 권한 줄로 나눔.
//! - 두 분류의 관계(세 줄의 키·IP·권한 이름은 모두 접근 오류, 주문 내용 오류는 접근 오류가 아님)는 테스트로 묶음.

// --- 오류 문장·접근 오류 판정 ---------------------------------------------------

/// 허용 IP를 먼저 말해야 하는 오류들.
pub(crate) const IP_FIRST_ERRORS: [&str; 4] =
    ["no_authorization_ip", "NotAllowIP", "invalid_access_key", "no_authorization_token"];
pub(crate) const EXPIRED_KEY_ERRORS: [&str; 1] = ["expired_access_key"];
pub(crate) const NO_FUNDS_ERRORS: [&str; 3] =
    ["insufficient_funds_bid", "insufficient_funds_ask", "insufficient_funds"];
pub(crate) const MARKET_ERRORS: [&str; 3] = ["notfoundmarket", "invalid_market", "market_offline"];
pub(crate) const OTHER_ACCESS_ERRORS: [&str; 4] =
    ["out_of_scope", "invalid_query_payload", "expired_jwt", "jwt_verification"];

// --- 연결 도우미 세 줄 -----------------------------------------------------------

/// 키는 받아들였지만 호출한 IP가 허용 목록에 없음 — 업비트 no_authorization_ip, 빗썸 NotAllowIP.
pub(crate) const IP_REJECTED: [&str; 2] = ["no_authorization_ip", "NotAllowIP"];
/// 키 자체를 받아들이지 않음(틀린 키·시크릿 짝 불일치·만료).
pub(crate) const KEY_REJECTED: [&str; 4] =
    ["invalid_access_key", "jwt_verification", "no_authorization_token", "expired_access_key"];
/// 키와 IP는 통과했고 주문하기 권한이 없음.
pub(crate) const SCOPE_REJECTED: [&str; 1] = ["out_of_scope"];
/// 거래소가 주문 내용(잔고·최소 금액)까지 따져 본 오류 — 키·IP·주문 권한은 이미 통과했다는 뜻임.
pub(crate) const ORDER_EVALUATED: [&str; 5] =
    ["insufficient_funds_bid", "insufficient_funds_ask", "insufficient_funds", "over_krw_funds_bid", "under_min_total"];

#[cfg(test)]
mod tests {
    use super::*;
    use crate::brokers::domestic::is_access_error;

    #[test]
    fn row_lists_and_explanation_lists_agree() {
        // 세 줄에서 키·IP·권한으로 막는 이름은 모두 접근 오류(시작 준비에서 '마켓 없음'으로 접지 않음).
        for name in IP_REJECTED.iter().chain(&KEY_REJECTED).chain(&SCOPE_REJECTED) {
            assert!(is_access_error(400, name), "{name}");
        }
        // 주문 내용까지 따져 본 오류는 접근 오류가 아님.
        for name in ORDER_EVALUATED {
            assert!(!is_access_error(400, name), "{name}");
        }
        // 잔고 부족 문장을 받는 이름은 세 줄에서도 '연결은 됨'.
        for name in NO_FUNDS_ERRORS {
            assert!(ORDER_EVALUATED.contains(&name), "{name}");
        }
        // IP를 먼저 말하는 문장의 이름은 세 줄에서 IP 줄이나 키 줄 중 하나를 막음.
        for name in IP_FIRST_ERRORS {
            assert!(IP_REJECTED.contains(&name) || KEY_REJECTED.contains(&name), "{name}");
        }
    }
}
