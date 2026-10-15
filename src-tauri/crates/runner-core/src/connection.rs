//! 연결 준비 — 세 줄 판정·클립보드 키·공인 IP·검사 지문·키 마스킹.
use hmac::{Hmac, KeyInit, Mac};
use once_cell::sync::Lazy;
use regex::Regex;
use serde::Serialize;
use serde_json::{json, Value};
use sha2::Sha256;
use std::net::Ipv4Addr;
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use crate::brokers::Broker;
use crate::outbox::py_dumps;
use crate::types::{Credentials, IpObservation, RunMode};

pub const PUBLIC_IPV4_URL: &str = "https://api4.ipify.org?format=json";
pub const PUBLIC_IP_MAX_AGE_SECS: f64 = 120.0;
pub const PREFLIGHT_MAX_AGE_SECS: f64 = 120.0;

/// 정적 허용 목록 — 다른 주소는 열지 않음.
pub const API_MANAGEMENT_PAGES: [(&str, &str); 2] = [
    ("upbit", "https://www.upbit.com/mypage/open_api_management"),
    ("bithumb", "https://www.bithumb.com/react/api-support/management-api"),
];
/// 연결 도우미의 모든 단계 맨 위에 두는 '거래소 열기' 바로가기.
pub const EXCHANGE_HOME_PAGES: [(&str, &str); 2] =
    [("upbit", "https://upbit.com/"), ("bithumb", "https://www.bithumb.com/")];

pub const VALIDATE_IP_FAILURE: &str =
    "이 기기 주소를 확인하지 못했어요. 네트워크를 확인한 뒤 [다시 확인]을 눌러 주세요.";
pub const DETECT_IP_FAILURE: &str =
    "이 기기 주소를 확인하지 못했어요. VPN이나 인터넷 연결을 확인한 뒤 [다시 확인]을 눌러 주세요.";
const CACHE_OK_REASON: &str = "이 기기 주소를 확인했어요.";
const GENERIC_PREFLIGHT_REASON: &str =
    "네트워크 또는 인증 응답을 확인하지 못했어요. 인터넷 연결·허용 IP·API 권한을 확인한 뒤 다시 눌러 주세요.";
pub const HISTORY_MAX_FUTURE_SKEW_SECS: f64 = 300.0;

/// 프로세스마다 새로 뽑는 지문 소금 — 지문은 이 앱 안에서만 비교함.
static FINGERPRINT_SALT: Lazy<[u8; 32]> = Lazy::new(|| {
    let mut salt = [0u8; 32];
    rand::fill(&mut salt);
    salt
});

#[derive(Serialize, Clone, Copy, PartialEq, Eq, Debug)]
#[serde(rename_all = "lowercase")]
pub enum Check {
    Ok,
    Bad,
    Wait,
    Na,
}

impl Check {
    pub fn as_str(self) -> &'static str {
        match self {
            Check::Ok => "ok",
            Check::Bad => "bad",
            Check::Wait => "wait",
            Check::Na => "na",
        }
    }
}

#[derive(Serialize, Clone, Debug, PartialEq)]
pub struct CheckView {
    pub auth: Check,
    pub ip: Check,
    pub order: Check,
    pub connected: bool,
    pub problem: String,
    pub headline: String,
}

// 키는 받아들였지만 호출한 IP가 허용 목록에 없음 — 업비트 no_authorization_ip, 빗썸 NotAllowIP.
const IP_REJECTED: [&str; 2] = ["no_authorization_ip", "NotAllowIP"];
// 키 자체를 받아들이지 않음(틀린 키·시크릿 짝 불일치·만료).
const KEY_REJECTED: [&str; 4] =
    ["invalid_access_key", "jwt_verification", "no_authorization_token", "expired_access_key"];
// 키와 IP는 통과했고 주문하기 권한이 없음.
const SCOPE_REJECTED: [&str; 1] = ["out_of_scope"];
// 거래소가 주문 내용(잔고·최소 금액)까지 따져 본 오류 — 키·IP·주문 권한은 이미 통과했다는 뜻임.
const ORDER_EVALUATED: [&str; 5] =
    ["insufficient_funds_bid", "insufficient_funds_ask", "insufficient_funds", "over_krw_funds_bid", "under_min_total"];

fn view(auth: Check, ip: Check, order: Check, connected: bool, problem: &str, headline: &str) -> CheckView {
    CheckView { auth, ip, order, connected, problem: problem.into(), headline: headline.into() }
}

