//! 실행 기록 보관함 — 서버가 저장했다고 답한 것만 지우는 로컬 SQLite 저널.
//!
//! 서버가 확인하지 않은 기록은 모든 계정을 합쳐 최대 7일·10,000줄·UTF-8 JSON 32 MiB까지만 둠.
//! 보관 한도 때문에 잃은 수는 서버 확인과 따로 셈. 주문 대기열이 아님.
use once_cell::sync::Lazy;
use regex::{Captures, Regex};
use rusqlite::{params, params_from_iter, Connection, OptionalExtension, TransactionBehavior};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::{Arc, Mutex};

pub const MAX_BATCH: usize = 100;
/// JSON 본문 틀까지 포함해 서버 상한(256 KiB)보다 낮게 잡음. 보낼 때는 비 ASCII 글자를 이스케이프하므로
/// 저널의 UTF-8 크기가 실제로 보내는 크기와 같지 않음.
pub const MAX_WIRE_BODY_BYTES: usize = 200_000;
pub const PENDING_TTL_SECONDS: i64 = 7 * 86400;
pub const MAX_PENDING_ROWS: i64 = 10_000;
pub const MAX_PENDING_BYTES: i64 = 32 * 1024 * 1024;
pub const MAX_RETAINED_JOURNAL_BYTES: i64 = 4 * 1024 * 1024;
pub const MEMORY_MAX_ROWS: usize = 1_000;
pub const MEMORY_MAX_BYTES: usize = 2 * 1024 * 1024;
pub const MAINTENANCE_SECONDS: i64 = 60;
pub const MAX_EVENT_BYTES: usize = 16384;
pub const LOSS_REASONS: [&str; 4] = ["disk_expired", "disk_capacity", "memory_expired", "memory_capacity"];
const MAX_COUNTER: i64 = i64::MAX;

