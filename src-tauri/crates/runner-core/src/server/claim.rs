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

/// 티켓 교환. 실패 이유는 돌려주지 않음 — HTTP·프록시 오류에 요청 정보가 섞일 수 있음.
#[allow(clippy::result_unit_err)] // 이유를 일부러 숨김(위 설명).
pub async fn claim_ticket(http: &reqwest::Client, base: &str, ticket: &str) -> Result<ClaimedLaunch, ()> {
    let r = http
        .post(format!("{}{}", base.trim_end_matches('/'), PROTOCOL_CLAIM_PATH))
        .json(&json!({"ticket": ticket, "runner_version": RUNNER_VERSION}))
        .timeout(Duration::from_secs(15))
        .send()
        .await
        .map_err(|_| ())?;
    if !r.status().is_success() {
        return Err(());
    }
    let payload: Value = r.json().await.map_err(|_| ())?;
    let obj = payload.as_object().ok_or(())?;
    let macro_ = obj.get("macro").filter(|m| m.is_object()).cloned().ok_or(())?;
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
        return Err(());
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
    let user_macro_id = positive_int(obj.get("user_macro_id"))?;
    let launch_id = positive_int(obj.get("launch_id")).ok().flatten();
    let name =
        obj.get("name").and_then(Value::as_str).map(str::trim).filter(|s| !s.is_empty()).map(|s| truncate_chars(s, 60));
    Ok(ClaimedLaunch { macro_, runner_key, user_macro_id, launch_id, name })
}