/// 검사 결과 하나를 도우미의 세 줄(키 인증·허용 IP·주문 권한)과 제목으로 옮김.
pub fn check_view(exchange: &str, ok: bool, error_name: &str) -> CheckView {
    use Check::*;
    let name = error_name;
    let order_ok = if exchange == "upbit" { Ok } else { Na };
    if ok || ORDER_EVALUATED.contains(&name) || name.starts_with("under_min_total") {
        return view(Ok, Ok, order_ok, true, if ok { "" } else { "order_size" }, "연결 완료");
    }
    if IP_REJECTED.contains(&name) {
        return view(Ok, Bad, Wait, false, "ip", "허용 IP 확인 필요");
    }
    if KEY_REJECTED.contains(&name) {
        return view(Bad, Wait, Wait, false, "auth", "키 확인 필요");
    }
    if SCOPE_REJECTED.contains(&name) {
        return view(Ok, Ok, Bad, false, "order", "주문 권한 확인 필요");
    }
    view(Wait, Wait, Wait, false, if name == "network" { "network" } else { "other" }, "연결 확인 필요")
}

/// 업비트 키는 영문·숫자뿐임. '_' '-'는 받지 않음 — 토큰·JWT·비밀번호 같은 다른 복사본을 키로
/// 잘못 받지 않게.
static UPBIT_KEY: Lazy<Regex> = Lazy::new(|| Regex::new(r"^[A-Za-z0-9]{30,100}$").unwrap());
/// 빗썸(API 2.0) 키 모양 — Access Key는 영문·숫자 46자, Secret Key는 62바이트 base64(82자 + "=="),
/// 실제 발급 키로 확인한 형식임. 모양이 정확해 비밀번호 관리자에서 복사한 비밀번호 같은 값은 받지 않고,
/// 모양으로 어느 칸인지도 가림(시크릿을 먼저 복사해도 맞는 칸에). 다른 모양이면 직접 붙여 넣음.
static BITHUMB_ACCESS: Lazy<Regex> = Lazy::new(|| Regex::new(r"^[A-Za-z0-9]{46}$").unwrap());
static BITHUMB_SECRET: Lazy<Regex> = Lazy::new(|| Regex::new(r"^[A-Za-z0-9+/]{82}==$").unwrap());

/// 클립보드 키가 어느 칸 것인지 — 모양으로 알 수 있으면 Some(true: Secret, false: Access), 모르면 None(복사 순서).
pub fn clipboard_key_is_secret(exchange: &str, key: &str) -> Option<bool> {
    if exchange != "bithumb" {
        return None;
    }
    if BITHUMB_SECRET.is_match(key) {
        Some(true)
    } else if BITHUMB_ACCESS.is_match(key) {
        Some(false)
    } else {
        None
    }
}

/// Python `str.strip()`과 같은 공백(유니코드 공백 + \x1c-\x1f).
pub(crate) fn py_strip(text: &str) -> &str {
    text.trim_matches(|c: char| c.is_whitespace() || ('\u{1c}'..='\u{1f}').contains(&c))
}

/// 거래소 발급 창의 [복사]로 클립보드에 들어온 값이 그 거래소 키처럼 생겼는지.
/// 무작위로 발급된 키라 영문과 숫자가 함께 섞여 있어야 함.
pub fn clipboard_key(exchange: &str, text: &str) -> String {
    let value = py_strip(text);
    let shaped = if exchange == "upbit" {
        UPBIT_KEY.is_match(value)
    } else {
        BITHUMB_ACCESS.is_match(value) || BITHUMB_SECRET.is_match(value)
    };
    let mixed = value.chars().any(|c| c.is_ascii_digit()) && value.chars().any(|c| c.is_ascii_alphabetic());
    if shaped && mixed {
        value.to_string()
    } else {
        String::new()
    }
}

pub fn api_management_url(ex: &str) -> Result<&'static str, String> {
    API_MANAGEMENT_PAGES
        .iter()
        .find(|(name, _)| *name == ex)
        .map(|(_, url)| *url)
        .ok_or_else(|| "이 거래소의 공식 API 관리 페이지를 확인하지 못했어요.".to_string())
}

pub fn exchange_home_url(ex: &str) -> Result<&'static str, String> {
    EXCHANGE_HOME_PAGES
        .iter()
        .find(|(name, _)| *name == ex)
        .map(|(_, url)| *url)
        .ok_or_else(|| "이 거래소의 공식 홈페이지를 확인하지 못했어요.".to_string())
}

