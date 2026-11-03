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

/// 시세 요청 사이에 두는 최소 간격 — 거래소 기준 주소별로 모든 실행이 함께 씀.
///
/// [`cool`]과 다름. 쉬기는 **한도에 걸린 뒤** 다 같이 멈췄다 다 같이 다시 쏘는 것이라,
/// 묶음처럼 여럿이 한꺼번에 시작하면 그 다음 바퀴에 또 부딪힘. 여기서는 **차례를 미리
/// 나눠 줌** — 스물한 개가 한꺼번에 와도 줄을 서서 간격만큼 떨어져 나감.
///
/// 업비트 시세 한도가 초당 10건이라 그 8할만 씀. 시험은 짧게.
pub(crate) const QUOTE_GAP: std::time::Duration = std::time::Duration::from_millis(if cfg!(test) { 20 } else { 120 });

/// 주소별 '다음 차례' 시각.
fn gates() -> &'static std::sync::Mutex<std::collections::HashMap<String, std::time::Instant>> {
    static GATES: std::sync::OnceLock<std::sync::Mutex<std::collections::HashMap<String, std::time::Instant>>> =
        std::sync::OnceLock::new();
    GATES.get_or_init(Default::default)
}

/// 이 주소로 보낼 차례가 올 때까지 기다림.
///
/// 차례는 **받는 즉시 예약**함 — 그래서 동시에 부른 쪽끼리도 서로 겹치지 않음. 기다리는
/// 동안 자물쇠를 쥐지 않음(쥐면 뒤에 선 쪽이 차례를 못 받음).
pub(crate) async fn space(base: &str, gap: std::time::Duration) {
    let wait = {
        let now = std::time::Instant::now();
        let mut map = gates().lock().unwrap_or_else(|e| e.into_inner());
        let slot = map.entry(cool_key(base)).or_insert(now);
        let turn = (*slot).max(now);
        *slot = turn + gap;
        turn - now
    };
    if !wait.is_zero() {
        tokio::time::sleep(wait).await;
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{Duration, Instant};

    #[tokio::test]
    async fn 차례를_미리_나눠_줘서_한꺼번에_와도_겹치지_않는다() {
        let gap = Duration::from_millis(30);
        let started = Instant::now();
        let mut running = Vec::new();
        for _ in 0..5 {
            running.push(tokio::spawn(async move {
                space("https://gate.test", gap).await;
                Instant::now()
            }));
        }
        let mut at = Vec::new();
        for task in running {
            at.push(task.await.expect("작업"));
        }
        at.sort();
        // 다섯이면 네 칸 — 마지막은 적어도 그만큼 뒤에 나감.
        assert!(at[4] - started >= gap * 4, "{:?}", at[4] - started);
        for pair in at.windows(2) {
            // 칸 하나에 둘이 들어가지 않음. 재는 시각이 조금 흔들릴 수 있어 여유를 둠.
            assert!(pair[1] - pair[0] >= gap - Duration::from_millis(5), "{:?}", pair[1] - pair[0]);
        }
    }

    #[tokio::test]
    async fn 주소가_다르면_서로_기다리지_않는다() {
        let gap = Duration::from_millis(50);
        let started = Instant::now();
        tokio::join!(space("https://one.test", gap), space("https://two.test", gap));
        assert!(started.elapsed() < gap, "{:?}", started.elapsed());
    }
}
