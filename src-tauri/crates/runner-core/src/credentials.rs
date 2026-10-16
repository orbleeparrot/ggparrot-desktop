//! 이 기기에 키 기억하기 — OS 키체인에 자격증명 v2 JSON 하나와 그 저장 규칙.
//!
//! 값 모양(v2): {"version": 2, "member_key": str, "exchanges": {거래소: {"api_key": str, "api_secret": str}}
//! [, "connection_history": {거래소: {"public_ipv4", "observed_at"}}]}.
//! v1(바이낸스 한 쌍만, version 없음)은 읽을 때 binance로 옮겨 담음. 저장하면 v2로 다시 씀.
//! 서버로는 보내지 않음.
use serde::Serialize;
use serde_json::{Map, Value};
use std::collections::BTreeMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;

use crate::connection::valid_ip_observation;
use crate::outbox::{py_float, py_str};
use crate::types::{Credentials, IpObservation, KeyPair, KeySlot};

/// 화면에 보이는 OS 키 저장소 이름 — "키체인"은 macOS 말이라 Windows에서는 자격 증명 관리자라고 부름.
pub const KEY_STORE_NAME: &str = if cfg!(windows) { "Windows 자격 증명 관리자" } else { "키체인" };
pub const KEYRING_SERVICE: &str = "ggparrot-desktop";
pub const KEYRING_USER: &str = "credentials";
/// 저장 항목의 키 칸 — 세 거래소와 바이낸스 연습 칸 둘(KeySlot과 같은 이름).
pub const EXCHANGES: [&str; 5] = ["binance", "binance_testnet", "binance_demo", "upbit", "bithumb"];
const DECRYPT_ERROR: &str = "decrypt_error";

pub const UNREADABLE_NOT_CHANGED: &str =
    "기억한 키를 읽지 못해 바꾸지 않았어요. 설정 › 키 저장의 안내를 먼저 확인하세요.";
pub const INCOMPLETE_PAIR: &str = "저장하려는 거래소의 API 키와 시크릿을 모두 입력하세요.";
pub const STATE_CHANGED: &str = "키 저장 항목이 그사이 바뀌었어요. 앱을 다시 실행해 기억한 키를 불러온 뒤 확인하세요.";
pub const SAVE_FAILED: &str = "키를 이 기기에 저장하지 못했어요.";
pub const TOO_LONG: &str = "키가 너무 길어 이 기기의 키 저장소에 넣지 못했어요. 붙여 넣은 키가 맞는지 확인하세요.";
const TOO_LONG_CODE: &str = "too_long";
/// 키 한 칸의 최대 길이. 거래소 키는 64자 안팎이라 넉넉하고, 잘못 붙여 넣은 긴 글은 막음
/// (Windows 자격 증명 관리자는 한 항목이 2560바이트까지라 모든 키를 합쳐도 넘지 않게).
pub const MAX_KEY_CHARS: usize = 200;
pub const CLEAR_FAILED: &str = "저장된 키를 지우지 못했어요.";

#[derive(Debug, Clone)]
pub struct StoreError(pub String);

impl StoreError {
    /// 저장된 값을 풀지 못함(키체인 값이 UTF-8이 아님 등) — load 상태 decrypt_error.
    pub fn decrypt() -> Self {
        StoreError(DECRYPT_ERROR.into())
    }
    pub fn is_decrypt(&self) -> bool {
        self.0 == DECRYPT_ERROR
    }
}

pub trait SecretStore: Send + Sync {
    fn read(&self) -> Result<Option<String>, StoreError>;
    fn write(&self, value: &str) -> Result<(), StoreError>;
    fn delete(&self) -> Result<(), StoreError>;
}

/// OS 키체인(macOS Keychain·Windows Credential Manager) 항목 하나.
pub struct KeyringStore {
    service: &'static str,
}

