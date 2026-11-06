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

/// 주소별 '다음 차례' 시각. tokio 시각을 씀 — 시험이 멈춘 시계로 기다림을 정확히 잴 수 있게.
fn gates() -> &'static std::sync::Mutex<std::collections::HashMap<String, tokio::time::Instant>> {
    static GATES: std::sync::OnceLock<std::sync::Mutex<std::collections::HashMap<String, tokio::time::Instant>>> =
        std::sync::OnceLock::new();
    GATES.get_or_init(Default::default)
}

/// 이 주소의 다음 차례를 받아 예약함 — `now`와 지난 예약 중 늦은 쪽이 내 차례, 다음 차례는 그 `gap` 뒤.
///
/// 예약끼리는 늘 `gap`만큼 떨어짐. 실제로 나가는 시각은 OS가 깨워 주는 만큼(Windows는 보통 0~16ms) 뒤로 밀릴
/// 뿐 앞당겨지지 않음 — 그래서 어느 1초에도 예약 수(+1)를 넘지 않음(120ms면 9건, 업비트 시세 한도 10건).
fn reserve(base: &str, gap: std::time::Duration, now: tokio::time::Instant) -> tokio::time::Instant {
    let mut map = gates().lock().unwrap_or_else(|e| e.into_inner());
    let slot = map.entry(cool_key(base)).or_insert(now);
    let turn = (*slot).max(now);
    *slot = turn + gap;
    turn
}

/// 이 주소로 시세 차례를 예약한 적이 있는지 — 시험이 '이 요청은 줄에 서지 않음'을 시각 대신 확인할 때 씀.
#[cfg(test)]
pub(crate) fn reserved(base: &str) -> bool {
    gates().lock().unwrap_or_else(|e| e.into_inner()).contains_key(&cool_key(base))
}

/// 이 주소로 보낼 차례가 올 때까지 기다림.
///
/// 차례는 **받는 즉시 예약**함 — 그래서 동시에 부른 쪽끼리도 서로 겹치지 않음. 기다리는
/// 동안 자물쇠를 쥐지 않음(쥐면 뒤에 선 쪽이 차례를 못 받음).
pub(crate) async fn space(base: &str, gap: std::time::Duration) {
    let now = tokio::time::Instant::now();
    let turn = reserve(base, gap, now);
    if turn > now {
        tokio::time::sleep_until(turn).await;
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
    use std::time::Duration;
    use tokio::time::Instant;

    // 실제로 깨어난 시각은 재지 않음 — OS가 늦게 깨우는 만큼 흔들려(Windows CI에서 30ms 간격이 21.8ms로 잼)
    // 보장하지 않는 것을 확인하게 됨. 보장하는 것(예약 간격)은 시각을 넘겨 정확히 보고, 기다림은 멈춘 시계로 봄.

    #[test]
    fn 한꺼번에_와도_차례는_간격만큼_떨어져_예약된다() {
        let gap = Duration::from_millis(120);
        let now = Instant::now();
        let turns: Vec<_> = (0..5).map(|_| reserve("https://gate.test", gap, now)).collect();
        for (i, turn) in turns.iter().enumerate() {
            assert_eq!(*turn, now + gap * i as u32, "{i}번째");
        }
    }

    #[test]
    fn 줄이_비면_바로_차례() {
        let gap = Duration::from_millis(120);
        let now = Instant::now();
        assert_eq!(reserve("https://idle.test", gap, now), now);
        // 다음 차례(now+gap)보다 늦게 오면 기다리지 않음 — 지난 예약을 몰아 쓰지도 않음.
        let later = now + gap * 3;
        assert_eq!(reserve("https://idle.test", gap, later), later);
        assert_eq!(reserve("https://idle.test", gap, later), later + gap);
    }

    #[test]
    fn 주소가_다르면_서로_기다리지_않는다() {
        let gap = Duration::from_millis(120);
        let now = Instant::now();
        assert_eq!(reserve("https://one.test", gap, now), now);
        assert_eq!(reserve("https://two.test", gap, now), now);
    }

    #[tokio::test(start_paused = true)]
    async fn 기다리는_쪽은_제_차례에_나간다() {
        let gap = Duration::from_millis(30);
        let started = Instant::now();
        let running: Vec<_> = (0..5)
            .map(|_| {
                tokio::spawn(async move {
                    space("https://wait.test", gap).await;
                    Instant::now()
                })
            })
            .collect();
        let mut at = Vec::new();
        for task in running {
            at.push(task.await.expect("작업") - started);
        }
        at.sort();
        assert_eq!(at, (0..5).map(|i| gap * i).collect::<Vec<_>>());
    }
}
