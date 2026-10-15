//! 화면으로 보내기 — 상태 스냅샷(모아 보내기·매크로 폴더 목록은 바뀐 판만)·활동·알림·화면 이동.

use super::*;

/// 봇 상태 갱신을 모으는 시간. 틱(5초)보다 훨씬 짧아 화면이 늦어 보이지 않음.
const EMIT_COALESCE: std::time::Duration = std::time::Duration::from_millis(250);

impl Shared {
    // --- 출력 -------------------------------------------------------------------
    pub fn emit(&self) {
        use std::sync::atomic::Ordering;
        let _order = lock_or_recover(&self.emit_lock);
        let (dto, progress, items) = {
            let mut c = self.lock();
            let items = std::mem::take(&mut c.pending_activity);
            let library = self.sent_library_rev.swap(c.library_rev, Ordering::Relaxed) != c.library_rev;
            (c.render_with(&self.version, library), c.progress_payloads(), items)
        };
        for item in items {
            let _ = self.app.emit(EV_ACTIVITY, item);
        }
        let _ = self.app.emit(EV_STATE, serde_json::to_value(dto).unwrap_or_default());
        for (rep, payload) in progress {
            rep.report(payload);
        }
        self.refresh_tray();
    }

    /// 봇의 상태 갱신처럼 잦은 변화는 잠깐 모아 한 번에 보냄 — 실행이 여럿이면 틱마다 상태 전체를
    /// 몇 번씩 그리고 보내지 않게. 사용자 동작(시작·종료·설정)은 [`Shared::emit`]으로 바로 보냄.
    pub fn emit_soon(self: &S) {
        use std::sync::atomic::Ordering;
        if self.emit_queued.swap(true, Ordering::AcqRel) {
            return;
        }
        let me = self.clone();
        tauri::async_runtime::spawn(async move {
            tokio::time::sleep(EMIT_COALESCE).await;
            // 내리고 나서 보냄 — 보내는 사이에 온 갱신은 다음 묶음으로 감.
            me.emit_queued.store(false, Ordering::Release);
            me.emit();
        });
    }

    /// 처음 화면을 그릴 때(get_state) — 매크로 폴더 목록까지 늘 실음.
    pub fn state_value(&self) -> Value {
        serde_json::to_value(self.lock().render(&self.version)).unwrap_or(Value::Null)
    }

    pub fn notice(&self, level: &str, message: impl Into<String>) {
        let _ =
            self.app.emit(EV_NOTICE, Notice { level: level.into(), title: APP_TITLE.into(), message: message.into() });
    }

    pub fn navigate(&self, to: &str) {
        let _ = self.app.emit(EV_NAVIGATE, json!({ "to": to }));
    }

    /// 활동만 보냄(상태 스냅샷은 그대로) — 로그 줄마다 전체 상태를 보내지 않게.
    pub(crate) fn emit_activity_only(&self) {
        // emit과 같은 순서 잠금 — 활동 줄이 두 경로로 나가며 순서가 뒤바뀌지 않게.
        let _order = lock_or_recover(&self.emit_lock);
        let items = std::mem::take(&mut self.lock().pending_activity);
        for item in items {
            let _ = self.app.emit(EV_ACTIVITY, item);
        }
    }
}