impl Default for KeyringStore {
    fn default() -> Self {
        KeyringStore { service: KEYRING_SERVICE }
    }
}

impl KeyringStore {
    pub fn new() -> Self {
        Self::default()
    }

    /// macOS: 로그인 키체인(사용자별). Windows: 자격 증명 관리자 — 로밍 프로필로 다른 PC에 퍼지지 않게
    /// '이 컴퓨터에만'(Local)으로 저장함.
    fn entry(&self) -> Result<keyring::Entry, StoreError> {
        #[cfg(target_os = "windows")]
        {
            if let Err(e) = keyring::Entry::store_status() {
                return Err(StoreError(e.to_string()));
            }
            let mods = std::collections::HashMap::from([("persistence", "Local")]);
            let inner = keyring_core::Entry::new_with_modifiers(self.service, KEYRING_USER, &mods)
                .map_err(|e| StoreError(e.to_string()))?;
            Ok(keyring::Entry { inner })
        }
        #[cfg(not(target_os = "windows"))]
        {
            keyring::Entry::new(self.service, KEYRING_USER).map_err(|e| StoreError(e.to_string()))
        }
    }
}

impl SecretStore for KeyringStore {
    fn read(&self) -> Result<Option<String>, StoreError> {
        match self.entry()?.get_password() {
            Ok(value) => Ok(Some(value)),
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(keyring::Error::BadEncoding(_)) => Err(StoreError::decrypt()),
            Err(e) => Err(StoreError(e.to_string())),
        }
    }
    fn write(&self, value: &str) -> Result<(), StoreError> {
        self.entry()?.set_password(value).map_err(|e| match e {
            keyring::Error::TooLong(..) => StoreError(TOO_LONG_CODE.into()),
            e => StoreError(e.to_string()),
        })
    }
    fn delete(&self) -> Result<(), StoreError> {
        match self.entry()?.delete_credential() {
            Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
            Err(e) => Err(StoreError(e.to_string())),
        }
    }
}

/// 테스트·키체인 없는 OS용 메모리 저장소. 읽기/쓰기 실패를 흉내 낼 수 있음.
#[derive(Default)]
pub struct MemoryStore {
    value: Mutex<Option<String>>,
    fail_reads: AtomicBool,
    fail_writes: AtomicBool,
    decrypt_error: AtomicBool,
}

impl MemoryStore {
    pub fn new() -> Self {
        Self::default()
    }
    #[cfg(test)]
    pub fn with(value: impl Into<String>) -> Self {
        let store = Self::default();
        *store.value.lock().unwrap() = Some(value.into());
        store
    }
    pub fn value(&self) -> Option<String> {
        self.value.lock().unwrap_or_else(|e| e.into_inner()).clone()
    }
    #[cfg(test)]
    pub fn set_fail_reads(&self, fail: bool) {
        self.fail_reads.store(fail, Ordering::SeqCst);
    }
    #[cfg(test)]
    /// 쓰기와 지우기를 함께 실패시킴.
    pub fn set_fail_writes(&self, fail: bool) {
        self.fail_writes.store(fail, Ordering::SeqCst);
    }
    #[cfg(test)]
    pub fn set_decrypt_error(&self, fail: bool) {
        self.decrypt_error.store(fail, Ordering::SeqCst);
    }
}

impl SecretStore for MemoryStore {
    fn read(&self) -> Result<Option<String>, StoreError> {
        if self.fail_reads.load(Ordering::SeqCst) {
            return Err(StoreError("read failed".into()));
        }
        if self.decrypt_error.load(Ordering::SeqCst) {
            return Err(StoreError::decrypt());
        }
        Ok(self.value())
    }
    fn write(&self, value: &str) -> Result<(), StoreError> {
        if self.fail_writes.load(Ordering::SeqCst) {
            return Err(StoreError("write failed".into()));
        }
        *self.value.lock().unwrap_or_else(|e| e.into_inner()) = Some(value.to_string());
        Ok(())
    }
    fn delete(&self) -> Result<(), StoreError> {
        if self.fail_writes.load(Ordering::SeqCst) {
            return Err(StoreError("delete failed".into()));
        }
        *self.value.lock().unwrap_or_else(|e| e.into_inner()) = None;
        Ok(())
    }
}

