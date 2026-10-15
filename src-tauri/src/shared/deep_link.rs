//! 웹 연결(ggparrot://launch 딥링크·티켓 교환).

use super::*;

impl Shared {
    pub fn handle_deep_link(self: &S, url: &str) {
        self.bring_to_front();
        let launch = match parse_protocol_url(url) {
            Ok(l) => {
                // 링크 주소(티켓)는 남기지 않음 — 받은 사실과 서버 종류만.
                let env = match l.environment {
                    Environment::Production => "껄무새 서버",
                    Environment::Local => "개발용 로컬 서버",
                };
                self.log_app("info", "web", "웹 연결 요청을 받았어요", env);
                l
            }
            Err(_) => {
                self.notice_from(
                    "web",
                    "warn",
                    "올바르지 않은 웹 연결 요청은 무시했어요. 사이트에서 다시 시도해 주세요.",
                );
                return;
            }
        };
        {
            let mut c = self.lock();
            if c.claim_busy {
                drop(c);
                self.notice_from("web", "info", "이미 웹 매크로를 연결하고 있어요. 잠시만 기다려 주세요.");
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
                self.notice_from(
                    "web",
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
                Err(server::ClaimError::UpdateRequired(detail)) => {
                    // 서버가 이 앱의 서버 연동 레벨으로는 이 매크로를 열지 않음 — 이유와 함께 업데이트 화면으로 데려감.
                    me.emit();
                    me.navigate("settings:about");
                    me.update_check(false);
                    me.notice_from("web", "warn", update_required_message(&detail));
                }
                Err(server::ClaimError::Failed) => {
                    me.emit();
                    me.notice_from(
                        "web",
                        "warn",
                        "웹 연결 요청을 확인하지 못했어요. 사이트로 돌아가 다시 시도해 주세요.",
                    );
                }
            }
        });
    }

    pub(crate) fn apply_claim(self: &S, claimed: server::ClaimedLaunch, base: String) {
        let name = claimed.name.clone().unwrap_or_else(|| "웹에서 연결한 내 매크로".into());
        // 지원하지 않는 매크로(정기 분할매수·여러 종목 등)는 다시 시도해도 안 되므로 이유를 그대로 알림.
        // 폴더에 파일을 쓰기 전에 봄 — 열지 못할 매크로를 폴더에 남기지 않게.
        if let Err(why) = runs::check_runnable(&claimed.macro_) {
            self.notice_from("web", "warn", format!("웹에서 보낸 매크로를 열지 못했어요.\n{why}"));
            return;
        }
        // 매크로 폴더에 `.ggm.json`으로 남김(같은 매크로 파일이 있으면 그 파일과 연결) — 실패해도 웹 실행은 막지 않음
        // (실행 목록에만 둠). 폴더는 상태 잠금 밖에서 씀. 목록은 실행을 넣기 전에 다시 읽어 '실행 목록에 1개'가 바로 보이게.
        let saved = library::save_macro(&self.library_dir(), &claimed.macro_, now_ms());
        if saved.is_ok() {
            self.refresh_library_quiet();
        }
        let file = saved.as_ref().ok().map(|s| s.file_name.clone());
        let id = match self.add_run(claimed.macro_.clone(), &name, "web", file, Some(base.clone())) {
            Ok(id) => id,
            Err(why) => {
                self.notice_from("web", "warn", format!("웹에서 보낸 매크로를 열지 못했어요.\n{why}"));
                return;
            }
        };
        let edited = self.edit_runs(|c| {
            // 웹 연결은 거래소 키를 실어 오지 않음. 링크로 받은 회원 키는 사용자가 확인한 뒤에만 씀 —
            // 비어 있을 때 조용히 채우면, 남의 티켓을 담은 피싱 링크로 그 사람 계정이 이 앱에 들어올 수 있음.
            let incoming = claimed.runner_key.trim().to_string();
            let current = c.member_key.trim().to_string();
            let (mut switch_asked, mut first_key) = (false, false);
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
            let detail = c.run(&id).map(|r| format!("{} {}", exchange_label(&r.exchange_name()), r.symbol()));
            c.push_app_log("info", "web", &format!("웹 매크로를 연결했어요 — {name}"), &detail.unwrap_or_default());
            c.push_activity(
                &id,
                "info",
                "웹에서 매크로를 연결했어요",
                "모드는 모의로 준비했어요. 확인한 뒤 직접 시작하세요.",
                "웹 매크로와 껄무새 계정을 연결했어요. 실행 모드(기본 모의)를 확인한 뒤 직접 시작해 주세요.",
                false,
                "ok",
            );
            push_saved_activity(c, &id, &saved);
            Ok::<_, ()>((switch_asked, first_key))
        });
        let Ok((switch_asked, first_key)) = edited else { return };
        self.navigate(&format!("run:{id}"));
        self.bring_to_front();
        if first_key {
            self.notice_from("web", 
                "info",
                "웹 연결로 껄무새 계정의 회원 키를 받았어요.\n설정 › 껄무새 계정에서 내 계정이 맞는지 확인하고 [이 키 쓰기]를 누르세요. 그 전에는 시작할 수 없어요.",
            );
            self.navigate("settings");
        }
        if switch_asked {
            self.notice_from("web", 
                "warn",
                "웹 연결에서 지금과 다른 껄무새 계정의 회원 키를 받았어요.\n설정 › 껄무새 계정에서 그 키로 바꿀지 정하세요. 바꾸기 전까지는 지금 회원 키를 써요.",
            );
            self.navigate("settings");
        }
    }
}