static EVENT_KEY: Lazy<Regex> = Lazy::new(|| Regex::new(r"^runner:[a-f0-9]{32}$").unwrap());
static SECRET: Lazy<Regex> = Lazy::new(|| {
    Regex::new(
        r#"(?i)((?:api[_-]?key|secret|password|authorization|access[_-]?token|token|x-runner-key)["']?\s*[:=]\s*)["']?(?:bearer\s+)?[^\s,;"']+"#,
    )
    .unwrap()
});
static BEARER: Lazy<Regex> = Lazy::new(|| Regex::new(r"(?i)\bBearer\s+[A-Za-z0-9._-]+").unwrap());
static PROVIDER_KEY: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"\b(?:sk-[A-Za-z0-9_-]{12,}|AIza[A-Za-z0-9_-]{20,}|ggp_[A-Za-z0-9_-]{20,})\b").unwrap());
static JWT: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"\beyJ[A-Za-z0-9_-]{8,}\.[A-Za-z0-9_-]{8,}\.[A-Za-z0-9_-]{8,}\b").unwrap());
static DSN: Lazy<Regex> =
    Lazy::new(|| Regex::new(r#"(?i)\b(?:postgres(?:ql)?|mysql|redis)(?:\+[A-Za-z0-9_-]+)?://[^\s"']+"#).unwrap());
static URL: Lazy<Regex> = Lazy::new(|| Regex::new(r#"(?i)https?://[^\s<>"']+"#).unwrap());

/// 앱을 업데이트해도 바뀌지 않는 위치. 저널을 실행 파일 폴더에 두지 않음.
pub fn default_path() -> PathBuf {
    crate::paths::data_dir().join("activity").join("outbox.sqlite3")
}

/// 인증 값은 메모리에만 두고 저장하지 않음. 계정·서버가 다르면 우연히 둘 다 세션 번호가 1이어도
/// 이 기록을 내보낼 수 없음.
pub fn namespace(base: &str, member_key: &str) -> String {
    let material = format!("ggp-event-outbox-v1\0{}\0{}", base.trim_end_matches('/'), member_key);
    hex::encode(Sha256::digest(material.as_bytes()))
}

pub fn redact(value: &str, member_key: &str) -> String {
    let mut text = value.to_string();
    if !member_key.is_empty() {
        text = text.replace(member_key, "[키 숨김]");
    }
    let text = SECRET.replace_all(&text, "${1}[redacted]");
    let text = BEARER.replace_all(&text, "Bearer [redacted]");
    let text = PROVIDER_KEY.replace_all(&text, "[redacted]");
    let text = JWT.replace_all(&text, "[redacted]");
    let text = DSN.replace_all(&text, "[connection]");
    URL.replace_all(&text, |caps: &Captures| public_url(&caps[0])).into_owned()
}

/// Python `urlsplit`처럼 나눔 — 사용자 정보(userinfo)가 붙은 URL은 통째로 가리고,
/// 나머지는 scheme·netloc·path만 남김.
fn public_url(url: &str) -> String {
    let Some(idx) = url.find("://") else { return "[url]".into() };
    let scheme = url[..idx].to_lowercase();
    let rest = &url[idx + 3..];
    let netloc_end = rest.find(['/', '?', '#']).unwrap_or(rest.len());
    let netloc = &rest[..netloc_end];
    let after = &rest[netloc_end..];
    let path_end = after.find(['?', '#']).unwrap_or(after.len());
    let path = &after[..path_end];
    let has_open = netloc.contains('[');
    let has_close = netloc.contains(']');
    if has_open != has_close {
        return "[url]".into();
    }
    if has_open && has_close && !bracketed_netloc_ok(netloc) {
        return "[url]".into();
    }
    if let Some(at) = netloc.rfind('@') {
        let userinfo = &netloc[..at];
        let (user, password) = match userinfo.find(':') {
            Some(colon) => (&userinfo[..colon], Some(&userinfo[colon + 1..])),
            None => (userinfo, None),
        };
        if !user.is_empty() || password.is_some_and(|p| !p.is_empty()) {
            return "[authenticated-url]".into();
        }
    }
    format!("{scheme}://{netloc}{path}")
}

fn bracketed_netloc_ok(netloc: &str) -> bool {
    let host_port = match netloc.rfind('@') {
        Some(at) => &netloc[at + 1..],
        None => netloc,
    };
    let hostname = match host_port.find('[') {
        Some(open) => {
            if open > 0 {
                return false;
            }
            let bracketed = &host_port[open + 1..];
            let (host, port) = match bracketed.find(']') {
                Some(close) => (&bracketed[..close], &bracketed[close + 1..]),
                None => (bracketed, ""),
            };
            if !port.is_empty() && !port.starts_with(':') {
                return false;
            }
            host
        }
        None => host_port.split(':').next().unwrap_or(""),
    };
    if let Some(rest) = hostname.strip_prefix('v') {
        static FUTURE: Lazy<Regex> = Lazy::new(|| Regex::new(r"^[a-fA-F0-9]+\..+$").unwrap());
        return FUTURE.is_match(rest);
    }
    hostname.parse::<std::net::Ipv6Addr>().is_ok()
}

/// 오래된 것부터, 개수 한도와 실제 ASCII JSON 바이트 한도에 모두 맞는 앞부분.
///
/// 기본 구분자로는 []를 채울 때 기록마다 인코딩한 길이에 기록 사이 ', '가 더 듦. 둘러싼
/// heartbeat·stopped 본문도 함께 셈. 다음 요청으로 넘어갈 기록이라고 지우거나 건너뛰지 않음.
pub fn wire_batch(events: Vec<Value>, body: &Value) -> Vec<Value> {
    let mut framing = match body {
        Value::Object(map) => map.clone(),
        _ => serde_json::Map::new(),
    };
    framing.insert("events".into(), Value::Array(Vec::new()));
    let mut size = py_dumps(&Value::Object(framing), true, false, false).len();
    let mut result = Vec::new();
    for event in events.into_iter().take(MAX_BATCH) {
        let contribution = py_dumps(&event, true, false, false).len() + if result.is_empty() { 0 } else { 2 };
        if size + contribution > MAX_WIRE_BODY_BYTES {
            break;
        }
        result.push(event);
        size += contribution;
    }
    result
}

// --- Python json.dumps와 같은 바이트 ------------------------------------------

/// `json.dumps(value, ensure_ascii=?, sort_keys=?, separators=compact ? (",",":") : (", ",": "))`.
pub(crate) fn py_dumps(value: &Value, ensure_ascii: bool, sort_keys: bool, compact: bool) -> String {
    let mut out = String::new();
    write_py(value, &mut out, ensure_ascii, sort_keys, compact);
    out
}

fn write_py(value: &Value, out: &mut String, ascii: bool, sort: bool, compact: bool) {
    let (item_sep, key_sep) = if compact { (",", ":") } else { (", ", ": ") };
    match value {
        Value::Null => out.push_str("null"),
        Value::Bool(b) => out.push_str(if *b { "true" } else { "false" }),
        Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                out.push_str(&i.to_string());
            } else if let Some(u) = n.as_u64() {
                out.push_str(&u.to_string());
            } else {
                out.push_str(&py_float(n.as_f64().unwrap_or(0.0)));
            }
        }
        Value::String(s) => py_str_into(s, out, ascii),
        Value::Array(items) => {
            out.push('[');
            for (i, item) in items.iter().enumerate() {
                if i > 0 {
                    out.push_str(item_sep);
                }
                write_py(item, out, ascii, sort, compact);
            }
            out.push(']');
        }
        Value::Object(map) => {
            out.push('{');
            let mut entries: Vec<(&String, &Value)> = map.iter().collect();
            if sort {
                entries.sort_by(|a, b| a.0.cmp(b.0));
            }
            for (i, (k, v)) in entries.into_iter().enumerate() {
                if i > 0 {
                    out.push_str(item_sep);
                }
                py_str_into(k, out, ascii);
                out.push_str(key_sep);
                write_py(v, out, ascii, sort, compact);
            }
            out.push('}');
        }
    }
}

pub(crate) fn py_str(s: &str, ascii: bool) -> String {
    let mut out = String::new();
    py_str_into(s, &mut out, ascii);
    out
}

fn py_str_into(s: &str, out: &mut String, ascii: bool) {
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            '\u{08}' => out.push_str("\\b"),
            '\u{0c}' => out.push_str("\\f"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c if ascii && !(' '..='~').contains(&c) => {
                let mut buf = [0u16; 2];
                for unit in c.encode_utf16(&mut buf) {
                    out.push_str(&format!("\\u{:04x}", unit));
                }
            }
            c => out.push(c),
        }
    }
    out.push('"');
}

/// Python `float.__repr__` — 되읽어 같은 값이 나오는 가장 짧은 자릿수, decpt <= -4 또는 > 16이면 지수 표기.
pub(crate) fn py_float(f: f64) -> String {
    if f.is_nan() {
        return "NaN".into();
    }
    if f.is_infinite() {
        return if f > 0.0 { "Infinity".into() } else { "-Infinity".into() };
    }
    let sci = format!("{:e}", f);
    let (negative, sci) = match sci.strip_prefix('-') {
        Some(rest) => (true, rest.to_string()),
        None => (false, sci),
    };
    let (mantissa, exp) = sci.split_once('e').unwrap_or((&sci, "0"));
    let exp: i32 = exp.parse().unwrap_or(0);
    let digits: String = mantissa.chars().filter(|c| *c != '.').collect();
    let decpt = exp + 1;
    let mut out = String::new();
    if negative {
        out.push('-');
    }
    if decpt <= -4 || decpt > 16 {
        out.push_str(&digits[..1]);
        if digits.len() > 1 {
            out.push('.');
            out.push_str(&digits[1..]);
        }
        let e = decpt - 1;
        out.push_str(&format!("e{}{:02}", if e < 0 { '-' } else { '+' }, e.abs()));
    } else if decpt <= 0 {
        out.push_str("0.");
        out.push_str(&"0".repeat((-decpt) as usize));
        out.push_str(&digits);
    } else if decpt as usize >= digits.len() {
        out.push_str(&digits);
        out.push_str(&"0".repeat(decpt as usize - digits.len()));
        out.push_str(".0");
    } else {
        out.push_str(&digits[..decpt as usize]);
        out.push('.');
        out.push_str(&digits[decpt as usize..]);
    }
    out
}

// --- 저널 ---------------------------------------------------------------------

pub type Clock = Arc<dyn Fn() -> f64 + Send + Sync>;

/// 기본 시계 — 유닉스 초(소수).
pub fn system_clock() -> Clock {
    Arc::new(crate::clock::unix_secs)
}

/// 테스트·진단용 장애 주입. `Append`는 append만, `All`은 DB를 여는 모든 동작이 실패함.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OutboxFault {
    None = 0,
    Append = 1,
    All = 2,
}