fn in_net(ip: Ipv4Addr, net: [u8; 4], prefix: u32) -> bool {
    let mask = if prefix == 0 { 0 } else { u32::MAX << (32 - prefix) };
    (u32::from(ip) & mask) == (u32::from(Ipv4Addr::from(net)) & mask)
}

/// Python `ipaddress.IPv4Address.is_global`(3.13 표): 사설·예약 목록과 100.64/10을 뺌.
pub(crate) fn ipv4_is_global(ip: Ipv4Addr) -> bool {
    const PRIVATE: [([u8; 4], u32); 14] = [
        ([0, 0, 0, 0], 8),
        ([10, 0, 0, 0], 8),
        ([127, 0, 0, 0], 8),
        ([169, 254, 0, 0], 16),
        ([172, 16, 0, 0], 12),
        ([192, 0, 0, 0], 24),
        ([192, 0, 0, 170], 31),
        ([192, 0, 2, 0], 24),
        ([192, 168, 0, 0], 16),
        ([198, 18, 0, 0], 15),
        ([198, 51, 100, 0], 24),
        ([203, 0, 113, 0], 24),
        ([240, 0, 0, 0], 4),
        ([255, 255, 255, 255], 32),
    ];
    const EXCEPTIONS: [([u8; 4], u32); 2] = [([192, 0, 0, 9], 32), ([192, 0, 0, 10], 32)];
    let private =
        PRIVATE.iter().any(|(n, p)| in_net(ip, *n, *p)) && !EXCEPTIONS.iter().any(|(n, p)| in_net(ip, *n, *p));
    !in_net(ip, [100, 64, 0, 0], 10) && !private
}

pub fn validate_public_ipv4(v: &str) -> Result<String, String> {
    match py_strip(v).parse::<Ipv4Addr>() {
        Ok(ip) if ipv4_is_global(ip) && !ip.is_multicast() => Ok(ip.to_string()),
        _ => Err(VALIDATE_IP_FAILURE.to_string()),
    }
}

/// 인증 없는 요청 한 번만 보냄 — 대체 주소·재시도·키를 실은 세션은 쓰지 않음.
///
/// 리다이렉트를 따라가지 않도록 이 요청만을 위한 클라이언트를 따로 만듦(넘겨받은 클라이언트는
/// 리다이렉트 정책을 요청마다 끌 수 없음).
pub async fn detect_public_ipv4(_http: &reqwest::Client) -> Result<String, String> {
    detect_public_ipv4_from(PUBLIC_IPV4_URL).await
}

/// 주소를 받는 판 — 테스트가 가짜 서버를 가리킴.
pub async fn detect_public_ipv4_from(url: &str) -> Result<String, String> {
    // 네트워크 오류 문장은 그대로 내보내지 않음 — 프록시 URL에 비밀번호가 들어 있을 수 있음.
    let fail = |_| DETECT_IP_FAILURE.to_string();
    let client = crate::net::harden(reqwest::Client::builder())
        .connect_timeout(Duration::from_secs(3))
        .timeout(Duration::from_secs(5))
        .build()
        .map_err(fail)?;
    let response = client.get(url).send().await.map_err(fail)?;
    if response.status().as_u16() != 200 {
        return Err(DETECT_IP_FAILURE.to_string());
    }
    let body: Value = response.json().await.map_err(fail)?;
    match body.get("ip") {
        Some(Value::String(ip)) => validate_public_ipv4(ip).map_err(|_| DETECT_IP_FAILURE.to_string()),
        _ => Err(DETECT_IP_FAILURE.to_string()),
    }
}

fn unix_now() -> f64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs_f64()).unwrap_or(0.0)
}

/// 벽시계로도 신선한지. macOS의 Instant는 잠자기 동안 멈춰서, 잠들기 전 결과가 깨어난 뒤에도
/// 방금 것처럼 보임(집에서 확인하고 덮은 뒤 사무실에서 열면 옛 IP 등).
fn wall_within(checked_wall: f64, max_age: f64) -> bool {
    let elapsed = unix_now() - checked_wall;
    (0.0..max_age).contains(&elapsed)
}

fn within(checked_at: Instant, now: Instant, max_age: f64) -> bool {
    // 0 <= elapsed < max_age — 기준보다 이른 now(음수 경과)는 신선하지 않음.
    now.checked_duration_since(checked_at).is_some_and(|elapsed| elapsed.as_secs_f64() < max_age)
}