#[derive(Serialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum LoadStatus {
    Missing,
    Loaded,
    ReadError,
    DecryptError,
    InvalidData,
}

#[derive(Clone)]
pub struct LoadResult {
    pub values: Option<Credentials>,
    pub status: LoadStatus,
}

/// 키가 로그에 새지 않게 값은 감춤.
impl std::fmt::Debug for LoadResult {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LoadResult")
            .field("values", &self.values.as_ref().map(|_| "<hidden>"))
            .field("status", &self.status)
            .finish()
    }
}

// --- 정규화(문자열·키 쌍·거래소 칸·연결 기록·저장 값) ---

/// 문자열만 믿음. 숫자·dict 같은 깨진 값을 키처럼 둔갑시키지 않음.
fn text(value: Option<&Value>) -> String {
    match value {
        Some(Value::String(s)) => s.clone(),
        _ => String::new(),
    }
}

/// 키 한 쌍. dict가 아니거나 둘 다 비면 None. 한쪽만 있어도 버리지 않음.
fn pair_value(raw: Option<&Value>) -> Option<KeyPair> {
    let map = raw?.as_object()?;
    let pair = KeyPair { api_key: text(map.get("api_key")), api_secret: text(map.get("api_secret")) };
    (!pair.api_key.is_empty() || !pair.api_secret.is_empty()).then_some(pair)
}

/// 아는 거래소만. 모르는 이름·깨진 항목은 조용히 버림.
fn exchanges_value(raw: Option<&Value>) -> BTreeMap<String, KeyPair> {
    let Some(map) = raw.and_then(Value::as_object) else { return BTreeMap::new() };
    EXCHANGES.iter().filter_map(|name| pair_value(map.get(*name)).map(|p| (name.to_string(), p))).collect()
}

/// 지난 관측값일 뿐임 — 등록된 IP나 지금 인증이 통한다는 증거로 쓰지 않음.
fn history_value(raw: Option<&Value>) -> BTreeMap<String, IpObservation> {
    let Some(map) = raw.and_then(Value::as_object) else { return BTreeMap::new() };
    EXCHANGES
        .iter()
        .filter_map(|name| {
            let item = map.get(*name)?;
            if !matches!(item.get("public_ipv4"), Some(Value::String(_))) {
                return None;
            }
            valid_ip_observation(item).map(|o| (name.to_string(), o))
        })
        .collect()
}

fn payload_from_value(values: &Value) -> Credentials {
    Credentials {
        member_key: text(values.get("member_key")),
        exchanges: exchanges_value(values.get("exchanges")),
        connection_history: history_value(values.get("connection_history")),
    }
}

fn observation_value(o: &IpObservation) -> Value {
    let mut map = Map::new();
    map.insert("public_ipv4".into(), Value::String(o.public_ipv4.clone()));
    map.insert(
        "observed_at".into(),
        serde_json::Number::from_f64(o.observed_at).map(Value::Number).unwrap_or(Value::Null),
    );
    Value::Object(map)
}

/// 타입이 있는 값을 저장 규칙대로 다시 거름.
pub fn normalize(c: &Credentials) -> Credentials {
    let exchanges: BTreeMap<String, KeyPair> = EXCHANGES
        .iter()
        .filter_map(|name| {
            let p = c.exchanges.get(*name)?;
            (!p.api_key.is_empty() || !p.api_secret.is_empty()).then(|| (name.to_string(), p.clone()))
        })
        .collect();
    let connection_history = EXCHANGES
        .iter()
        .filter_map(|name| {
            let o = c.connection_history.get(*name)?;
            valid_ip_observation(&observation_value(o)).map(|o| (name.to_string(), o))
        })
        .collect();
    Credentials { member_key: c.member_key.clone(), exchanges, connection_history }
}

