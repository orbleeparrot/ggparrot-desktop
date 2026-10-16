//! 서버 연동 클라이언트 — 회원 키로 인증하고 구동 상태·명령 결과·실행 기록만 보냄(거래소 키는 절대 안 보냄).
//! 세션 시작·heartbeat·종료 보고, 서버 오류 문장·재시도 대기 읽기, 티켓 교환·다운로드 정보.

use serde_json::{json, Map, Value};
use std::collections::{BTreeMap, HashMap, HashSet};
use std::fmt;
use std::io::Write;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use crate::brokers::LogFn;
use crate::outbox::{self, EventOutbox};
use crate::protocol::PROTOCOL_CLAIM_PATH;
use crate::SERVER_LINK_LEVEL;

pub const START_ATTEMPTS: usize = 4;
pub const MAX_RETRIES: usize = 3;
/// heartbeat 한 번에 실어 보내는 최대 기록 수.
pub const EVENT_BUFFER_MAX: usize = 100;

const ALLOWED_DETAIL_KEYS: &[&str] = &[
    "command_id",
    "action",
    "reason",
    "status",
    "raw_status",
    "command_seq",
    "signal_price",
    "notional_frac",
    "qty_frac",
    "executed_qty",
    "fill_price",
    "client_order_id",
    "exchange_order_id",
    "acquired_qty",
    "fees_known",
    "error",
    "mode",
    "exchange",
    "symbol",
    "verification",
    "ack_source",
    "outcome",
    "position_uncertain",
    "reconciliation_scope",
    "requested_qty",
    "requested_notional",
    "quote_currency",
    "reduce_only",
    "closing",
];

fn truncate_chars(s: &str, n: usize) -> String {
    s.chars().take(n).collect()
}

/// 시작 요청이 응답을 기다리다 끊겼을 때 [`StartError::Network`] 앞에 붙는 표시.
pub const START_TIMEOUT_TAG: &str = "timeout: ";

/// 서버가 받고서 거절한 요청. 화면에는 서버가 준 이유(detail)만 보임.
#[derive(Clone, Debug)]
pub struct ServerRequestError {
    pub status: u16,
    pub detail: String,
}

impl fmt::Display for ServerRequestError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.detail.is_empty() {
            write!(f, "서버가 요청을 받지 않았어요(HTTP {}). 잠시 뒤 다시 시작해 보세요.", self.status)
        } else {
            // 서버 문구가 이유를 이미 말함 — 상태 코드는 덧붙이지 않음.
            f.write_str(&self.detail)
        }
    }
}

#[derive(Clone, Debug)]
pub enum StartError {
    Server(ServerRequestError),
    Network(String),
}

impl fmt::Display for StartError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            StartError::Server(e) => write!(f, "{e}"),
            StartError::Network(m) => write!(f, "{m}"),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Default)]
pub struct HeartbeatReply {
    /// continue | stop_only | close_and_stop
    pub action: String,
    pub commands: Vec<Value>,
    pub offline: bool,
    /// 서버가 회원 키를 받지 않음(401·403) — 재시도로는 풀리지 않음.
    pub auth_failed: bool,
    /// 서버가 요청 자체를 거절함(408·429를 뺀 4xx, 읽을 수 없는 응답) — 그 HTTP 상태. 0이면 아님.
    /// 기다린다고 풀리지 않을 수 있어 일시적인 끊김과 따로 알림.
    pub rejected: u16,
    /// 서버가 종료를 지시한 이유(세션이 없어짐·이미 끝남 등). 비어 있으면 웹에서 누른 종료임.
    pub reason: String,
}

impl HeartbeatReply {
    pub fn idle() -> Self {
        HeartbeatReply { action: "continue".into(), ..Default::default() }
    }
    fn offline(auth_failed: bool, rejected: u16) -> Self {
        HeartbeatReply { action: "continue".into(), offline: true, auth_failed, rejected, ..Default::default() }
    }
}

/// 하트비트 하나의 제한 — 파이썬 실행기와 같은 10초.
/// - 서버가 명령을 실어 답하는 중에 끊으면 그 답(명령)을 놓치고 다음 하트비트까지 늦어짐 — 느린 서버에도 답을 받게.
/// - 실행 루프 안에서 기다리지만 손절 확인이 길게 밀리지 않음: 실패 뒤에는 백오프로 하트비트를 건너뛰고(`heartbeat_backoff`)
///   손절은 매 틱 따로 봄.
const HEARTBEAT_TIMEOUT: Duration = Duration::from_secs(10);