#[derive(Clone, Debug)]
pub struct PublicIpResult {
    pub ok: bool,
    pub address: String,
    pub reason: String,
    pub checked_at: Instant,
    pub observed_at: f64,
    pub changed: bool,
    /// 확인한 벽시계 시각(유닉스 초) — [`wall_within`].
    pub checked_wall: f64,
}

impl PublicIpResult {
    /// 지금 조회한 결과(checked_at=지금, observed_at=지금의 유닉스 초).
    pub fn new(ok: bool, address: impl Into<String>, reason: impl Into<String>) -> Self {
        PublicIpResult {
            ok,
            address: address.into(),
            reason: reason.into(),
            checked_at: Instant::now(),
            observed_at: unix_now(),
            changed: false,
            checked_wall: unix_now(),
        }
    }
    pub fn fresh(&self) -> bool {
        self.fresh_at(Instant::now()) && wall_within(self.checked_wall, PUBLIC_IP_MAX_AGE_SECS)
    }
    /// 주어진 시각으로 판정하는 판(단조 시계만 봄, 벽시계 확인은 부르는 쪽이 함).
    pub fn fresh_at(&self, now: Instant) -> bool {
        within(self.checked_at, now, PUBLIC_IP_MAX_AGE_SECS)
    }
}

pub type IpToken = (u64, u64);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BeginOutcome {
    Shared(IpToken),
    Cached,
    Request(IpToken),
}

/// PC 범위의 짧은 캐시와 singleflight 요청 세대.
#[derive(Default)]
pub struct PublicIpCache {
    result: Option<PublicIpResult>,
    pub generation: u64,
    sequence: u64,
    active: Option<IpToken>,
    previous_address: String,
}

impl PublicIpCache {
    pub fn new() -> Self {
        Self::default()
    }

    /// 지난 관측값으로만 되살림 — 현재 주소나 거래소에 등록된 주소로 쓰지 않음.
    pub fn seed_previous(&mut self, history: Option<&IpObservation>) -> bool {
        let Some(h) = history else { return false };
        match valid_observation(&Value::String(h.public_ipv4.clone()), &json_f64(h.observed_at)) {
            Some(observation) => {
                self.previous_address = observation.public_ipv4;
                true
            }
            None => false,
        }
    }

    /// 키체인에 저장된 JSON 값을 그대로 받는 판.
    pub fn seed_previous_value(&mut self, history: &Value) -> bool {
        match valid_ip_observation(history) {
            Some(observation) => {
                self.previous_address = observation.public_ipv4;
                true
            }
            None => false,
        }
    }

    pub fn busy(&self) -> bool {
        self.active.is_some()
    }

    pub fn begin(&mut self, force: bool) -> BeginOutcome {
        self.begin_at(force, Instant::now())
    }

    pub fn begin_at(&mut self, force: bool, now: Instant) -> BeginOutcome {
        if let Some(token) = self.active {
            return BeginOutcome::Shared(token);
        }
        if !force && self.result.as_ref().is_some_and(|r| r.fresh_at(now)) {
            return BeginOutcome::Cached;
        }
        self.result = None;
        self.sequence += 1;
        let token = (self.generation, self.sequence);
        self.active = Some(token);
        BeginOutcome::Request(token)
    }

    pub fn finish(&mut self, token: IpToken, value: Result<String, String>) -> Option<PublicIpResult> {
        self.finish_at(token, value, Instant::now())
    }

    pub fn finish_at(&mut self, token: IpToken, value: Result<String, String>, now: Instant) -> Option<PublicIpResult> {
        if Some(token) != self.active {
            return None;
        }
        self.active = None;
        if token.0 != self.generation {
            return None;
        }
        let address = match value {
            Ok(v) => validate_public_ipv4(&v).ok(),
            Err(_) => None,
        };
        let ok = address.is_some();
        let address = address.unwrap_or_default();
        let changed = ok && !self.previous_address.is_empty() && address != self.previous_address;
        if ok {
            self.previous_address = address.clone();
        }
        // 전송 계층·프록시가 낸 실패 문장은 캐시하지도 화면에 보이지도 않음.
        let reason = if ok { CACHE_OK_REASON } else { DETECT_IP_FAILURE };
        let result = PublicIpResult {
            ok,
            address,
            reason: reason.into(),
            checked_at: now,
            observed_at: unix_now(),
            changed,
            checked_wall: unix_now(),
        };
        self.result = Some(result.clone());
        Some(result)
    }

