//! 창 닫기·종료·포커스·지난 기록 이어 보내기.

use super::*;

impl Shared {
    /// 닫아도 되면 참. 실행 중이면 거절하고 안내함.
    pub fn request_close(self: &S) -> bool {
        let running = self.lock().busy_count();
        if running > 0 {
            self.bring_to_front();
            self.notice(
                "warn",
                format!("실행 중이거나 시작하는 매크로가 {running}개 있어요.\n각 매크로를 먼저 종료해 주세요. 연결 확인 · 시작하는 중이면 잠시 기다려 주세요."),
            );
            return false;
        }
        true
    }

    /// 웹 가이드가 '진행 중'에 멈춰 있지 않게 닫힘을 알림(최대 1.5초).
    pub async fn shutdown(self: &S) {
        let reporters = {
            let mut c = self.lock();
            c.closed = true;
            c.wizard = None;
            let out: Vec<_> = c.runs.iter_mut().filter_map(Run::take_closing_report).collect();
            self.save_runs(&c);
            out
        };
        for (rep, payload) in reporters {
            finish_report(rep, payload).await;
        }
        self.outbox.close();
    }

    pub fn note_focus(self: &S, focused: bool) {
        if !focused {
            self.lock().app_away = true;
            return;
        }
        let was_away = {
            let mut c = self.lock();
            std::mem::replace(&mut c.app_away, false)
        };
        if was_away {
            // 다른 앱에서 파일을 넣었을 수 있음 — 매크로 폴더를 다시 읽음(메인 스레드를 막지 않게 따로).
            let me = self.clone();
            tauri::async_runtime::spawn_blocking(move || {
                let _ = me.refresh_library();
            });
            self.wizard_on_return();
        }
    }

    /// 지난 실행에서 서버가 저장 확인을 못 한 기록을 이어서 보냄(회원 키가 있을 때만).
    pub fn resume_outbox(self: &S) {
        let (key, base) = {
            let c = self.lock();
            (c.member_key.trim().to_string(), c.server_base.clone())
        };
        if key.is_empty() {
            return;
        }
        ServerClient::new(&key, &base, self.outbox.clone(), self.http.clone()).resume_pending_events();
    }
}

/// 웹 가이드 보고의 마지막 상태를 보내고 닫음(최대 1.5초 기다림). 상태가 없으면 닫기만 함.
pub(crate) async fn finish_report(rep: Arc<SetupProgressReporter>, payload: Option<Value>) {
    match payload {
        Some(p) => rep.finish(p, Duration::from_millis(1500)).await,
        None => rep.close(),
    }
}