/// 연속 실패 n번째 뒤 다음 하트비트까지 쉬는 시간: 5·10·20·40·60초(+ 최대 20% 흔들기).
/// 서버가 내려간 동안 여러 실행이 같은 박자로 두드리지 않게 함. 손절은 그동안에도 매 틱 봄.
pub fn heartbeat_backoff(failures: u32, jitter: f64) -> Duration {
    if failures == 0 {
        return Duration::ZERO;
    }
    let base = (5.0 * 2f64.powi(failures.min(5) as i32 - 1)).min(60.0);
    Duration::from_secs_f64(base * (1.0 + 0.2 * jitter.clamp(0.0, 1.0)))
}

async fn server_detail(body: &str) -> String {
    serde_json::from_str::<Value>(body)
        .ok()
        .and_then(|v| v.get("detail").and_then(Value::as_str).map(|s| truncate_chars(s.trim(), 200)))
        .unwrap_or_default()
}

fn retry_after_seconds(headers: &reqwest::header::HeaderMap) -> f64 {
    let value = headers
        .get("Retry-After")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.trim().parse::<f64>().ok())
        .unwrap_or(1.0);
    value.clamp(0.5, 3.0)
}

struct EventState {
    /// 디스크에 못 쓴 기록(메모리 보관).
    events: Vec<Value>,
    event_sessions: HashMap<String, i64>,
    memory_created_ms: HashMap<String, i64>,
    memory_payload_bytes: HashMap<String, usize>,
    memory_loss_counts: BTreeMap<String, u64>,
    memory_flushed_loss_counts: BTreeMap<String, u64>,
    memory_noticed_loss_counts: BTreeMap<String, u64>,
    last_memory_notice_ms: i64,
    unassigned_event_keys: HashSet<String>,
    warning_emitted: bool,
}

struct Inner {
    base: String,
    key: String,
    http: reqwest::Client,
    outbox: Arc<EventOutbox>,
    namespace: String,
    session_id: Mutex<Option<i64>>,
    macro_origin: Mutex<String>,
    state: Mutex<EventState>,
    on_event_warning: Mutex<Option<LogFn>>,
    unsent_acks: Mutex<Vec<Value>>,
    /// 하트비트 연속 실패 수와 다음 시도 시각(백오프).
    heartbeat_failures: Mutex<(u32, Option<Instant>)>,
    echo_stdout: bool,
}

#[derive(Clone)]
pub struct ServerClient {
    inner: Arc<Inner>,
}

impl ServerClient {
    pub fn new(runner_key: &str, base: &str, outbox: Arc<EventOutbox>, http: reqwest::Client) -> Self {
        let key = runner_key.trim().to_string();
        let base = base.trim_end_matches('/').to_string();
        let zero = || BTreeMap::from([("memory_expired".to_string(), 0u64), ("memory_capacity".to_string(), 0u64)]);
        ServerClient {
            inner: Arc::new(Inner {
                namespace: outbox::namespace(&base, &key),
                base,
                key,
                http,
                outbox,
                session_id: Mutex::new(None),
                macro_origin: Mutex::new(String::new()),
                state: Mutex::new(EventState {
                    events: vec![],
                    event_sessions: HashMap::new(),
                    memory_created_ms: HashMap::new(),
                    memory_payload_bytes: HashMap::new(),
                    memory_loss_counts: zero(),
                    memory_flushed_loss_counts: zero(),
                    memory_noticed_loss_counts: zero(),
                    last_memory_notice_ms: 0,
                    unassigned_event_keys: HashSet::new(),
                    warning_emitted: false,
                }),
                on_event_warning: Mutex::new(None),
                unsent_acks: Mutex::new(vec![]),
                heartbeat_failures: Mutex::new((0, None)),
                // 배포판은 기록마다 표준 출력에 쓰지 않음(창 앱이라 보는 사람이 없고, 실행이 많으면 쓰기만 늘어남).
                echo_stdout: cfg!(debug_assertions),
            }),
        }
    }

