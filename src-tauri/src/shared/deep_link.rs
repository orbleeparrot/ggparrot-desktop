//! 웹 연결(ggparrot://launch 딥링크·티켓 교환).

use super::*;

impl Shared {
    pub fn handle_deep_link(self: &S, url: &str) {
        self.bring_to_front();
        let launch = match parse_protocol_url(url) {
            Ok(l) => l,
            Err(_) => {
                self.notice("warn", "올바르지 않은 웹 연결 요청은 무시했어요. 사이트에서 다시 시도해 주세요.");
                return;
            }
        };
        {
            let mut c = self.lock();
            if c.claim_busy {
                drop(c);
                self.notice("info", "이미 웹 매크로를 연결하고 있어요. 잠시만 기다려 주세요.");
                return;
            }
            c.claim_busy = true;
        }
        self.emit();
        let base = match launch.environment {
            // 개발용 로컬 서버 연결은 개발 빌드에서만 받음(배포판에서는 평문 HTTP로 회원 키가 새지 않게).
            Environment::Local if cfg!(debug_assertions) => LOCAL_SERVER_BASE.to_string(),
            Environment::Local => {
                self.lock().claim_busy = false;
                self.emit();
                self.notice(
                    "warn",
                    "개발용 웹 연결 요청은 이 버전에서 받지 않아요. 껄무새 사이트에서 다시 시도해 주세요.",
                );
                return;
            }
            Environment::Production => server_base(),
        };
        let me = self.clone();
        tauri::async_runtime::spawn(async move {
            let claimed = server::claim_ticket(&me.http, &base, &launch.ticket).await;
            me.lock().claim_busy = false;
            match claimed {
                Ok(claimed) => me.apply_claim(claimed, base),
                Err(()) => {
                    me.emit();
                    me.notice("warn", "웹 연결 요청을 확인하지 못했어요. 사이트로 돌아가 다시 시도해 주세요.");
                }
            }
        });
    }

    pub(crate) fn apply_claim(self: &S, claimed: server::ClaimedLaunch, base: String) {
        let mut switch_asked = false;
        let mut first_key = false;
        let name = claimed.name.clone().unwrap_or_else(|| "웹에서 연결한 내 매크로".into());
        let id = match self.add_run(claimed.macro_.clone(), &name, "web", None, Some(base.clone())) {
            Ok(id) => id,
            // 지원하지 않는 매크로(정기 분할매수·여러 종목 등)는 다시 시도해도 안 되므로 이유를 그대로 알림.
            Err(why) => {
                self.notice("warn", format!("웹에서 보낸 매크로를 열지 못했어요.\n{why}"));
                return;
            }
        };
        {
            let mut c = self.lock();
            // 웹 연결은 거래소 키를 실어 오지 않음. 링크로 받은 회원 키는 사용자가 확인한 뒤에만 씀 —
            // 비어 있을 때 조용히 채우면, 남의 티켓을 담은 피싱 링크로 그 사람 계정이 이 앱에 들어올 수 있음.
            let incoming = claimed.runner_key.trim().to_string();
            let current = c.member_key.trim().to_string();
            if current == incoming {
                c.pending_member_key = None;
            } else {
                switch_asked = !current.is_empty();
                first_key = current.is_empty();
                c.pending_member_key = Some(incoming);
            }
            let reporter = claimed
                .launch_id
                .and_then(|lid| {
                    SetupProgressReporter::new(&base, claimed.runner_key.trim(), lid, self.http.clone()).ok()
                })
                .map(Arc::new);
            if let Some(r) = c.run_mut(&id) {
                r.user_macro_id = claimed.user_macro_id;
                r.web_linked = true;
                r.reporter = reporter;
            }
            c.push_activity(
                &id,
                "info",
                "웹에서 매크로를 연결했어요",
                "모드는 모의로 준비했어요. 확인한 뒤 직접 시작하세요.",
                "웹 매크로와 껄무새 계정을 연결했어요. 실행 모드(기본 모의)를 확인한 뒤 직접 시작해 주세요.",
                false,
                "ok",
            );
            self.save_runs(&c);
        }
        self.emit();
        self.navigate(&format!("run:{id}"));
        self.bring_to_front();
        if first_key {
            self.notice(
                "info",
                "웹 연결로 껄무새 계정의 회원 키를 받았어요.\n설정 › 껄무새 계정에서 내 계정이 맞는지 확인하고 [이 키 쓰기]를 누르세요. 그 전에는 시작할 수 없어요.",
            );
            self.navigate("settings");
        }
        if switch_asked {
            self.notice(
                "warn",
                "웹 연결에서 지금과 다른 껄무새 계정의 회원 키를 받았어요.\n설정 › 껄무새 계정에서 그 키로 바꿀지 정하세요. 바꾸기 전까지는 지금 회원 키를 써요.",
            );
            self.navigate("settings");
        }
    }
}
