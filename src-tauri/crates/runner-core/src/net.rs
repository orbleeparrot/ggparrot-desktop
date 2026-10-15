//! 공용 HTTP 클라이언트 — 모든 실행이 한 연결 풀을 나눠 씀.
//! 요청마다 제한 시간을 따로 두지 않은 호출도 여기서 막음(멈춘 연결 하나가 실행을 붙잡지 않게).

use std::time::Duration;

/// 연결 맺기 제한.
pub const CONNECT_TIMEOUT: Duration = Duration::from_secs(5);
/// 요청 하나의 전체 제한(요청마다 더 짧게 줄 수 있음).
pub const REQUEST_TIMEOUT: Duration = Duration::from_secs(15);

pub fn user_agent() -> String {
    format!("GGParrotDesktop/{} (runner {})", crate::APP_VERSION, crate::RUNNER_VERSION)
}

/// 모든 클라이언트에 거는 보안 설정.
/// - 리디렉션을 따라가지 않음: reqwest는 다른 호스트로 넘어갈 때 Authorization·Cookie만 지우고
///   직접 단 `X-Runner-Key`·`X-MBX-APIKEY`는 그대로 넘김(https → http도 막지 않음).
///   서버·거래소 API는 리디렉션하지 않으므로 잃는 것이 없음.
/// - 배포판은 https만(개발 빌드는 로컬 서버·테스트용 가짜 서버가 http임).
pub fn harden(builder: reqwest::ClientBuilder) -> reqwest::ClientBuilder {
    builder.redirect(reqwest::redirect::Policy::none()).https_only(!cfg!(debug_assertions))
}

/// 실행·서버 보고에 쓰는 클라이언트.
pub fn http_client() -> reqwest::Client {
    harden(reqwest::Client::builder())
        .connect_timeout(CONNECT_TIMEOUT)
        .timeout(REQUEST_TIMEOUT)
        .tcp_keepalive(Duration::from_secs(30))
        .pool_idle_timeout(Duration::from_secs(60))
        .user_agent(user_agent())
        .build()
        // TLS 초기화가 실패하는 경우뿐 — 그래도 제한 시간은 지킴.
        .unwrap_or_else(|_| fallback(REQUEST_TIMEOUT))
}

/// 빌더가 실패했을 때도 제한 시간이 있는 클라이언트.
pub fn fallback(timeout: Duration) -> reqwest::Client {
    // 두 번째 빌드도 실패하면 기본 클라이언트밖에 없음 — 요청마다 거는 제한 시간(.timeout)이 그때도 지킴.
    harden(reqwest::Client::builder()).timeout(timeout).build().unwrap_or_else(|e| {
        eprintln!("http client fallback without default timeout: {e}");
        reqwest::Client::new()
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn a_stalled_server_times_out() {
        // 연결은 받지만 답하지 않는 서버
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move {
            let (_sock, _) = listener.accept().await.unwrap();
            tokio::time::sleep(Duration::from_secs(60)).await;
        });
        let client = reqwest::Client::builder().timeout(Duration::from_millis(300)).build().unwrap();
        let started = std::time::Instant::now();
        assert!(client.get(format!("http://{addr}/")).send().await.is_err());
        assert!(started.elapsed() < Duration::from_secs(5));
        assert!(user_agent().starts_with("GGParrotDesktop/"));
    }
}