    pub fn base(&self) -> &str {
        &self.inner.base
    }
    pub fn session_id(&self) -> Option<i64> {
        *self.inner.session_id.lock().unwrap()
    }
    pub fn macro_origin(&self) -> String {
        self.inner.macro_origin.lock().unwrap().clone()
    }
    pub fn set_event_warning(&self, log: LogFn) {
        *self.inner.on_event_warning.lock().unwrap() = Some(log);
    }
    pub fn has_event_warning(&self) -> bool {
        self.inner.on_event_warning.lock().unwrap().is_some()
    }

    fn headers(&self) -> reqwest::header::HeaderMap {
        let mut h = reqwest::header::HeaderMap::new();
        if let Ok(v) = reqwest::header::HeaderValue::from_str(&self.inner.key) {
            h.insert("X-Runner-Key", v);
        }
        h
    }

    // --- 실행 기록 -----------------------------------------------------------
    /// 기록 하나를 만듦. 처음 만들 때 붙인 고유 번호를 재전송에도 그대로 씀.
    pub fn push_event(
        &self,
        kind: &str,
        message: &str,
        severity: &str,
        event_name: &str,
        operation_id: &str,
        details: &Map<String, Value>,
    ) -> Value {
        let key = &self.inner.key;
        let mut snapshot = Map::new();
        for (k, v) in details {
            if !ALLOWED_DETAIL_KEYS.contains(&k.as_str()) {
                continue;
            }
            match v {
                Value::String(s) => {
                    snapshot.insert(k.clone(), Value::String(truncate_chars(&outbox::redact(s, key), 600)));
                }
                Value::Number(_) | Value::Bool(_) => {
                    snapshot.insert(k.clone(), v.clone());
                }
                _ => {}
            }
        }
        let severity =
            if ["DEBUG", "INFO", "WARN", "ERROR", "UNKNOWN"].contains(&severity) { severity } else { "UNKNOWN" };
        let event = json!({
            "ts": chrono::Utc::now().format("%Y-%m-%dT%H:%M:%SZ").to_string(),
            "kind": kind,
            "message": truncate_chars(&outbox::redact(message, key), 300),
            "event_key": format!("runner:{}", uuid::Uuid::new_v4().simple()),
            "severity": severity,
            "component": "runner",
            "event_name": truncate_chars(&outbox::redact(event_name, key), 80),
            "operation_id": truncate_chars(&outbox::redact(operation_id, key), 120),
            "details": Value::Object(snapshot),
        });
        let session_id = self.session_id().unwrap_or(0);
        let event_key = event["event_key"].as_str().unwrap_or_default().to_string();
        {
            let mut st = self.inner.state.lock().unwrap();
            if session_id == 0 {
                st.unassigned_event_keys.insert(event_key.clone());
            }
            if self.inner.outbox.append(&self.inner.namespace, session_id, &event).is_err() {
                // 디스크가 막혀도 매매 프로세스가 끝없이 커지면 안 됨 — 보관 한도 밖은 세어서 버림.
                let bytes = serde_json::to_vec(&event).map(|b| b.len()).unwrap_or(0);
                st.events.push(event.clone());
                st.event_sessions.insert(event_key.clone(), session_id);
                st.memory_created_ms.insert(event_key.clone(), self.inner.outbox.now_ms());
                st.memory_payload_bytes.insert(event_key.clone(), bytes);
                self.storage_warning_locked(&mut st);
            }
            self.maintain_locked(&mut st);
        }
        if self.inner.echo_stdout {
            let _ = writeln!(std::io::stdout(), "{}", event);
        }
        event
    }

    fn direct_warning(&self, message: &str) {
        let log = self.inner.on_event_warning.lock().unwrap().clone();
        if let Some(log) = log {
            log(message.to_string());
        }
        let _ = writeln!(std::io::stderr(), "{message}");
    }

    fn storage_warning_locked(&self, st: &mut EventState) {
        if st.warning_emitted {
            return;
        }
        st.warning_emitted = true;
        self.direct_warning(
            "활동 기록을 이 기기에 저장하지 못했어요. 아직 서버로 보내지 못한 기록은 앱을 끄면 사라질 수 있어요.",
        );
    }