    /// 진행 중인 요청이 돌아올 때까지 singleflight는 잡아 둠. 그 요청의 세대는 finish가 버리므로
    /// 바깥으로 요청이 동시에 두 개 나가지 않음.
    pub fn invalidate(&mut self) {
        self.generation += 1;
        self.result = None;
    }

    pub fn result(&self) -> Option<&PublicIpResult> {
        self.result.as_ref()
    }
}

pub fn public_ip_status(r: Option<&PublicIpResult>) -> String {
    public_ip_status_at(r, Instant::now())
}

pub fn public_ip_status_at(r: Option<&PublicIpResult>, now: Instant) -> String {
    let Some(result) = r else {
        return "아직 주소를 확인하지 않았어요. 허용 IP 등록은 거래소 공식 페이지에서 직접 해요.".into();
    };
    if !result.fresh_at(now) {
        return "주소를 확인한 지 오래돼 만료됐어요. [다시 확인]을 눌러 현재 주소를 받은 뒤 복사하세요.".into();
    }
    if !result.ok {
        return result.reason.clone();
    }
    let observed = local_hms(result.observed_at);
    let changed = if result.changed {
        " · 전에 확인한 주소와 달라요. 거래소의 허용 IP를 고친 뒤 다시 연결을 확인하세요."
    } else {
        ""
    };
    format!("{} · {} 조회 당시 주소예요(2분 동안 유효).{}", result.address, observed, changed)
}

fn local_hms(unix: f64) -> String {
    use chrono::TimeZone;
    if !unix.is_finite() {
        return String::new();
    }
    match chrono::Local.timestamp_opt(unix.floor() as i64, 0) {
        chrono::LocalResult::Single(t) | chrono::LocalResult::Ambiguous(t, _) => t.format("%H:%M:%S").to_string(),
        chrono::LocalResult::None => String::new(),
    }
}

/// Python `str(macro.get("exchange") or "binance").lower()`.
fn macro_exchange(m: &Value) -> String {
    match m.get("exchange") {
        Some(Value::String(s)) if !s.is_empty() => s.to_lowercase(),
        Some(Value::Bool(true)) => "true".into(),
        Some(Value::Number(n)) if n.as_f64() != Some(0.0) => n.to_string().to_lowercase(),
        Some(v @ Value::Array(a)) if !a.is_empty() => v.to_string().to_lowercase(),
        Some(v @ Value::Object(o)) if !o.is_empty() => v.to_string().to_lowercase(),
        _ => "binance".into(),
    }
}

pub fn preflight_fingerprint(m: &Value, mode: RunMode, c: &Credentials) -> String {
    let exchange = macro_exchange(m);
    let pair = match c.exchanges.get(&exchange) {
        Some(p) => json!({"api_key": p.api_key, "api_secret": p.api_secret}),
        None => json!({}),
    };
    let payload = py_dumps(&json!([m, mode.as_str(), pair]), false, true, true);
    let mut mac = Hmac::<Sha256>::new_from_slice(&FINGERPRINT_SALT[..]).expect("hmac key");
    mac.update(payload.as_bytes());
    hex::encode(mac.finalize().into_bytes())
}

static UNSAFE_REASON: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"(?i)https?://|authorization|bearer\s|signature\s*[=:]|proxy|[^\s/:@]+:[^\s/@]+@|eyJ[A-Za-z0-9_-]+\.")
        .unwrap()
});

/// 이유 문장을 화면이나 로그에 내기 전에 넘겨받은 키를 모두 가림.
pub fn safe_preflight_reason(reason: &str, c: &Credentials) -> String {
    // 거래소 라이브러리는 네트워크 예외를 이유 문장에 섞어 돌려줄 때가 있음.
    // 거기 담긴 서명 URL·프록시 인증 정보·인증 헤더는 절대 보이지 않음.
    if UNSAFE_REASON.is_match(reason) {
        return GENERIC_PREFLIGHT_REASON.into();
    }
    mask_secrets(reason, c).chars().take(600).collect()
}

/// 글 안에 들어간 실제 키 값(회원 키·거래소 키·시크릿)을 "[키 숨김]"으로 바꿈. 긴 값부터.
pub fn mask_secrets(text: &str, c: &Credentials) -> String {
    let mut values: Vec<&str> = vec![c.member_key.trim()];
    for pair in c.exchanges.values() {
        values.push(pair.api_key.trim());
        values.push(pair.api_secret.trim());
    }
    values.retain(|v| !v.is_empty());
    values.sort();
    values.dedup();
    values.sort_by_key(|v| std::cmp::Reverse(v.chars().count()));
    let mut text = text.to_string();
    for value in values {
        text = text.replace(value, "[키 숨김]");
    }
    text
}

