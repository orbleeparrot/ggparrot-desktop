//! 복구 미리보기·세션 소유권 갱신. 거래소 주문·거래소 키는 전송하지 않음.

use super::*;

impl ServerClient {
    pub fn generation(&self) -> u64 {
        self.inner.generation.lock().unwrap().unwrap_or(0)
    }

    pub fn macro_digest(&self) -> String {
        self.inner.macro_digest.lock().unwrap().clone()
    }

    pub(super) fn bind_session(&self, data: &Value) -> Result<(), StartError> {
        let session_id = data.get("session_id").and_then(Value::as_i64).filter(|v| *v > 0).ok_or_else(|| {
            StartError::Server(ServerRequestError {
                status: 200, detail: "실행 세션을 확인하지 못했어요.".into()
            })
        })?;
        *self.inner.session_id.lock().unwrap() = Some(session_id);
        *self.inner.generation.lock().unwrap() = data.get("generation").and_then(Value::as_u64);
        *self.inner.macro_digest.lock().unwrap() =
            data.get("macro_digest").and_then(Value::as_str).unwrap_or("").into();
        *self.inner.decides.lock().unwrap() = data.get("decides").and_then(Value::as_str).unwrap_or("server").into();
        *self.inner.macro_origin.lock().unwrap() =
            data.get("macro_origin").and_then(Value::as_str).unwrap_or("").into();
        self.recovery().live.insert(session_id);
        Ok(())
    }

    async fn recovery_post(&self, path: &str, body: &Value) -> Result<Value, String> {
        let response = self
            .inner
            .http
            .post(format!("{}{path}", self.inner.base))
            .headers(self.headers())
            .json(body)
            .timeout(Duration::from_secs(15))
            .send()
            .await
            .map_err(|_| "서버에 연결하지 못했어요. 연결을 확인한 뒤 다시 이어서 실행하세요.".to_string())?;
        let status = response.status();
        let data = response.json::<Value>().await.map_err(|_| "서버의 복구 응답을 읽지 못했어요.".to_string())?;
        if !status.is_success() {
            return Err(data
                .get("detail")
                .and_then(Value::as_str)
                .unwrap_or("실행을 복구하지 못했어요. 웹에서 실행 상태를 확인하세요.")
                .to_string());
        }
        Ok(data)
    }

    pub async fn recovery_preview(&self, session_id: i64, recovery_id: &str) -> Result<Value, String> {
        self.recovery_post("/api/runner/recovery", &json!({"session_id": session_id,"recovery_id": recovery_id})).await
    }

    pub async fn resume_session(&self, payload: &Value) -> Result<Value, String> {
        let data = self.recovery_post("/api/runner/resume", payload).await?;
        self.bind_session(&data).map_err(|e| e.to_string())?;
        Ok(data)
    }
}