    fn remove_memory_keys(st: &mut EventState, keys: &HashSet<String>, forget_unassigned: bool) {
        st.events.retain(|e| !keys.contains(e["event_key"].as_str().unwrap_or_default()));
        for k in keys {
            st.event_sessions.remove(k);
            st.memory_created_ms.remove(k);
            st.memory_payload_bytes.remove(k);
            if forget_unassigned {
                st.unassigned_event_keys.remove(k);
            }
        }
    }

    /// 메모리 보관 정리 → 디스크 손실 기록 → 손실 알림. 기록을 쓰거나 보낼 때마다 부름.
    fn maintain_locked(&self, st: &mut EventState) {
        let now = self.inner.outbox.now_ms();
        Self::trim_memory(st, now);
        let storage_failed = !self.flush_to_disk_locked(st);
        let notice = self.loss_notice_locked(st, now, storage_failed);
        if notice.values().any(|v| *v > 0) {
            let get = |k: &str| notice.get(k).copied().unwrap_or(0);
            let count: u64 = notice.values().sum();
            let expired_count = get("disk_expired") + get("memory_expired");
            let capacity_count = get("disk_capacity") + get("memory_capacity");
            self.direct_warning(&format!(
                "서버로 보내지 못한 활동 기록 {count}건을 더 보관할 수 없어 지웠어요\
                 (7일 지남 {expired_count}건 · 용량 초과 {capacity_count}건). 이 기록은 서버에 남지 않았을 수 있어요."
            ));
        }
    }

    /// 메모리에 둔 기록 중 기한(7일)이 지난 것과 개수·바이트 한도를 넘는 오래된 것을 지우고 그 수를 셈.
    fn trim_memory(st: &mut EventState, now: i64) {
        let ttl_ms = outbox::PENDING_TTL_SECONDS * 1000;
        let expired: HashSet<String> = st
            .events
            .iter()
            .filter_map(|e| e["event_key"].as_str().map(str::to_string))
            .filter(|k| st.memory_created_ms.get(k).copied().unwrap_or(now) <= now - ttl_ms)
            .collect();
        let expired_n = expired.len() as u64;
        Self::remove_memory_keys(st, &expired, true);
        *st.memory_loss_counts.entry("memory_expired".into()).or_default() += expired_n;

        let mut size: usize = st.memory_payload_bytes.values().sum();
        let mut overflow = HashSet::new();
        let total = st.events.len();
        for e in &st.events {
            if total - overflow.len() <= outbox::MEMORY_MAX_ROWS && size <= outbox::MEMORY_MAX_BYTES {
                break;
            }
            let k = e["event_key"].as_str().unwrap_or_default().to_string();
            size = size.saturating_sub(st.memory_payload_bytes.get(&k).copied().unwrap_or(0));
            overflow.insert(k);
        }
        let overflow_n = overflow.len() as u64;
        Self::remove_memory_keys(st, &overflow, true);
        *st.memory_loss_counts.entry("memory_capacity".into()).or_default() += overflow_n;
    }

    /// 디스크 보관함 유지 보수, 아직 옮기지 않은 메모리 손실 수 기록, 세션 없는 기록 키 정리. 하나라도 실패하면 거짓.
    fn flush_to_disk_locked(&self, st: &mut EventState) -> bool {
        if self.inner.outbox.try_maintain(false).is_err() {
            return false;
        }
        let deltas: BTreeMap<String, u64> = st
            .memory_loss_counts
            .iter()
            .map(|(r, c)| (r.clone(), c.saturating_sub(st.memory_flushed_loss_counts.get(r).copied().unwrap_or(0))))
            .collect();
        if deltas.values().any(|v| *v > 0) {
            if self.inner.outbox.try_record_memory_loss(&deltas).is_err() {
                return false;
            }
            st.memory_flushed_loss_counts = st.memory_loss_counts.clone();
        }
        if !st.unassigned_event_keys.is_empty() {
            let keys: Vec<String> = st.unassigned_event_keys.iter().cloned().collect();
            let Ok(disk) = self.inner.outbox.existing_unassigned_keys(&self.inner.namespace, &keys) else {
                return false;
            };
            let keep: HashSet<String> = disk.into_iter().chain(st.event_sessions.keys().cloned()).collect();
            st.unassigned_event_keys.retain(|k| keep.contains(k));
        }
        true
    }

