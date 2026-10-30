//! 앱 기록 — 알림 없이 남기는 앱 전체의 일, 앱 기록 읽기·복사.

use super::*;
use crate::state::{ACTIVITY_COPY_CHARS, AppLogEntry, app_log_copy_text};

impl Shared {
    /// 앱 기록에 한 줄 남기고 바로 화면으로 보냄(알림은 띄우지 않음). Core 잠금을 든 채 부르지 않음 —
    /// 잠금 안에서는 `Core::push_app_log`로 넣고 다음 emit이 보내게 함.
    pub fn log_app(&self, level: &str, source: &'static str, title: &str, detail: &str) {
        self.lock().push_app_log(level, source, title, detail);
        self.emit_app_log_only();
    }

    /// 앱 기록만 보냄(상태 스냅샷은 그대로).
    pub(crate) fn emit_app_log_only(&self) {
        let _order = lock_or_recover(&self.emit_lock);
        let logs = std::mem::take(&mut self.lock().pending_app_log);
        for entry in logs {
            let _ = self.app.emit(EV_APP_LOG, entry);
        }
    }

    /// 앱 기록 전체(오래된 것 → 최근).
    pub fn app_log(&self) -> Vec<AppLogEntry> {
        self.lock().app_log_list()
    }

    /// [앱 기록 복사] — 머리말(버전·OS) + 줄마다 날짜·시각. 길면 최근 줄만.
    pub fn copy_app_log(&self) -> Result<(), String> {
        let text = {
            let c = self.lock();
            let head = format!(
                "[껄무새 앱 기록] v{} · {} · {}",
                self.version,
                os_summary(),
                crate::state::copied_at(chrono::Local::now())
            );
            app_log_copy_text(&head, &c.app_log, ACTIVITY_COPY_CHARS)
        };
        self.copy_text(&text, "앱 기록을 복사하지 못했어요.")
    }

    /// 켤 때 한 줄 — 버전·OS·서버 연동 레벨(기본 서버가 아니면 그 주소도).
    pub(crate) fn log_app_start(&self) {
        let mut c = self.lock();
        let mut detail = format!("{} · 서버 연동 레벨 {}", os_summary(), runner_core::SERVER_LINK_LEVEL);
        if !crate::state::is_default_server(&c.server_base) {
            detail.push_str(&format!(" · 서버 {} (기본 서버 아님)", c.server_base));
        }
        c.push_app_log("info", "app", &format!("앱을 시작했어요 — v{}", self.version), &detail);
    }
}
