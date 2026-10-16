//! 웹 [실행기 열기] 티켓 교환 — 매크로·회원 키를 받음(거래소 키는 오지 않음).

use super::*;

/// 웹 [실행기 열기] 티켓을 교환해 받은 값. 거래소 키는 오지 않음.
#[derive(Clone)]
pub struct ClaimedLaunch {
    pub macro_: Value,
    pub runner_key: String,
    pub user_macro_id: Option<i64>,
    pub launch_id: Option<i64>,
    pub name: Option<String>,
}

// 회원 키는 디버그 출력에도 남기지 않음.
impl std::fmt::Debug for ClaimedLaunch {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ClaimedLaunch")
            .field("runner_key", &"[가림]")
            .field("user_macro_id", &self.user_macro_id)
            .field("launch_id", &self.launch_id)
            .field("name", &self.name)
            .finish_non_exhaustive()
    }
}

/// 티켓 교환이 안 된 까닭.
#[derive(Clone, Debug, PartialEq)]
pub enum ClaimError {
    /// 서버가 이 앱의 서버 연동 레벨(`runner_version`)으로는 이 매크로를 열지 않음(426) — 앱을 업데이트해야 함.
    /// 서버가 준 안내(detail)를 담음. 주소가 섞인 안내는 비움.
    UpdateRequired(String),
    /// 그 밖의 실패. 이유는 돌려주지 않음 — HTTP·프록시 오류에 요청 정보가 섞일 수 있음.
    Failed,
}

/// 426 안내에서 화면에 보일 글자만 남김 — 주소·제어 문자가 섞이면 버리고 기본 안내를 씀.
fn update_detail(detail: String) -> String {
    if detail.contains("://") || detail.chars().any(char::is_control) {
        String::new()
    } else {
        detail
    }
}

/// 티켓 교환. 426(업데이트 필요)만 서버 안내를 돌려주고, 다른 실패는 이유를 숨김.
pub async fn claim_ticket(http: &reqwest::Client, base: &str, ticket: &str) -> Result<ClaimedLaunch, ClaimError> {
    use ClaimError::Failed;
    let r = http
        .post(format!("{}{}", base.trim_end_matches('/'), PROTOCOL_CLAIM_PATH))
        .json(&json!({"ticket": ticket, "runner_version": SERVER_LINK_LEVEL}))
        .timeout(Duration::from_secs(15))
        .send()
        .await
        .map_err(|_| Failed)?;
    if r.status().as_u16() == 426 {
        let body = r.text().await.unwrap_or_default();
        return Err(ClaimError::UpdateRequired(update_detail(server_detail(&body).await)));
    }
    if !r.status().is_success() {
        return Err(Failed);
    }
    let payload: Value = r.json().await.map_err(|_| Failed)?;
    let obj = payload.as_object().ok_or(Failed)?;
    let macro_ = obj.get("macro").filter(|m| m.is_object()).cloned().ok_or(Failed)?;
    let symbol_ok = match macro_.get("symbol") {
        Some(Value::String(s)) => !s.trim().is_empty(),
        Some(Value::Null) | None => false,
        Some(v) => !v.to_string().trim().is_empty(),
    };
    let runner_key = obj.get("runner_key").and_then(Value::as_str).map(str::trim).unwrap_or_default().to_string();
    // 회원 키 칸과 같은 규칙(200자 이하) — 보이지 않는 문자·제어 문자가 섞이면 헤더에 실리지 못해 키 없이
    // 요청이 나가므로 받지 않음.
    let key_ok = runner_key.chars().count() <= crate::credentials::MAX_KEY_CHARS
        && runner_key.chars().all(|c| c.is_ascii_graphic());
    if !symbol_ok || runner_key.is_empty() || !key_ok {
        return Err(Failed);
    }
    let positive_int = |v: Option<&Value>| -> Result<Option<i64>, ()> {
        match v {
            None | Some(Value::Null) => Ok(None),
            Some(Value::Number(n)) => match n.as_i64() {
                Some(i) if i > 0 => Ok(Some(i)),
                _ => Err(()),
            },
            _ => Err(()),
        }
    };
    let user_macro_id = positive_int(obj.get("user_macro_id")).map_err(|_| Failed)?;
    let launch_id = positive_int(obj.get("launch_id")).ok().flatten();
    let name =
        obj.get("name").and_then(Value::as_str).map(str::trim).filter(|s| !s.is_empty()).map(|s| truncate_chars(s, 60));
    Ok(ClaimedLaunch { macro_, runner_key, user_macro_id, launch_id, name })
}