    /// 이번에 알릴 손실 수(사유별). 디스크가 되면 디스크 기록의 1분 단위 알림을, 막혔으면 저장 실패 경고와 함께
    /// 메모리에서 센 수를 1분에 한 번 씀. 알릴 것이 없으면 0뿐이거나 빈 맵.
    fn loss_notice_locked(&self, st: &mut EventState, now: i64, storage_failed: bool) -> BTreeMap<String, u64> {
        let disk_notice = if storage_failed { None } else { self.inner.outbox.try_take_loss_notice().ok() };
        if let Some(notice) = disk_notice {
            return notice;
        }
        self.storage_warning_locked(st);
        let since = now - st.last_memory_notice_ms;
        if (0..outbox::MAINTENANCE_SECONDS * 1000).contains(&since) {
            return BTreeMap::new();
        }
        let notice: BTreeMap<String, u64> = st
            .memory_loss_counts
            .iter()
            .map(|(r, c)| (r.clone(), c.saturating_sub(st.memory_noticed_loss_counts.get(r).copied().unwrap_or(0))))
            .collect();
        if notice.values().any(|v| *v > 0) {
            st.last_memory_notice_ms = now;
            st.memory_noticed_loss_counts = st.memory_loss_counts.clone();
        }
        notice
    }

    /// 보낼 기록 한 묶음(지우지 않음 — 서버가 저장했다고 답한 것만 ack로 지움).
    fn peek_events(&self, session_id: Option<i64>) -> Vec<Value> {
        let current = self.session_id().unwrap_or(0);
        let session_id = session_id.unwrap_or(current);
        let mut st = self.inner.state.lock().unwrap();
        self.maintain_locked(&mut st);
        let ns = &self.inner.namespace;
        let mut ok = true;
        if current != 0 {
            let keys: Vec<String> = st.unassigned_event_keys.iter().cloned().collect();
            if self.inner.outbox.bind_unassigned(ns, current, &keys).is_ok() {
                st.unassigned_event_keys.clear();
            } else {
                ok = false;
            }
        }
        if ok {
            for e in st.events.clone() {
                let k = e["event_key"].as_str().unwrap_or_default().to_string();
                let original = match st.event_sessions.get(&k).copied().unwrap_or(0) {
                    0 => current,
                    s => s,
                };
                let created = st.memory_created_ms.get(&k).copied();
                if self.inner.outbox.append_at(ns, original, &e, created).is_err() {
                    continue;
                }
                Self::remove_memory_keys(&mut st, &HashSet::from([k]), original != 0);
            }
        }
        let durable = if ok {
            let keys: Option<Vec<String>> =
                if session_id == 0 { Some(st.unassigned_event_keys.iter().cloned().collect()) } else { None };
            match self.inner.outbox.peek(ns, session_id, outbox::MAX_BATCH, keys.as_deref()) {
                Ok(v) => v,
                Err(_) => {
                    self.storage_warning_locked(&mut st);
                    vec![]
                }
            }
        } else {
            self.storage_warning_locked(&mut st);
            vec![]
        };
        self.maintain_locked(&mut st);
        let durable_keys: HashSet<String> =
            durable.iter().filter_map(|e| e["event_key"].as_str().map(str::to_string)).collect();
        let pending: Vec<Value> = st
            .events
            .iter()
            .filter(|e| {
                let k = e["event_key"].as_str().unwrap_or_default();
                let s = match st.event_sessions.get(k).copied().unwrap_or(0) {
                    0 => current,
                    s => s,
                };
                !durable_keys.contains(k) && s == session_id
            })
            .cloned()
            .collect();
        let mut all = durable;
        all.extend(pending);
        all.truncate(EVENT_BUFFER_MAX);
        outbox::wire_batch(all, &Value::Null)
    }

    /// 서버가 `accepted_event_keys`로 저장을 확인한 것만 지움. HTTP 200은 저장 확인이 아님.
    fn ack_events(&self, events: &[Value], data: &Value, session_id: Option<i64>) {
        let Some(accepted) = data.get("accepted_event_keys").and_then(Value::as_array) else {
            return;
        };
        let sent: HashSet<&str> = events.iter().filter_map(|e| e["event_key"].as_str()).collect();
        let keys: Vec<String> = accepted
            .iter()
            .filter_map(Value::as_str)
            .filter(|k| sent.contains(k))
            .take(EVENT_BUFFER_MAX)
            .map(str::to_string)
            .collect();
        if keys.is_empty() {
            return;
        }
        let session_id = session_id.unwrap_or_else(|| self.session_id().unwrap_or(0));
        let mut st = self.inner.state.lock().unwrap();
        if self.inner.outbox.acknowledge(&self.inner.namespace, session_id, &keys).is_err() {
            self.storage_warning_locked(&mut st);
        }
        Self::remove_memory_keys(&mut st, &keys.into_iter().collect(), true);
    }