fn number_text(v: f64) -> String {
    // 정수였던 관측 시각은 정수로 씀(읽은 타입을 그대로 다시 씀).
    if v.fract() == 0.0 && v.abs() < 9_007_199_254_740_992.0 {
        format!("{}", v as i64)
    } else {
        py_float(v)
    }
}

/// v2 JSON 문자열 — 키 순서 version·member_key·exchanges·connection_history, 거래소는 고정 순서.
pub fn to_json(c: &Credentials) -> String {
    let c = normalize(c);
    let mut out = format!("{{\"version\": 2, \"member_key\": {}, \"exchanges\": {{", py_str(&c.member_key, false));
    let pairs: Vec<String> = EXCHANGES
        .iter()
        .filter_map(|name| {
            let p = c.exchanges.get(*name)?;
            Some(format!(
                "{}: {{\"api_key\": {}, \"api_secret\": {}}}",
                py_str(name, false),
                py_str(&p.api_key, false),
                py_str(&p.api_secret, false)
            ))
        })
        .collect();
    out.push_str(&pairs.join(", "));
    out.push('}');
    if !c.connection_history.is_empty() {
        let items: Vec<String> = EXCHANGES
            .iter()
            .filter_map(|name| {
                let o = c.connection_history.get(*name)?;
                Some(format!(
                    "{}: {{\"public_ipv4\": {}, \"observed_at\": {}}}",
                    py_str(name, false),
                    py_str(&o.public_ipv4, false),
                    number_text(o.observed_at)
                ))
            })
            .collect();
        out.push_str(", \"connection_history\": {");
        out.push_str(&items.join(", "));
        out.push('}');
    }
    out.push('}');
    out
}

/// 사용자에게 보이는 불러오기 상태에는 OS·암호화 오류 원문이나 비밀 값을 담지 않음.
pub fn load_result(store: &dyn SecretStore) -> LoadResult {
    let raw = match store.read() {
        Ok(None) => return LoadResult { values: None, status: LoadStatus::Missing },
        Ok(Some(raw)) => raw,
        Err(e) if e.is_decrypt() => return LoadResult { values: None, status: LoadStatus::DecryptError },
        Err(_) => return LoadResult { values: None, status: LoadStatus::ReadError },
    };
    let data = match serde_json::from_str::<Value>(&raw) {
        Ok(Value::Object(map)) => map,
        _ => return LoadResult { values: None, status: LoadStatus::InvalidData },
    };
    let data = if data.contains_key("version") {
        Value::Object(data)
    } else {
        // v1: 바이낸스 한 쌍뿐이었음. 회원 키는 그 자리에 둠.
        let v1 = Value::Object(data.clone());
        let mut exchanges = Map::new();
        if let Some(pair) = pair_value(Some(&v1)) {
            exchanges.insert("binance".into(), serde_json::to_value(pair).unwrap_or(Value::Null));
        }
        let mut migrated = Map::new();
        migrated.insert("version".into(), Value::from(2));
        migrated.insert("member_key".into(), data.get("member_key").cloned().unwrap_or(Value::Null));
        migrated.insert("exchanges".into(), Value::Object(exchanges));
        migrated.insert("connection_history".into(), data.get("connection_history").cloned().unwrap_or(Value::Null));
        Value::Object(migrated)
    };
    LoadResult { values: Some(payload_from_value(&data)), status: LoadStatus::Loaded }
}

pub fn save(store: &dyn SecretStore, c: &Credentials) -> Result<(), String> {
    store.write(&to_json(c)).map_err(|e| if e.0 == TOO_LONG_CODE { TOO_LONG } else { SAVE_FAILED }.to_string())
}

