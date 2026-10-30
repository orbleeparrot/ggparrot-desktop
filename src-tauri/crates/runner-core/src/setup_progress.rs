//! 웹 '매크로 만들기 가이드'에 지금 단계만 알림.
//!
//! 웹 [실행기 열기]로 들어온 앱만 보냄(그 연결의 launch_id와 회원 키가 있을 때).
//! 보내는 것은 단계 이름·바이낸스 사이트(env)·키 칸이 찼는지(참/거짓)·검사 세 줄의 상태뿐임 —
//! 거래소 키·시크릿·IP 주소·오류 문장은 어떤 경로로도 실리지 않음.
//! 보내기 실패는 조용히 버림. 앱 동작은 이 보고에 기대지 않음.
use once_cell::sync::Lazy;
use serde_json::{Value, json};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tokio::sync::{Notify, watch};

use crate::connection::Check;

pub const STAGES: [&str; 12] = [
    "ready",
    "closed",
    "ask",
    "ip",
    "perm",
    "keys",
    "existing",
    "manual",
    "checking",
    "result_ok",
    "result_fail",
    "started",
];
pub const PATHS: [&str; 3] = ["", "new", "existing"];
pub const PROBLEMS: [&str; 7] = ["", "ip", "auth", "order", "order_size", "network", "other"];
pub const EXCHANGES: [&str; 3] = ["binance", "upbit", "bithumb"];
/// 바이낸스 도우미가 안내하는 사이트(`connection::binance_env`). 국내는 "" — 보내지 않음.
pub const ENVS: [&str; 4] = ["", "testnet", "demo", "live"];
pub const PROGRESS_PATH: &str = "/api/runner/launch-tickets/{launch_id}/progress";
/// 웹 연결 하나에 대해 보고하는 최대 시간. 그 뒤로는 가이드 화면도 이미 닫혔다고 봄.
pub const REPORT_WINDOW_SECONDS: f64 = 2.0 * 60.0 * 60.0;
const RETRY_DELAY: Duration = Duration::from_secs(3);
const SEND_TIMEOUT: Duration = Duration::from_secs(5);

/// 서버가 받는 모양 그대로 만듦. 모르는 값은 보내지 않고 Err — 호출 쪽 버그를 숨기지 않음.
/// `env`는 바이낸스만(국내에 실으면 서버가 422로 거절함). 빈 값이면 키 자체를 싣지 않음 —
/// env를 모르는 서버(extra=forbid)에도 국내 보고가 그대로 통함.
/// `slots`는 (access, secret) 칸이 찼는지, `checks`는 (auth, ip, order) — 없으면 모두 "wait".
#[allow(clippy::too_many_arguments)] // 서버 요청 칸 하나하나가 인자임 — 묶으면 호출마다 구조체만 늘어남.
pub fn progress_payload(
    stage: &str,
    exchange: &str,
    env: &str,
    path: &str,
    slots: (bool, bool),
    checks: Option<(Check, Check, Check)>,
    problem: &str,
    connected: bool,
) -> Result<Value, String> {
    if !STAGES.contains(&stage)
        || !EXCHANGES.contains(&exchange)
        || !PATHS.contains(&path)
        || !PROBLEMS.contains(&problem)
        || !ENVS.contains(&env)
    {
        return Err("unknown progress field".into());
    }
    if !env.is_empty() && exchange != "binance" {
        return Err("env is binance-only".into());
    }
    let (auth, ip, order) = checks.unwrap_or((Check::Wait, Check::Wait, Check::Wait));
    let mut payload = json!({
        "stage": stage,
        "exchange": exchange,
        "path": path,
        "slots": {"access": slots.0, "secret": slots.1},
        "checks": {"auth": auth.as_str(), "ip": ip.as_str(), "order": order.as_str()},
        "problem": problem,
        "connected": connected,
    });
    if !env.is_empty() {
        payload["env"] = json!(env);
    }
    Ok(payload)
}

/// 단조 시계(초).
pub type ProgressClock = Arc<dyn Fn() -> f64 + Send + Sync>;

static PROCESS_START: Lazy<Instant> = Lazy::new(Instant::now);

fn monotonic_clock() -> ProgressClock {
    Arc::new(|| PROCESS_START.elapsed().as_secs_f64())
}

#[derive(Default)]
struct State {
    pending: Option<Value>,
    last_sent: Option<Value>,
    closed: bool,
}

struct Inner {
    url: String,
    runner_key: String,
    http: reqwest::Client,
    clock: ProgressClock,
    deadline: f64,
    state: Mutex<State>,
    wake: Notify,
    done: watch::Sender<bool>,
}

