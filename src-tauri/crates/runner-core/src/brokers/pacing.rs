//! 거래소 요청 속도 — 요청 한도(429·418)로 쉬는 시간(거래소 주소별 공유)과 주문 결과 조회 간격.

/// 요청 한도(429·418)로 쉬는 시간 — 거래소 기준 주소별로 모든 실행이 함께 씀. 같은 IP로 나가는 실행들이
/// 한 실행의 차단을 모른 채 계속 두드리면 바이낸스는 차단 시간을 늘림(최대 며칠).
fn cooldowns() -> &'static std::sync::Mutex<std::collections::HashMap<String, std::time::Instant>> {
    static COOL: std::sync::OnceLock<std::sync::Mutex<std::collections::HashMap<String, std::time::Instant>>> =
        std::sync::OnceLock::new();
    COOL.get_or_init(Default::default)
}

/// 쉬는 시간을 나누는 열쇠. 테스트는 테스트(스레드)마다 따로 — 가짜 서버 포트가 다시 쓰여도 다른 테스트의
/// 쉬는 시간이 남지 않게.
pub(crate) fn cool_key(base: &str) -> String {
    #[cfg(test)]
    {
        static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        thread_local!(static SCOPE: u64 = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed));
        format!("{base}#{}", SCOPE.with(|s| *s))
    }
    #[cfg(not(test))]
    base.to_string()
}

/// 이 주소로 요청을 쉬는 중이면 남은 초(올림).
pub(crate) fn cooling(base: &str) -> Option<u64> {
    let now = std::time::Instant::now();
    let until = *cooldowns().lock().unwrap_or_else(|e| e.into_inner()).get(&cool_key(base))?;
    (until > now).then(|| (until - now).as_secs_f64().ceil() as u64)
}

/// 이 주소로 `secs`초 동안 요청을 쉼(이미 더 길게 쉬는 중이면 그대로).
pub(crate) fn cool(base: &str, secs: u64) {
    let until = std::time::Instant::now() + std::time::Duration::from_secs(secs);
    let mut map = cooldowns().lock().unwrap_or_else(|e| e.into_inner());
    let slot = map.entry(cool_key(base)).or_insert(until);
    if *slot < until {
        *slot = until;
    }
}

/// 응답 머리의 Retry-After(초). 없거나 읽지 못하면 None.
pub(crate) fn retry_after_secs(headers: &reqwest::header::HeaderMap) -> Option<u64> {
    headers.get("retry-after").and_then(|v| v.to_str().ok()).and_then(|v| v.trim().parse::<u64>().ok())
}

/// 주문 뒤 결과 조회 횟수. 시장가라도 거래소가 체결 상태를 늦게 보일 때가 있음 — 너무 일찍 '결과 모름'으로
/// 실행을 멈추지 않게 몇 초에 걸쳐 확인함(새 주문은 내지 않고 같은 client id로 조회만).
pub const LOOKUP_ATTEMPTS: usize = 6;

/// `attempt`번째 조회 뒤 기다릴 시간: 0.25·0.5·1·2·4초(합 7.75초). 테스트는 짧게.
pub fn lookup_delay(attempt: usize) -> std::time::Duration {
    if cfg!(test) {
        return std::time::Duration::from_millis(2);
    }
    std::time::Duration::from_secs_f64((0.25 * 2f64.powi(attempt as i32)).min(4.0))
}