/// 웹 매크로를 매크로 폴더에 남긴 결과를 실행의 활동 기록과 앱 기록에 적음.
fn push_saved_activity(c: &mut Core, id: &str, saved: &Result<library::SavedMacro, String>) {
    match saved {
        Ok(s) if s.reused => {
            c.push_app_log("info", "library", &format!("웹 매크로를 이미 있는 파일과 연결했어요 — {}", s.file_name), "")
        }
        Ok(s) => {
            c.push_app_log("info", "library", &format!("웹 매크로를 매크로 폴더에 저장했어요 — {}", s.file_name), "")
        }
        Err(why) => c.push_app_log("warn", "library", "웹 매크로를 매크로 폴더에 저장하지 못했어요", why),
    };
    match saved {
        Ok(s) if s.reused => c.push_activity(
            id,
            "info",
            &format!("이미 있는 파일과 연결했어요 — {}", s.file_name),
            "매크로 폴더에 같은 매크로 파일이 있어 새로 저장하지 않았어요.",
            &format!("매크로 폴더의 같은 매크로 파일과 연결했어요: {}", s.file_name),
            false,
            "muted",
        ),
        Ok(s) => c.push_activity(
            id,
            "info",
            &format!("매크로 폴더에 저장했어요 — {}", s.file_name),
            "실행 목록에서 빼도 파일은 매크로 폴더에 남아요.",
            &format!("웹 매크로를 매크로 폴더에 저장했어요: {}", s.file_name),
            false,
            "muted",
        ),
        Err(why) => c.push_activity(
            id,
            "info",
            "매크로 폴더에 저장하지 못했어요",
            "실행은 그대로 할 수 있어요. 이 매크로는 실행 목록에만 있어요 — 폴더에 두려면 웹에서 파일로 내려받으세요.",
            &format!("웹 매크로를 매크로 폴더에 저장하지 못했어요: {why}"),
            false,
            "warn",
        ),
    }
}

/// 웹 연결이 426(업데이트 필요)으로 막혔을 때의 안내. 서버 안내가 있으면 그대로 앞에 둠.
fn update_required_message(detail: &str) -> String {
    let tail = "설정 › 정보에서 껄무새 앱을 최신 버전으로 업데이트한 뒤 사이트에서 다시 시도하세요.";
    if detail.trim().is_empty() {
        format!("이 웹 매크로를 열려면 새 버전의 껄무새 앱이 필요해요.\n{tail}")
    } else {
        format!("{}\n{tail}", detail.trim())
    }
}

#[cfg(test)]
mod tests {
    use crate::state::{Core, Run};
    use runner_core::library;

    #[test]
    fn a_saved_web_macro_stays_web_and_counts_as_in_use() {
        let dir = std::env::temp_dir().join(format!("ggp-web-save-{}", uuid::Uuid::new_v4().simple()));
        let m = serde_json::json!({"symbol": "KRW-BTC", "exchange": "upbit", "rule_type": "F"});
        let saved = library::save_macro(&dir, &m, 0);
        let file = saved.as_ref().unwrap().file_name.clone();
        let mut c = Core::new("https://example.test".into(), true, dir.clone());
        c.library = library::scan(&dir).unwrap().files;
        let _ = std::fs::remove_dir_all(&dir);
        let mut run = Run::new("w".into(), m, "내 매크로".into(), "web", "https://example.test".into());
        run.library_file = Some(file.clone());
        c.runs.push(run);
        super::push_saved_activity(&mut c, "w", &saved);
        let v = serde_json::to_value(c.render("0.19.2")).unwrap();
        assert_eq!(v["runs"][0]["macro"]["source"], "web", "웹 연결은 그대로 — 파일과 이어져도 출처는 웹");
        assert_eq!(v["runs"][0]["macro"]["signature"], "web");
        assert_eq!(v["library"]["files"][0]["inUse"], 1, "매크로 폴더 목록에 '실행 목록에 1개'");
        let a = &c.run("w").unwrap().activity;
        assert_eq!(a.last().unwrap().title, format!("매크로 폴더에 저장했어요 — {file}"));

        let reused = Ok(library::SavedMacro { file_name: file.clone(), reused: true });
        super::push_saved_activity(&mut c, "w", &reused);
        assert!(c.run("w").unwrap().activity.last().unwrap().title.starts_with("이미 있는 파일과 연결했어요"));
        super::push_saved_activity(&mut c, "w", &Err("디스크가 가득 찼어요".into()));
        let last = c.run("w").unwrap().activity.last().unwrap().clone();
        assert_eq!((last.title.as_str(), last.tone.as_str()), ("매크로 폴더에 저장하지 못했어요", "warn"));
        assert!(last.raw.contains("디스크가 가득 찼어요"));
        // 매크로 폴더 저장 결과는 앱 기록에도 남음(저장·연결·실패).
        let logs: Vec<(&str, &str, String)> = c.app_log.iter().map(|e| (e.level, e.source, e.title.clone())).collect();
        assert_eq!(logs.len(), 3, "{logs:?}");
        assert_eq!(logs[0], ("info", "library", format!("웹 매크로를 매크로 폴더에 저장했어요 — {file}")));
        assert!(logs[1].2.starts_with("웹 매크로를 이미 있는 파일과 연결했어요"));
        assert_eq!((logs[2].0, logs[2].1), ("warn", "library"));
        assert_eq!(c.app_log.back().unwrap().detail, "디스크가 가득 찼어요");
    }

    #[test]
    fn an_update_notice_keeps_the_server_reason_and_says_what_to_do() {
        let m = super::update_required_message("바이낸스 빠른 실행은 실행기 v19 이상이 필요해요.");
        assert!(m.starts_with("바이낸스 빠른 실행은") && m.contains("업데이트"));
        assert!(super::update_required_message(" ").contains("새 버전의 껄무새 앱"));
    }
}