/// 테스트·내장용 정책은 기본값보다 엄격할 수는 있어도 무제한이거나 더 클 수는 없음(열 때 기본값으로 자름).
#[derive(Clone, Copy, Debug)]
pub struct OutboxLimits {
    pub max_rows: i64,
    pub max_bytes: i64,
    pub ttl_seconds: i64,
}

impl Default for OutboxLimits {
    fn default() -> Self {
        OutboxLimits { max_rows: MAX_PENDING_ROWS, max_bytes: MAX_PENDING_BYTES, ttl_seconds: PENDING_TTL_SECONDS }
    }
}

/// 보관 상태 숫자 — 한도·기간 때문에 지운 기록 수.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct LossStats {
    pub disk_expired: i64,
    pub disk_capacity: i64,
    pub memory_expired: i64,
    pub memory_capacity: i64,
    pub pending_rows: i64,
    pub pending_bytes: i64,
}

pub struct EventOutbox {
    path: PathBuf,
    clock: Clock,
    /// 열 때 기본값 안으로 자른 보관 한도.
    limits: OutboxLimits,
    fault: AtomicU8,
    conn: Mutex<Option<Connection>>,
    /// 밀린 기록 다시 보내기 상태 — 같은 저장함을 쓰는 모든 실행(ServerClient)이 함께 씀.
    /// 실행마다 따로 두면 서버가 돌아왔을 때 모두가 같은 남의 세션을 한꺼번에 다시 보냄.
    pub(crate) recovery: Mutex<RecoveryState>,
    /// 보관 중인 (줄 수, 바이트)의 위쪽 어림값. 마지막 전체 세기 뒤로 더한 것만 반영하고 지운 것은 빼지 않으므로
    /// 늘 실제 이상임 — 어림값으로도 한도 아래면 기록마다 표 전체를 세지 않음. None이면 모름.
    estimate: Mutex<Option<(i64, i64)>>,
}

