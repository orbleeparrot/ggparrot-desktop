//! 도우미 단계 순서 — 국내·바이낸스 사이트(env)마다 '처음이에요'의 단계가 다름.

const STAGES: [&str; 9] = ["ask", "ip", "perm", "keys", "existing", "manual", "checking", "result_ok", "result_fail"];

/// '처음이에요'의 단계 순서(웹 가이드도 같은 순서로 그림).
/// - 국내는 허용 IP를 먼저 등록하고 발급함.
/// - 바이낸스는 키를 만든 뒤에야 그 키에 IP·권한을 걺. 테스트넷·데모에는 IP 제한이 없어 두 단계로 끝남.
pub fn flow(env: &str) -> &'static [&'static str] {
    match env {
        "testnet" | "demo" => &["perm", "keys"],
        "live" => &["perm", "keys", "ip"],
        _ => &["ip", "perm", "keys"],
    }
}

/// 키를 넣는 단계 — 기존 키 길이면 기존 키 넣기, 아니면 키 복사.
pub(super) fn key_stage(path: &str) -> &'static str {
    if path == "existing" { "existing" } else { "keys" }
}

/// [이전]으로 갈 단계. None이면 도우미를 닫음.
/// - 발급 단계는 앞 단계로, 첫 단계는 '발급 받은 적 있나요?'로.
/// - 확인·결과 화면은 '처음이에요'(new)면 그 사이트의 마지막 단계(바이낸스 실거래는 허용 IP)로, 기존 키면 기존 키 넣기로.
/// - 직접 입력·길을 정하지 않고 연 결과 화면은 키 넣는 단계로.
pub(super) fn back_stage(env: &str, stage: &str, path: &str) -> Option<&'static str> {
    let steps = flow(env);
    Some(match stage {
        "ask" => return None,
        "existing" => "ask",
        "checking" | "result_ok" | "result_fail" if path == "new" => steps[steps.len() - 1],
        s => match steps.iter().position(|x| *x == s) {
            Some(0) => "ask",
            Some(i) => steps[i - 1],
            None => key_stage(path),
        },
    })
}

/// 이 도우미에서 열 수 있는 단계인지 — 허용 IP 단계는 그 단계가 있는 곳(국내·바이낸스 실거래)만.
pub(super) fn stage_allowed(env: &str, stage: &str) -> bool {
    STAGES.contains(&stage) && (stage != "ip" || flow(env).contains(&"ip"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn binance_flows_create_the_key_before_ip_and_practice_has_no_ip_step() {
        assert_eq!(flow(""), ["ip", "perm", "keys"]);
        assert_eq!(flow("testnet"), ["perm", "keys"]);
        assert_eq!(flow("demo"), ["perm", "keys"]);
        assert_eq!(flow("live"), ["perm", "keys", "ip"]);
        assert!(!stage_allowed("demo", "ip") && !stage_allowed("testnet", "ip"));
        assert!(stage_allowed("live", "ip") && stage_allowed("", "ip"));
        assert!(stage_allowed("testnet", "result_fail") && !stage_allowed("", "typed-key"));
    }

    #[test]
    fn back_from_the_result_returns_to_the_last_step_of_the_flow() {
        // 바이낸스 실거래 '처음이에요'는 허용 IP가 마지막 단계임.
        for stage in ["checking", "result_ok", "result_fail"] {
            assert_eq!(back_stage("live", stage, "new"), Some("ip"), "{stage}");
            assert_eq!(back_stage("demo", stage, "new"), Some("keys"), "{stage}");
            assert_eq!(back_stage("", stage, "new"), Some("keys"), "{stage}");
            assert_eq!(back_stage("live", stage, "existing"), Some("existing"), "{stage}");
            assert_eq!(back_stage("live", stage, ""), Some("keys"), "{stage}");
        }
        assert_eq!(back_stage("live", "manual", "new"), Some("keys"), "직접 입력은 키 복사로");
        assert_eq!(back_stage("live", "manual", "existing"), Some("existing"));
        assert_eq!(back_stage("live", "ip", "new"), Some("keys"));
        assert_eq!(back_stage("live", "perm", "new"), Some("ask"));
        assert_eq!(back_stage("", "perm", "new"), Some("ip"));
        assert_eq!(back_stage("testnet", "existing", "existing"), Some("ask"));
        assert_eq!(back_stage("testnet", "ask", ""), None);
    }
}
