//! 바이낸스 REST 호출 — 서명·시계 맞추기·요청 한도·오류 모양.
use hmac::{Hmac, KeyInit, Mac};
use serde_json::Value;
use sha2::Sha256;
use std::sync::atomic::Ordering;
use std::time::Duration;

use super::BinanceBroker;
use crate::brokers::{cool, cooling, network_error, py_str, retry_after_secs, BrokerError};
use crate::clock::unix_ms;

/// 바이낸스 요청 하나의 제한 — 멈춘 연결이 손절·하트비트·정지를 붙잡지 않게.
const BINANCE_TIMEOUT: Duration = Duration::from_secs(10);

impl BinanceBroker {
    /// 거래소 시계와의 차이를 맞춤. 실패하면 그대로 둠(다음 -1021에서 다시 시도).
    pub(super) async fn sync_time(&self) {
        let path = if self.futures() { "/fapi/v1/time" } else { "/api/v3/time" };
        let before = unix_ms() as i64;
        if let Ok(v) = self.call_once(reqwest::Method::GET, self.futures(), path, &[], false).await {
            if let Some(server) = v.get("serverTime").and_then(Value::as_i64) {
                let local = (before + unix_ms() as i64) / 2;
                self.time_offset.store(server - local, Ordering::Relaxed);
            }
        }
    }

    fn sign(&self, payload: &str) -> String {
        let mut mac = Hmac::<Sha256>::new_from_slice(self.api_secret.as_bytes()).expect("hmac accepts any key");
        mac.update(payload.as_bytes());
        hex::encode(mac.finalize().into_bytes())
    }

    /// REST 호출. 서명 요청이 시계 차이(-1021)로 거절되면 시계를 맞추고 한 번만 다시 보냄
    /// (-1021은 거래소가 요청을 처리하지 않았다는 뜻이라 같은 client id로 다시 보내도 안전함).
    async fn call(
        &self,
        method: reqwest::Method,
        futures: bool,
        path: &str,
        params: &[(&str, String)],
        signed: bool,
    ) -> Result<Value, BrokerError> {
        match self.call_once(method.clone(), futures, path, params, signed).await {
            Err(BrokerError::Api { name, .. }) if signed && name == "-1021" => {
                self.log("바이낸스와 PC 시계가 맞지 않아 시계를 맞춘 뒤 다시 보냈어요.".into());
                self.sync_time().await;
                self.call_once(method, futures, path, params, signed).await
            }
            other => other,
        }
    }

    /// REST 호출 하나. 2xx가 아니면 `{"code","msg"}`를 Api 오류로(name = 코드 글자).
    async fn call_once(
        &self,
        method: reqwest::Method,
        futures: bool,
        path: &str,
        params: &[(&str, String)],
        signed: bool,
    ) -> Result<Value, BrokerError> {
        let base = if futures { &self.futures_base } else { &self.spot_base };
        // 요청 한도로 쉬는 중이면 보내지 않음(보내지 않았으니 주문은 분명히 '받지 않음').
        // 같은 주소로 나가는 다른 실행이 걸린 한도도 함께 지킴.
        if cooling(base).is_some() {
            return Err(BrokerError::Api {
                status: 429,
                name: "-1003".into(),
                message: "바이낸스 요청 한도로 잠시 쉬는 중이에요".into(),
            });
        }
        let mut query = {
            let mut ser = url::form_urlencoded::Serializer::new(String::new());
            for (k, v) in params {
                ser.append_pair(k, v);
            }
            if signed {
                let ts = unix_ms() as i64 + self.time_offset.load(Ordering::Relaxed);
                ser.append_pair("timestamp", &ts.to_string());
                ser.append_pair("recvWindow", "5000");
            }
            ser.finish()
        };
        if signed {
            let signature = self.sign(&query);
            query.push_str("&signature=");
            query.push_str(&signature);
        }
        let url = if query.is_empty() { format!("{base}{path}") } else { format!("{base}{path}?{query}") };
        let resp = self
            .http
            .request(method, url)
            .header("X-MBX-APIKEY", &self.api_key)
            .timeout(BINANCE_TIMEOUT)
            .send()
            .await
            .map_err(network_error)?;
        let status = resp.status().as_u16();
        let retry_after = retry_after_secs(resp.headers());
        let text = resp.text().await.map_err(network_error)?;
        if status == 429 || status == 418 {
            // 주문 개수 한도(-1015)는 계정 단위임 — 이 주소로 나가는 모든 실행의 시세 조회까지 쉬게 하지 않음.
            let order_rate = serde_json::from_str::<Value>(&text)
                .ok()
                .and_then(|b| b.get("code").and_then(Value::as_i64))
                .is_some_and(|c| c == -1015);
            if !order_rate {
                // Retry-After(초)만큼 쉼. 없으면 429는 30초, 418(IP 차단)은 2분.
                let wait = retry_after.unwrap_or(if status == 418 { 120 } else { 30 }).clamp(1, 3600);
                cool(base, wait);
                self.log(format!("바이낸스 요청 한도에 걸려 {wait}초 동안 요청을 쉬어요(HTTP {status})."));
            }
        }
        if !(200..300).contains(&status) {
            let (code, message) = match serde_json::from_str::<Value>(&text) {
                Ok(Value::Object(body)) => (
                    body.get("code").map(py_str).unwrap_or_else(|| "0".into()),
                    body.get("msg").map(py_str).unwrap_or_default(),
                ),
                _ => ("0".into(), format!("Invalid JSON error message from Binance: {text}")),
            };
            return Err(BrokerError::Api { status, name: code, message });
        }
        serde_json::from_str(&text)
            .map_err(|_| BrokerError::Other(format!("BinanceRequestException: Invalid Response: {text}")))
    }

    pub(super) async fn get(
        &self,
        futures: bool,
        path: &str,
        params: &[(&str, String)],
        signed: bool,
    ) -> Result<Value, BrokerError> {
        self.call(reqwest::Method::GET, futures, path, params, signed).await
    }

    pub(super) async fn post(
        &self,
        futures: bool,
        path: &str,
        params: &[(&str, String)],
    ) -> Result<Value, BrokerError> {
        self.call(reqwest::Method::POST, futures, path, params, true).await
    }
}
