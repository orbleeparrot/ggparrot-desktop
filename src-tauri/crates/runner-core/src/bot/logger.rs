//! 로그를 화면과 서버(실행 기록) 양쪽에 남김. 서버 사본과 화면 모두 실제 키를 가림.

use super::*;

/// 로그를 화면과 서버(실행 기록) 양쪽에 남김. 서버 사본은 실제 키를 가림.
#[derive(Clone)]
pub struct Logger {
    tx: mpsc::UnboundedSender<BotEvent>,
    server: Option<ServerClient>,
    credentials: Credentials,
    active_op: Arc<Mutex<String>>,
}

impl Logger {
    pub fn new(tx: mpsc::UnboundedSender<BotEvent>, server: Option<ServerClient>, credentials: Credentials) -> Self {
        Logger { tx, server, credentials, active_op: Arc::new(Mutex::new(String::new())) }
    }

    /// 화면에만. 화면·활동 복사에도 실제 키 값은 남기지 않음.
    pub fn ui(&self, msg: impl Into<String>) {
        let _ = self.tx.send(BotEvent::Log(crate::connection::mask_secrets(&msg.into(), &self.credentials)));
    }

    fn safe_details(&self, details: &Map<String, Value>) -> Map<String, Value> {
        details
            .iter()
            .map(|(k, v)| match v {
                Value::String(s) => (k.clone(), Value::String(safe_preflight_reason(s, &self.credentials))),
                _ => (k.clone(), v.clone()),
            })
            .collect()
    }

    pub fn log_full(
        &self,
        msg: &str,
        kind: Option<&str>,
        severity: &str,
        event_name: &str,
        operation_id: &str,
        details: &Map<String, Value>,
    ) {
        self.ui(msg);
        if let Some(server) = &self.server {
            let safe = safe_preflight_reason(msg, &self.credentials);
            let op =
                if operation_id.is_empty() { self.active_op.lock().unwrap().clone() } else { operation_id.to_string() };
            server.push_event(
                kind.unwrap_or_else(|| event_kind(msg)),
                &safe,
                severity,
                event_name,
                &op,
                &self.safe_details(details),
            );
        }
    }

    pub fn log(&self, msg: impl AsRef<str>) {
        self.log_full(msg.as_ref(), None, "UNKNOWN", "runner.message", "", &Map::new());
    }

    /// 웹에만 남기는 구조화 단계(화면에는 쓰지 않음).
    pub fn telemetry(
        &self,
        kind: &str,
        msg: &str,
        event_name: &str,
        severity: &str,
        operation_id: &str,
        details: &Map<String, Value>,
    ) {
        if let Some(server) = &self.server {
            server.push_event(
                kind,
                &safe_preflight_reason(msg, &self.credentials),
                severity,
                event_name,
                operation_id,
                &self.safe_details(details),
            );
        }
    }

    pub fn set_operation(&self, op: &str) {
        *self.active_op.lock().unwrap() = op.to_string();
    }
    pub fn operation(&self) -> String {
        self.active_op.lock().unwrap().clone()
    }

    /// 활동 기록 한 줄(화면에만). 오류 문장이 섞여 들어오므로 원문 로그처럼 실제 키 값을 가림.
    pub fn activity(
        &self,
        kind: &'static str,
        title: impl Into<String>,
        detail: impl Into<String>,
        raw: impl Into<String>,
        tone: &'static str,
    ) {
        let mask = |s: String| crate::connection::mask_secrets(&s, &self.credentials);
        let _ = self.tx.send(BotEvent::Activity(ActivityEvent {
            kind,
            title: mask(title.into()),
            detail: mask(detail.into()),
            raw: mask(raw.into()),
            tone,
        }));
    }

    pub fn as_log_fn(&self) -> LogFn {
        let me = self.clone();
        Arc::new(move |msg: String| me.log(&msg))
    }
}