fn json_f64(v: f64) -> Value {
    serde_json::Number::from_f64(v).map(Value::Number).unwrap_or(Value::Null)
}

fn valid_observation(address: &Value, observed_at: &Value) -> Option<IpObservation> {
    let observed = match observed_at {
        Value::Number(n) => n.as_f64()?,
        _ => return None,
    };
    if !(observed > 0.0 && observed <= unix_now() + HISTORY_MAX_FUTURE_SKEW_SECS) || !observed.is_finite() {
        return None;
    }
    let address = match address {
        Value::String(s) => validate_public_ipv4(s).ok()?,
        _ => return None,
    };
    Some(IpObservation { public_ipv4: address, observed_at: observed })
}

/// 믿을 수 없는 로컬 메타데이터로 사설 IP나 유한하지 않은 시각이 끼어들지 못하게 함.
pub fn valid_ip_observation(h: &Value) -> Option<IpObservation> {
    let map = h.as_object()?;
    valid_observation(map.get("public_ipv4").unwrap_or(&Value::Null), map.get("observed_at").unwrap_or(&Value::Null))
}

#[derive(Clone)]
pub struct PreflightResult {
    pub ok: bool,
    pub reason: String,
    pub fingerprint: String,
    pub checked_at: Instant,
    /// 확인한 벽시계 시각(유닉스 초) — [`wall_within`].
    pub checked_wall: f64,
    pub error_name: String,
    pub broker: Option<Arc<dyn Broker>>,
}

impl PreflightResult {
    pub fn new(ok: bool, reason: impl Into<String>, fingerprint: impl Into<String>) -> Self {
        PreflightResult {
            ok,
            reason: reason.into(),
            fingerprint: fingerprint.into(),
            checked_at: Instant::now(),
            checked_wall: unix_now(),
            error_name: String::new(),
            broker: None,
        }
    }
    pub fn matches(&self, fingerprint: &str) -> bool {
        self.matches_at(fingerprint, Instant::now()) && wall_within(self.checked_wall, PREFLIGHT_MAX_AGE_SECS)
    }
    /// 주어진 시각으로 판정하는 판(단조 시계만 봄, 벽시계 확인은 부르는 쪽이 함).
    pub fn matches_at(&self, fingerprint: &str, now: Instant) -> bool {
        self.ok && self.fingerprint == fingerprint && within(self.checked_at, now, PREFLIGHT_MAX_AGE_SECS)
    }
}

/// 바뀌었거나 저장하지 않은 키를 몰래 저장하지 않고 메타데이터만 준비함.
///
/// IP는 검사가 성공했을 때 ipify로 본 값일 뿐임. 거래소에 등록된 IP도 아니고,
/// 두 요청이 같은 경로로 나갔다는 증거도 아님.
pub fn with_successful_ip_history(
    remembered: &Credentials,
    m: &Value,
    mode: RunMode,
    current: &Credentials,
    ip: &PublicIpResult,
    pre: &PreflightResult,
) -> Option<Credentials> {
    with_successful_ip_history_at(remembered, m, mode, current, ip, pre, Instant::now())
}

pub fn with_successful_ip_history_at(
    remembered: &Credentials,
    m: &Value,
    mode: RunMode,
    current: &Credentials,
    ip: &PublicIpResult,
    pre: &PreflightResult,
    now: Instant,
) -> Option<Credentials> {
    if mode != RunMode::Live {
        return None;
    }
    let exchange = macro_exchange(m);
    api_management_url(&exchange).ok()?;
    let saved_pair = remembered.exchanges.get(&exchange)?;
    if current.exchanges.get(&exchange) != Some(saved_pair)
        || saved_pair.api_key.is_empty()
        || saved_pair.api_secret.is_empty()
    {
        return None;
    }
    if !ip.ok || !ip.fresh_at(now) || !pre.matches_at(&preflight_fingerprint(m, mode, current), now) {
        return None;
    }
    let observation = valid_observation(&Value::String(ip.address.clone()), &json_f64(ip.observed_at))?;
    if remembered.connection_history.get(&exchange) == Some(&observation) {
        return None;
    }
    let mut updated = remembered.clone();
    updated.connection_history.insert(exchange, observation);
    Some(updated)
}

#[cfg(test)]
mod tests;
