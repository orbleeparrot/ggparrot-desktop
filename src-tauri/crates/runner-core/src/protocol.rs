//! `ggparrot://` 실행 계약 파서 — 티켓과 이름 붙은 환경 말고는 아무것도 받지 않음.

use once_cell::sync::Lazy;
use regex::Regex;

pub const PROTOCOL_CLAIM_PATH: &str = "/api/runner/launch-tickets/claim";

static V1: Lazy<Regex> = Lazy::new(|| Regex::new(r"^ggparrot://launch/?\?v=1&ticket=([A-Za-z0-9_-]{43})$").unwrap());
static V2: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"^ggparrot://launch/?\?v=2&env=(local|production)&ticket=([A-Za-z0-9_-]{43})$").unwrap());

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Environment {
    Local,
    Production,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProtocolLaunch {
    pub ticket: String,
    pub environment: Environment,
    pub version: u8,
}

/// 딥링크 URL 하나를 검증함.
pub fn parse_protocol_url(url: &str) -> Result<ProtocolLaunch, String> {
    if let Some(c) = V1.captures(url) {
        return Ok(ProtocolLaunch { ticket: c[1].to_string(), environment: Environment::Production, version: 1 });
    }
    if let Some(c) = V2.captures(url) {
        let environment = if &c[1] == "local" { Environment::Local } else { Environment::Production };
        return Ok(ProtocolLaunch { ticket: c[2].to_string(), environment, version: 2 });
    }
    Err("지원하지 않는 웹 연결 주소예요.".into())
}

#[cfg(test)]
mod tests {
    use super::*;

    const T: &str = "abcdefghijklmnopqrstuvwxyzABCDEFGHIJ0123456"; // 43

    #[test]
    fn v1_is_production() {
        let l = parse_protocol_url(&format!("ggparrot://launch?v=1&ticket={T}")).unwrap();
        assert_eq!(l.environment, Environment::Production);
        assert_eq!(l.version, 1);
        assert_eq!(l.ticket, T);
    }

    #[test]
    fn v2_named_envs_and_root_slash() {
        for (env, want) in [("local", Environment::Local), ("production", Environment::Production)] {
            for slash in ["", "/"] {
                let u = format!("ggparrot://launch{slash}?v=2&env={env}&ticket={T}");
                assert_eq!(parse_protocol_url(&u).unwrap().environment, want);
            }
        }
        assert!(parse_protocol_url(&format!("ggparrot://launch/?v=2&env=staging&ticket={T}")).is_err());
    }

    #[test]
    fn strict_shape() {
        for bad in [
            format!("ggparrot://launch//?v=1&ticket={T}"),
            format!("ggparrot://launch?v=1&ticket={T}&x=1"),
            format!("ggparrot://launch?v=1&ticket={}", &T[..42]),
            format!("ggparrot://other?v=1&ticket={T}"),
            format!("ggparrot://launch?v=2&env=local&ticket={T}&host=evil"),
        ] {
            assert!(parse_protocol_url(&bad).is_err(), "{bad}");
        }
    }
}