    // --- 세션 -----------------------------------------------------------------
    /// 세션 시작. 서버가 혼잡(429·503)이면 Retry-After(0.5~3초)를 지켜 최대 4번 보냄.
    pub async fn start(&self, payload: &Value) -> Result<Value, StartError> {
        let url = format!("{}/api/runner/start", self.inner.base);
        let mut attempt = 0;
        let body = loop {
            let r = self
                .inner
                .http
                .post(&url)
                .headers(self.headers())
                .json(payload)
                .timeout(Duration::from_secs(15))
                .send()
                .await
                .map_err(|e| {
                    // 응답을 기다리다 끊긴 것은 따로 표시함 — 서버에는 세션이 만들어졌을 수 있음.
                    StartError::Network(if e.is_timeout() { format!("{START_TIMEOUT_TAG}{e}") } else { e.to_string() })
                })?;
            let status = r.status().as_u16();
            if status < 400 {
                break r.text().await.map_err(|e| StartError::Network(e.to_string()))?;
            }
            if (status == 429 || status == 503) && attempt + 1 < START_ATTEMPTS {
                tokio::time::sleep(Duration::from_secs_f64(retry_after_seconds(r.headers()))).await;
                attempt += 1;
                continue;
            }
            let text = r.text().await.unwrap_or_default();
            return Err(StartError::Server(ServerRequestError { status, detail: server_detail(&text).await }));
        };
        let data: Value = serde_json::from_str(&body).map_err(|e| StartError::Network(e.to_string()))?;
        let Some(session_id) = data.get("session_id").and_then(Value::as_i64) else {
            // 세션 번호가 없으면 하트비트를 보낼 곳이 없음 — '실행 중'인데 아무것도 하지 않는 실행이 되지 않게 막음.
            return Err(StartError::Server(ServerRequestError {
                status: 200,
                detail: "서버가 실행 세션 번호를 주지 않았어요. 잠시 뒤 다시 시작해 보세요.".into(),
            }));
        };
        *self.inner.session_id.lock().unwrap() = Some(session_id);
        self.recovery().live.insert(session_id);
        *self.inner.macro_origin.lock().unwrap() = match data.get("macro_origin") {
            Some(Value::String(s)) => s.clone(),
            _ => String::new(),
        };
        self.schedule_event_recovery(false);
        Ok(data)
    }

    /// 다음 하트비트를 바로 보내게 함 — 종료 직전의 마지막 보고, 잠자기에서 깨어난 직후.
    pub fn reset_backoff(&self) {
        *self.inner.heartbeat_failures.lock().unwrap() = (0, None);
    }

    /// 아직 서버에 올리지 못한 명령 결과가 있는지(종료 전에 한 번 더 보낼지).
    pub fn has_unsent_acks(&self) -> bool {
        !self.inner.unsent_acks.lock().unwrap().is_empty()
    }