impl Inner {
    fn lock(&self) -> std::sync::MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(|e| e.into_inner())
    }

    async fn send(&self, payload: &Value) -> bool {
        // 네트워크·프록시 오류 문구에는 요청 정보가 섞일 수 있음 — 아무것도 남기지 않음.
        let response = self
            .http
            .post(&self.url)
            .header("X-Runner-Key", &self.runner_key)
            .json(payload)
            .timeout(SEND_TIMEOUT)
            .send()
            .await;
        matches!(response, Ok(r) if r.status().is_success())
    }

    async fn run(self: Arc<Self>) {
        loop {
            self.wake.notified().await;
            let (payload, closed) = {
                let mut state = self.lock();
                (state.pending.take(), state.closed)
            };
            if let Some(payload) = payload {
                // 한 번 실패로 웹 가이드가 이전 단계에 멈춰 보이지 않게 두 번까지 다시 보냄(더 새 단계가 오거나 닫으면 그만).
                let mut sent = self.send(&payload).await;
                for _ in 0..2 {
                    // 닫는 중(창을 닫음)이면 기다리지 않음 — 닫기가 재시도만큼 늦어지지 않게.
                    if sent || self.lock().pending.is_some() || self.lock().closed {
                        break;
                    }
                    tokio::time::sleep(RETRY_DELAY).await;
                    sent = self.send(&payload).await;
                }
                if sent {
                    self.lock().last_sent = Some(payload);
                }
            }
            if closed && self.lock().pending.is_none() {
                let _ = self.done.send(true);
                return;
            }
        }
    }
}

/// 가장 최근 상태 하나만 보냄. 같은 상태는 다시 보내지 않고, 밀린 상태는 최신 것으로 덮음.
pub struct SetupProgressReporter {
    inner: Arc<Inner>,
    done: watch::Receiver<bool>,
}

impl SetupProgressReporter {
    pub fn new(base: &str, runner_key: &str, launch_id: i64, http: reqwest::Client) -> Result<Self, String> {
        Self::with_clock(base, runner_key, launch_id, http, monotonic_clock())
    }

    /// 시계를 주입하는 판(테스트는 가짜 시계를 넣음).
    pub fn with_clock(
        base: &str,
        runner_key: &str,
        launch_id: i64,
        http: reqwest::Client,
        clock: ProgressClock,
    ) -> Result<Self, String> {
        if launch_id <= 0 {
            return Err("launch_id must be a positive integer".into());
        }
        let url =
            format!("{}{}", base.trim_end_matches('/'), PROGRESS_PATH.replace("{launch_id}", &launch_id.to_string()));
        let (done_tx, done_rx) = watch::channel(false);
        let deadline = clock() + REPORT_WINDOW_SECONDS;
        let inner = Arc::new(Inner {
            url,
            runner_key: runner_key.to_string(),
            http,
            clock,
            deadline,
            state: Mutex::new(State::default()),
            wake: Notify::new(),
            done: done_tx,
        });
        let worker = inner.clone();
        match tokio::runtime::Handle::try_current() {
            Ok(handle) => {
                handle.spawn(worker.run());
            }
            Err(_) => {
                // 런타임 밖에서 만들었으면 보고 전용 작은 런타임을 따로 돌림.
                std::thread::Builder::new()
                    .name("ggparrot-setup-progress".into())
                    .spawn(move || {
                        if let Ok(rt) = tokio::runtime::Builder::new_current_thread().enable_all().build() {
                            rt.block_on(worker.run());
                        }
                    })
                    .map_err(|_| "setup progress reporter could not start".to_string())?;
            }
        }
        Ok(SetupProgressReporter { inner, done: done_rx })
    }

    pub fn url(&self) -> &str {
        &self.inner.url
    }

    pub fn report(&self, payload: Value) {
        {
            let mut state = self.inner.lock();
            if state.closed || (self.inner.clock)() > self.inner.deadline {
                return;
            }
            if state.last_sent.as_ref() == Some(&payload) || state.pending.as_ref() == Some(&payload) {
                return;
            }
            state.pending = Some(payload);
        }
        self.inner.wake.notify_one();
    }

    /// 마지막 상태(창 닫힘)를 보내고 닫음. 창이 닫히기 전 잠깐(wait)만 기다림 — 못 보내도 닫힘.
    pub async fn finish(&self, payload: Value, wait: Duration) {
        {
            let mut state = self.inner.lock();
            if !state.closed && (self.inner.clock)() <= self.inner.deadline {
                state.pending = Some(payload);
            }
            state.closed = true;
        }
        self.inner.wake.notify_one();
        let mut done = self.done.clone();
        let _ = tokio::time::timeout(wait, async {
            while !*done.borrow_and_update() {
                if done.changed().await.is_err() {
                    break;
                }
            }
        })
        .await;
    }

    pub fn close(&self) {
        self.inner.lock().closed = true;
        self.inner.wake.notify_one();
    }

    /// 보내는 작업이 끝났는지.
    #[cfg(test)]
    pub fn is_finished(&self) -> bool {
        *self.done.borrow()
    }
}

impl Drop for SetupProgressReporter {
    fn drop(&mut self) {
        self.close();
    }
}

#[cfg(test)]
mod tests;