pub fn clear(store: &dyn SecretStore) -> Result<(), String> {
    store.delete().map_err(|_| CLEAR_FAILED.to_string())
}

/// 이 거래소 칸만 바꿈. 다른 거래소 키와 회원 키는 그대로 둠.
///
/// 마지막 거래소 키를 지워도 회원 키만 남은 저장소는 지우지 않음 — 거래소를 빼는 일이
/// 회원 연결까지 잊게 하면 안 됨. 읽지 못한 저장소는 덮어쓰지 않음. 기준은 늘 디스크(키체인)임.
pub fn apply_exchange_choice(
    store: &dyn SecretStore,
    ex: KeySlot,
    remember: bool,
    values: &Credentials,
) -> Result<Option<Credentials>, String> {
    let exchange = ex.as_str().to_string();
    let loaded = load_result(store);
    if !matches!(loaded.status, LoadStatus::Missing | LoadStatus::Loaded) {
        return Err(UNREADABLE_NOT_CHANGED.into());
    }
    let mut result = normalize(&loaded.values.unwrap_or_default());
    let mut history = result.connection_history.clone();
    if remember {
        let selected = normalize(&Credentials { exchanges: values.exchanges.clone(), ..Default::default() })
            .exchanges
            .remove(&exchange);
        let Some(selected) = selected.filter(|p| {
            !crate::connection::py_strip(&p.api_key).is_empty()
                && !crate::connection::py_strip(&p.api_secret).is_empty()
        }) else {
            return Err(INCOMPLETE_PAIR.into());
        };
        result.exchanges.insert(exchange.clone(), selected);
        if !values.member_key.is_empty() {
            result.member_key = values.member_key.clone();
        }
        // 타입이 있는 값에서는 '이 거래소 항목이 들어왔다'가 곧 키가 있다는 뜻임.
        if let Some(incoming) = values.connection_history.get(&exchange) {
            match valid_ip_observation(&observation_value(incoming)) {
                Some(observation) => {
                    history.insert(exchange.clone(), observation);
                }
                None => {
                    history.remove(&exchange);
                }
            }
        }
    } else {
        result.exchanges.remove(&exchange);
        history.remove(&exchange);
    }
    result.connection_history = history;
    if !result.exchanges.is_empty() || !result.member_key.is_empty() || !result.connection_history.is_empty() {
        save(store, &result)?;
        return Ok(Some(result));
    }
    clear(store)?;
    Ok(None)
}

/// 읽을 수 없는 저장 항목을 지움. 키체인에는 따로 백업할 파일이 없으니, 내용이 깨진 항목일 때만 지움.
/// 읽기 자체가 실패한 것(키체인 접근 거부·잠김·일시 오류)은 지우지 않음 — 항목은 멀쩡할 수 있음.
pub fn reset_unreadable(store: &dyn SecretStore) -> Result<(), String> {
    let loaded = load_result(store);
    if !matches!(loaded.status, LoadStatus::DecryptError | LoadStatus::InvalidData) {
        return Err(STATE_CHANGED.into());
    }
    clear(store)
}

#[cfg(test)]
mod tests;

/// 회원 키만 바꿈. 저장 항목이 있을 때만 씀(키를 하나도 기억하지 않은 사용자의 회원 키를 몰래 저장하지 않음).
pub fn update_member_key(store: &dyn SecretStore, member_key: &str) -> Result<Option<Credentials>, String> {
    let loaded = load_result(store);
    match loaded.status {
        LoadStatus::Missing => Ok(None),
        LoadStatus::Loaded => {
            let mut values = normalize(&loaded.values.unwrap_or_default());
            if values.member_key == member_key {
                return Ok(Some(values));
            }
            values.member_key = member_key.to_string();
            save(store, &values)?;
            Ok(Some(values))
        }
        _ => Err(UNREADABLE_NOT_CHANGED.into()),
    }
}

#[cfg(test)]
mod slot_tests;