    /// 상태와 명령 결과(acks)를 올리고 {action, commands}를 받음.
    /// 네트워크 오류면 action=continue·commands=[]·offline — ack는 다음 heartbeat 앞머리에 다시 실음.
    pub async fn heartbeat(&self, snapshot: &Value, acks: Vec<Value>) -> HeartbeatReply {
        let Some(session_id) = self.session_id() else {
            return HeartbeatReply::idle();
        };
        // 백오프 중이면 보내지 않고 ack만 다음으로 미룸.
        let waiting =
            matches!(*self.inner.heartbeat_failures.lock().unwrap(), (_, Some(next)) if Instant::now() < next);
        if waiting {
            self.inner.unsent_acks.lock().unwrap().extend(acks);
            return HeartbeatReply::offline(false, 0);
        }
        let mut body = snapshot.as_object().cloned().unwrap_or_default();
        body.insert("session_id".into(), json!(session_id));
        let events = self.peek_events(None);
        let mut pending: Vec<Value> = std::mem::take(&mut *self.inner.unsent_acks.lock().unwrap());
        pending.extend(acks);
        body.insert("acks".into(), Value::Array(pending.clone()));
        body.insert("events".into(), Value::Array(vec![]));
        let events = outbox::wire_batch(events, &Value::Object(body.clone()));
        body.insert("events".into(), Value::Array(events.clone()));
        let result = async {
            let r = self
                .inner
                .http
                .post(format!("{}/api/runner/heartbeat", self.inner.base))
                .headers(self.headers())
                .json(&Value::Object(body))
                .timeout(HEARTBEAT_TIMEOUT)
                .send()
                .await
                .map_err(|_| 0u16)?;
            let status = r.status().as_u16();
            if !r.status().is_success() {
                return Err(status);
            }
            r.json::<Value>().await.map_err(|_| status)
        }
        .await;
        match result {
            Ok(data) => {
                *self.inner.heartbeat_failures.lock().unwrap() = (0, None);
                self.ack_events(&events, &data, None);
                self.schedule_event_recovery(false);
                HeartbeatReply {
                    action: data.get("action").and_then(Value::as_str).unwrap_or("continue").to_string(),
                    commands: data.get("commands").and_then(Value::as_array).cloned().unwrap_or_default(),
                    reason: data.get("reason").and_then(Value::as_str).unwrap_or_default().trim().to_string(),
                    ..Default::default()
                }
            }
            // 서버가 이 세션을 끝냄(410) — 명령 결과는 버리지 않고 종료 보고에 실음.
            Err(410) => {
                *self.inner.unsent_acks.lock().unwrap() = pending;
                HeartbeatReply {
                    action: "stop_only".into(),
                    reason: "서버가 이 실행 세션을 끝냈어요.".into(),
                    ..Default::default()
                }
            }
            Err(status) => {
                *self.inner.unsent_acks.lock().unwrap() = pending;
                let mut f = self.inner.heartbeat_failures.lock().unwrap();
                f.0 = f.0.saturating_add(1);
                let jitter: f64 = rand::random();
                f.1 = Some(Instant::now() + heartbeat_backoff(f.0, jitter));
                let auth = status == 401 || status == 403;
                // 4xx(408·429 제외)와 성공 코드인데 읽을 수 없는 응답은 기다려도 풀리지 않을 수 있음.
                let permanent = ((400..500).contains(&status) && !matches!(status, 401 | 403 | 408 | 429))
                    || (200..300).contains(&status);
                let rejected = if permanent { status } else { 0 };
                HeartbeatReply::offline(auth, rejected)
            }
        }
    }

    /// 종료 보고. 같은 종료 스냅샷을 다시 보내도 안전함(3회, 4xx는 408·429 말고는 바로 포기).
    pub async fn stopped(&self, status: &str, note: &str, snapshot: Option<&Value>) -> bool {
        let Some(session_id) = self.session_id() else {
            return true;
        };
        // 끝나는 세션 — 종료 보고가 실패해도 남은 기록은 이제 다시 보내기가 맡음.
        self.recovery().live.remove(&session_id);
        let mut body = json!({
            "session_id": session_id, "status": status, "note": note,
            "snapshot": snapshot.cloned().unwrap_or(Value::Null), "events": [],
        });
        let mut events = self.peek_events(None);
        for attempt in 0..MAX_RETRIES {
            events = outbox::wire_batch(events, &body);
            body["events"] = Value::Array(events.clone());
            let resp = self
                .inner
                .http
                .post(format!("{}/api/runner/stopped", self.inner.base))
                .headers(self.headers())
                .json(&body)
                .timeout(Duration::from_secs(10))
                .send()
                .await;
            if let Ok(r) = resp {
                let code = r.status().as_u16();
                if (400..500).contains(&code) && code != 408 && code != 429 {
                    return false;
                }
                if r.status().is_success() {
                    let data: Value = r.json().await.unwrap_or(Value::Null);
                    self.ack_events(&events, &data, None);
                    self.schedule_event_recovery(true);
                    return true;
                }
            }
            if attempt + 1 < MAX_RETRIES {
                tokio::time::sleep(Duration::from_secs((attempt + 1) as u64)).await;
            }
        }
        false
    }
}

mod claim;
mod recovery;
pub use claim::{claim_ticket, ClaimError, ClaimedLaunch};

#[cfg(test)]
mod tests;