/// 다시 보내기 상태. 세션 번호는 서버 전체에서 고유하므로 세션 번호로만 셈.
#[derive(Default)]
pub(crate) struct RecoveryState {
    /// 지금 다시 보내는 중인지(앱 전체에서 하나만).
    pub running: bool,
    /// 세션마다 다음에 다시 보내도 되는 때.
    pub retry_after: std::collections::HashMap<i64, std::time::Instant>,
    /// 여러 세션을 돌아가며 보내려는 시작 위치.
    pub cursor: usize,
    /// 지금 실행 중인 세션 — 그 세션의 기록은 주인이 하트비트로 보내므로 다시 보내기에서 건너뜀.
    pub live: std::collections::HashSet<i64>,
}

fn db_err(e: rusqlite::Error) -> String {
    e.to_string()
}

impl EventOutbox {
    /// 열지 못해도 객체는 만듦 — 그때는 '사용 불가'이고 모든 동작이 Err(다음 호출에서 다시 엶).
    pub fn open(path: Option<PathBuf>) -> Self {
        Self::open_with(path, system_clock(), OutboxLimits::default())
    }

    pub fn open_with(path: Option<PathBuf>, clock: Clock, limits: OutboxLimits) -> Self {
        let outbox = EventOutbox {
            path: path.unwrap_or_else(default_path),
            clock,
            limits: OutboxLimits {
                max_rows: limits.max_rows.clamp(1, MAX_PENDING_ROWS),
                max_bytes: limits.max_bytes.clamp(1, MAX_PENDING_BYTES),
                ttl_seconds: limits.ttl_seconds.clamp(1, PENDING_TTL_SECONDS),
            },
            fault: AtomicU8::new(OutboxFault::None as u8),
            conn: Mutex::new(None),
            recovery: Mutex::new(RecoveryState::default()),
            estimate: Mutex::new(None),
        };
        let _ = outbox.with_db(|_| Ok(()));
        outbox
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    #[cfg(test)]
    pub fn set_fault(&self, fault: OutboxFault) {
        self.fault.store(fault as u8, Ordering::SeqCst);
    }

    fn fault(&self) -> u8 {
        self.fault.load(Ordering::SeqCst)
    }

    /// 지금 DB를 쓸 수 있는지(열려 있지 않으면 한 번 열어 봄).
    #[cfg(test)]
    pub fn is_available(&self) -> bool {
        self.with_db(|_| Ok(())).is_ok()
    }

    pub fn now_ms(&self) -> i64 {
        (((self.clock)() * 1000.0) as i64).max(1)
    }

    fn with_db<T>(&self, f: impl FnOnce(&mut Connection) -> Result<T, String>) -> Result<T, String> {
        if self.fault() == OutboxFault::All as u8 {
            return Err("activity outbox unavailable".into());
        }
        let mut guard = self.conn.lock().unwrap_or_else(|e| e.into_inner());
        if guard.is_none() {
            *guard = Some(self.connect()?);
        }
        f(guard.as_mut().expect("connection"))
    }

    fn connect(&self) -> Result<Connection, String> {
        let memory = self.path.as_os_str() == ":memory:";
        if !memory {
            if let Some(parent) = self.path.parent() {
                if !parent.as_os_str().is_empty() {
                    let mut builder = std::fs::DirBuilder::new();
                    builder.recursive(true);
                    #[cfg(unix)]
                    {
                        use std::os::unix::fs::DirBuilderExt;
                        builder.mode(0o700);
                    }
                    builder.create(parent).map_err(|e| e.to_string())?;
                }
            }
        }
        let mut conn =
            if memory { Connection::open_in_memory() } else { Connection::open(&self.path) }.map_err(db_err)?;
        conn.busy_timeout(std::time::Duration::from_millis(1500)).map_err(db_err)?;
        // auto_vacuum은 DB를 처음 만들 때 켜짐(INCREMENTAL — 비운 페이지는 유지 보수 때 조금씩 돌려줌).
        // WAL + synchronous=NORMAL: 앱이 죽어도 기록은 남고, 커밋마다 디스크 동기화(fsync)를 기다리지 않음.
        // 로그 줄마다 쓰는 곳이라 FULL은 주문·손절 루프를 붙잡았음. 정전 직전 몇 줄만 잃을 수 있고,
        // 그 기록은 서버가 event_key로 중복을 걸러 다시 받음.
        conn.execute_batch(&format!(
            "PRAGMA auto_vacuum=INCREMENTAL; PRAGMA journal_mode=WAL; \
             PRAGMA journal_size_limit={MAX_RETAINED_JOURNAL_BYTES}; PRAGMA synchronous=NORMAL;"
        ))
        .map_err(db_err)?;
        {
            let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate).map_err(db_err)?;
            tx.execute_batch(
                "CREATE TABLE IF NOT EXISTS pending_events (
                    seq INTEGER PRIMARY KEY AUTOINCREMENT,
                    account_scope TEXT NOT NULL, session_id INTEGER NOT NULL,
                    event_key TEXT NOT NULL, payload TEXT NOT NULL,
                    created_ms INTEGER NOT NULL DEFAULT 0,
                    payload_bytes INTEGER NOT NULL DEFAULT 0,
                    UNIQUE(account_scope, session_id, event_key))",
            )
            .map_err(db_err)?;
            tx.execute_batch(
                "CREATE INDEX IF NOT EXISTS pending_events_created ON pending_events(created_ms,seq);
                 CREATE TABLE IF NOT EXISTS retention_meta (
                    id INTEGER PRIMARY KEY CHECK(id=1),
                    disk_expired INTEGER NOT NULL DEFAULT 0, disk_capacity INTEGER NOT NULL DEFAULT 0,
                    memory_expired INTEGER NOT NULL DEFAULT 0, memory_capacity INTEGER NOT NULL DEFAULT 0,
                    noted_disk_expired INTEGER NOT NULL DEFAULT 0, noted_disk_capacity INTEGER NOT NULL DEFAULT 0,
                    noted_memory_expired INTEGER NOT NULL DEFAULT 0, noted_memory_capacity INTEGER NOT NULL DEFAULT 0,
                    last_loss_ms INTEGER NOT NULL DEFAULT 0, last_notice_ms INTEGER NOT NULL DEFAULT 0,
                    last_maintenance_ms INTEGER NOT NULL DEFAULT 0);
                 INSERT OR IGNORE INTO retention_meta(id) VALUES(1);",
            )
            .map_err(db_err)?;
            tx.commit().map_err(db_err)?;
        }
        #[cfg(unix)]
        if !memory {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&self.path, std::fs::Permissions::from_mode(0o600)).map_err(|e| e.to_string())?;
        }
        Ok(conn)
    }

    fn count_loss(db: &Connection, reason: &str, count: i64, now: i64) -> Result<(), String> {
        if !LOSS_REASONS.contains(&reason) || count <= 0 {
            return Ok(());
        }
        db.execute(
            &format!("UPDATE retention_meta SET {reason}=MIN(?,{reason}+?),last_loss_ms=? WHERE id=1"),
            params![MAX_COUNTER, count, now],
        )
        .map_err(db_err)?;
        Ok(())
    }

    fn prune(&self, db: &Connection, now: i64) -> Result<(), String> {
        // 계정·세션 조건을 걸지 않음 — 다른 회원 키의 기록과 session=0 고아 기록도 같은 로컬 보관 한도에 듦.
        let OutboxLimits { max_rows, max_bytes, ttl_seconds: ttl } = self.limits;
        let expired =
            db.execute("DELETE FROM pending_events WHERE created_ms<=?", params![now - ttl * 1000]).map_err(db_err)?;
        Self::count_loss(db, "disk_expired", expired as i64, now)?;
        let (mut count, mut size): (i64, i64) = db
            .query_row("SELECT COUNT(*),COALESCE(SUM(payload_bytes),0) FROM pending_events", [], |r| {
                Ok((r.get(0)?, r.get(1)?))
            })
            .map_err(db_err)?;
        let mut removed = 0i64;
        while count > max_rows || size > max_bytes {
            let rows: Vec<(i64, i64)> = {
                let mut stmt = db
                    .prepare("SELECT seq,payload_bytes FROM pending_events ORDER BY seq LIMIT 250")
                    .map_err(db_err)?;
                let rows = stmt.query_map([], |r| Ok((r.get(0)?, r.get(1)?))).map_err(db_err)?;
                rows.collect::<Result<_, _>>().map_err(db_err)?
            };
            if rows.is_empty() {
                break;
            }
            for (seq, payload_bytes) in rows {
                if count <= max_rows && size <= max_bytes {
                    break;
                }
                db.execute("DELETE FROM pending_events WHERE seq=?", params![seq]).map_err(db_err)?;
                count -= 1;
                size -= payload_bytes;
                removed += 1;
            }
        }
        *self.estimate.lock().unwrap_or_else(|e| e.into_inner()) = Some((count, size));
        Self::count_loss(db, "disk_capacity", removed, now)
    }

    /// 기록 하나를 더하는 쓰기의 정리. 어림값으로도 한도 안이면 기한 지난 것만 지우고(색인을 탐),
    /// 아니면 전체를 세는 [`prune`]으로 — 장애 동안 실행 여럿이 로그를 쓸 때 매번 표 전체를 세지 않게.
    fn prune_for_append(&self, db: &Connection, now: i64, added_bytes: i64) -> Result<(), String> {
        let OutboxLimits { max_rows, max_bytes, ttl_seconds: ttl } = self.limits;
        let fits = {
            let mut est = self.estimate.lock().unwrap_or_else(|e| e.into_inner());
            match *est {
                Some((rows, bytes)) if rows < max_rows && bytes + added_bytes <= max_bytes => {
                    *est = Some((rows + 1, bytes + added_bytes));
                    true
                }
                _ => false,
            }
        };
        if !fits {
            return self.prune(db, now);
        }
        let expired =
            db.execute("DELETE FROM pending_events WHERE created_ms<=?", params![now - ttl * 1000]).map_err(db_err)?;
        Self::count_loss(db, "disk_expired", expired as i64, now)
    }

    /// 양이 정해져 있고 막지 않는 작업. PASSIVE 체크포인트는 다른 읽기를 기다리지 않음.
    fn compact(db: &Connection) {
        let _ = db.execute_batch("PRAGMA wal_checkpoint(PASSIVE); PRAGMA incremental_vacuum(128);");
    }

    pub fn try_maintain(&self, force: bool) -> Result<(), String> {
        self.with_db(|db| {
            let now = self.now_ms();
            let last: i64 = db
                .query_row("SELECT last_maintenance_ms FROM retention_meta WHERE id=1", [], |r| r.get(0))
                .map_err(db_err)?;
            if !force && (0..MAINTENANCE_SECONDS * 1000).contains(&(now - last)) {
                return Ok(());
            }
            let tx = db.transaction_with_behavior(TransactionBehavior::Immediate).map_err(db_err)?;
            self.prune(&tx, now)?;
            tx.execute("UPDATE retention_meta SET last_maintenance_ms=? WHERE id=1", params![now]).map_err(db_err)?;
            tx.commit().map_err(db_err)?;
            Self::compact(db);
            Ok(())
        })
    }

    #[cfg(test)]
    pub fn loss_stats(&self) -> Result<LossStats, String> {
        self.with_db(|db| {
            let mut stats = db
                .query_row(
                    "SELECT disk_expired,disk_capacity,memory_expired,memory_capacity FROM retention_meta WHERE id=1",
                    [],
                    |r| {
                        Ok(LossStats {
                            disk_expired: r.get(0)?,
                            disk_capacity: r.get(1)?,
                            memory_expired: r.get(2)?,
                            memory_capacity: r.get(3)?,
                            ..Default::default()
                        })
                    },
                )
                .map_err(db_err)?;
            let (rows, bytes): (i64, i64) = db
                .query_row("SELECT COUNT(*),COALESCE(SUM(payload_bytes),0) FROM pending_events", [], |r| {
                    Ok((r.get(0)?, r.get(1)?))
                })
                .map_err(db_err)?;
            stats.pending_rows = rows;
            stats.pending_bytes = bytes;
            Ok(stats)
        })
    }

    pub fn try_record_memory_loss(&self, counts: &BTreeMap<String, u64>) -> Result<(), String> {
        self.with_db(|db| {
            let tx = db.transaction_with_behavior(TransactionBehavior::Immediate).map_err(db_err)?;
            for reason in ["memory_expired", "memory_capacity"] {
                let count = counts.get(reason).copied().unwrap_or(0).min(i64::MAX as u64) as i64;
                Self::count_loss(&tx, reason, count, self.now_ms())?;
            }
            tx.commit().map_err(db_err)
        })
    }

    /// 이 파일을 쓰는 모든 클라이언트를 통틀어 1분에 한 번만, 그동안 늘어난 손실 수를 가져감.
    /// 키는 LOSS_REASONS 넷 모두, 알릴 것이 없으면 빈 맵.
    pub fn try_take_loss_notice(&self) -> Result<BTreeMap<String, u64>, String> {
        self.with_db(|db| {
            let now = self.now_ms();
            let tx = db.transaction_with_behavior(TransactionBehavior::Immediate).map_err(db_err)?;
            let values: Vec<i64> = tx
                .query_row(
                    "SELECT disk_expired,disk_capacity,memory_expired,memory_capacity,\
                     noted_disk_expired,noted_disk_capacity,noted_memory_expired,noted_memory_capacity,\
                     last_notice_ms FROM retention_meta WHERE id=1",
                    [],
                    |r| (0..9).map(|i| r.get::<_, i64>(i)).collect(),
                )
                .map_err(db_err)?;
            let delta: BTreeMap<String, u64> = LOSS_REASONS
                .iter()
                .enumerate()
                .map(|(i, reason)| (reason.to_string(), (values[i] - values[i + 4]).max(0) as u64))
                .collect();
            if delta.values().all(|v| *v == 0) || (0..MAINTENANCE_SECONDS * 1000).contains(&(now - values[8])) {
                tx.commit().map_err(db_err)?;
                return Ok(BTreeMap::new());
            }
            tx.execute(
                "UPDATE retention_meta SET noted_disk_expired=disk_expired,noted_disk_capacity=disk_capacity,\
                 noted_memory_expired=memory_expired,noted_memory_capacity=memory_capacity,last_notice_ms=? WHERE id=1",
                params![now],
            )
            .map_err(db_err)?;
            tx.commit().map_err(db_err)?;
            Ok(delta)
        })
    }

    /// 오류면 빈 맵 — 오류를 구분해야 하면 try_take_loss_notice.
    #[cfg(test)]
    pub fn take_loss_notice(&self) -> BTreeMap<String, u64> {
        self.try_take_loss_notice().unwrap_or_default()
    }

    pub fn existing_unassigned_keys(&self, scope: &str, keys: &[String]) -> Result<Vec<String>, String> {
        self.with_db(|db| {
            let mut existing: Vec<String> = Vec::new();
            for chunk in keys.chunks(MAX_BATCH) {
                let placeholders = vec!["?"; chunk.len()].join(",");
                let sql = format!(
                    "SELECT event_key FROM pending_events WHERE account_scope=? AND session_id=0 AND event_key IN ({placeholders})"
                );
                let mut stmt = db.prepare(&sql).map_err(db_err)?;
                let args = std::iter::once(scope).chain(chunk.iter().map(String::as_str));
                let rows = stmt.query_map(params_from_iter(args), |r| r.get::<_, String>(0)).map_err(db_err)?;
                for row in rows {
                    let key = row.map_err(db_err)?;
                    if !existing.contains(&key) {
                        existing.push(key);
                    }
                }
            }
            Ok(existing)
        })
    }

    pub fn append(&self, scope: &str, session_id: i64, event: &Value) -> Result<(), String> {
        self.append_at(scope, session_id, event, None)
    }

    /// `created_ms`는 메모리에 있던 기록을 다시 쓸 때 원래 나이를 지키려고 줌.
    pub fn append_at(
        &self,
        scope: &str,
        session_id: i64,
        event: &Value,
        created_ms: Option<i64>,
    ) -> Result<(), String> {
        let key = match event.get("event_key") {
            Some(Value::String(s)) => s.as_str(),
            _ => "",
        };
        if !EVENT_KEY.is_match(key) {
            return Err("invalid event identity".into());
        }
        if session_id < 0 {
            return Err("invalid session scope".into());
        }
        let payload = py_dumps(event, false, true, false);
        let payload_bytes = payload.len();
        if payload_bytes > MAX_EVENT_BYTES {
            return Err("event too large".into());
        }
        if self.fault() == OutboxFault::Append as u8 {
            return Err("activity outbox unavailable".into());
        }
        self.with_db(|db| {
            let tx = db.transaction_with_behavior(TransactionBehavior::Immediate).map_err(db_err)?;
            let prior: Option<String> = tx
                .query_row(
                    "SELECT payload FROM pending_events WHERE account_scope=? AND session_id=? AND event_key=?",
                    params![scope, session_id, key],
                    |r| r.get(0),
                )
                .optional()
                .map_err(db_err)?;
            if prior.is_some_and(|p| p != payload) {
                return Err("event identity is immutable".into());
            }
            let now = self.now_ms();
            let created = created_ms.map(|c| c.min(now).max(1)).unwrap_or(now);
            tx.execute(
                "INSERT OR IGNORE INTO pending_events(account_scope,session_id,event_key,payload,created_ms,payload_bytes) \
                 VALUES(?,?,?,?,?,?)",
                params![scope, session_id, key, payload, created, payload_bytes as i64],
            )
            .map_err(db_err)?;
            // 기록이 몰려도 이 쓰기를 커밋하기 전에 보관 한도를 지킴.
            self.prune_for_append(&tx, now, payload_bytes as i64)?;
            tx.commit().map_err(db_err)
        })?;
        self.try_maintain(false)
    }

    /// 새로 알게 된 세션은 아직 살아 있는 이 클라이언트가 낸 기록(키 목록)에만 붙임. 이전 실행의 고아 기록이나
    /// 다른 시작 중인 매크로의 기록을 짐작으로 이 실행에 넣지 않음.
    pub fn bind_unassigned(&self, scope: &str, session_id: i64, keys: &[String]) -> Result<(), String> {
        let keys: Vec<&String> = keys.iter().filter(|k| EVENT_KEY.is_match(k)).collect();
        if session_id <= 0 || keys.is_empty() {
            return Ok(());
        }
        self.with_db(|db| {
            let tx = db.transaction().map_err(db_err)?;
            for key in keys {
                tx.execute(
                    "UPDATE pending_events SET session_id=? WHERE account_scope=? AND session_id=0 AND event_key=?",
                    params![session_id, scope, key],
                )
                .map_err(db_err)?;
            }
            tx.commit().map_err(db_err)
        })
    }

    pub fn peek(
        &self,
        scope: &str,
        session_id: i64,
        limit: usize,
        keys: Option<&[String]>,
    ) -> Result<Vec<Value>, String> {
        let bound = limit.clamp(1, MAX_BATCH);
        self.try_maintain(false)?;
        let rows: Vec<(i64, String)> = self.with_db(|db| {
            let mut rows: Vec<(i64, String)> = Vec::new();
            match keys {
                None => {
                    let mut stmt = db
                        .prepare(
                            "SELECT seq,payload FROM pending_events WHERE account_scope=? AND session_id=? ORDER BY seq LIMIT ?",
                        )
                        .map_err(db_err)?;
                    let found = stmt
                        .query_map(params![scope, session_id, bound as i64], |r| Ok((r.get(0)?, r.get(1)?)))
                        .map_err(db_err)?;
                    for row in found {
                        rows.push(row.map_err(db_err)?);
                    }
                }
                Some(keys) => {
                    for chunk in keys.chunks(MAX_BATCH) {
                        let placeholders = vec!["?"; chunk.len()].join(",");
                        let sql = format!(
                            "SELECT seq,payload FROM pending_events WHERE account_scope=? AND session_id=? \
                             AND event_key IN ({placeholders}) ORDER BY seq LIMIT ?"
                        );
                        let mut stmt = db.prepare(&sql).map_err(db_err)?;
                        let mut args: Vec<rusqlite::types::Value> = vec![scope.to_string().into(), session_id.into()];
                        args.extend(chunk.iter().map(|k| rusqlite::types::Value::from(k.clone())));
                        args.push((bound as i64).into());
                        let found = stmt
                            .query_map(params_from_iter(args), |r| Ok((r.get(0)?, r.get(1)?)))
                            .map_err(db_err)?;
                        for row in found {
                            rows.push(row.map_err(db_err)?);
                        }
                    }
                    rows.sort();
                    rows.truncate(bound);
                }
            }
            Ok(rows)
        })?;
        rows.into_iter().map(|(_, payload)| serde_json::from_str(&payload).map_err(|e| e.to_string())).collect()
    }

    pub fn sessions(&self, scope: &str) -> Result<Vec<i64>, String> {
        self.try_maintain(false)?;
        self.with_db(|db| {
            let mut stmt = db
                .prepare(
                    "SELECT session_id FROM pending_events WHERE account_scope=? AND session_id>0 \
                     GROUP BY session_id ORDER BY MIN(seq)",
                )
                .map_err(db_err)?;
            let rows = stmt.query_map(params![scope], |r| r.get::<_, i64>(0)).map_err(db_err)?;
            rows.collect::<Result<_, _>>().map_err(db_err)
        })
    }

    pub fn acknowledge(&self, scope: &str, session_id: i64, keys: &[String]) -> Result<(), String> {
        let keys: Vec<&String> = keys.iter().take(MAX_BATCH).filter(|k| EVENT_KEY.is_match(k)).collect();
        if keys.is_empty() {
            return Ok(());
        }
        self.with_db(|db| {
            let tx = db.transaction().map_err(db_err)?;
            for key in keys {
                tx.execute(
                    "DELETE FROM pending_events WHERE account_scope=? AND session_id=? AND event_key=?",
                    params![scope, session_id, key],
                )
                .map_err(db_err)?;
            }
            tx.commit().map_err(db_err)
        })
    }

    pub fn close(&self) {
        let mut guard = self.conn.lock().unwrap_or_else(|e| e.into_inner());
        *guard = None;
    }

    #[cfg(test)]
    fn pragma_i64(&self, sql: &str) -> i64 {
        self.with_db(|db| db.query_row(sql, [], |r| r.get(0)).map_err(db_err)).unwrap()
    }
}

#[cfg(test)]
mod tests;
