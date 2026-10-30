//! 로컬 저장 — runs.json·prefs.json 읽고 쓰기(키는 들어가지 않음).

use super::*;

impl Shared {
    pub(crate) fn runs_path(&self) -> PathBuf {
        self.data_dir.join("runs.json")
    }
    pub(crate) fn prefs_path(&self) -> PathBuf {
        self.data_dir.join("prefs.json")
    }

    /// 임시 파일에 쓴 뒤 이름을 바꿔 덮음(쓰다 꺼져도 반쪽 파일이 남지 않게). 성공하면 참.
    pub(crate) fn write_json(path: &Path, value: &Value) -> bool {
        if let Some(dir) = path.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        let tmp = path.with_extension("json.tmp");
        std::fs::write(&tmp, serde_json::to_vec_pretty(value).unwrap_or_default()).is_ok()
            && std::fs::rename(&tmp, path).is_ok()
    }

    /// 저장에 실패하면 한 번만 알림(실패가 이어져도 같은 알림을 쏟아내지 않게). Core 잠금을 푼 뒤 부름.
    pub(crate) fn warn_save_failed(&self, ok: bool) {
        if ok {
            self.save_warned.store(false, std::sync::atomic::Ordering::Relaxed);
        } else if !self.save_warned.swap(true, std::sync::atomic::Ordering::Relaxed) {
            self.notice_from("storage", 
                "warn",
                "실행 목록 · 설정을 저장하지 못했어요. 디스크 공간과 폴더 권한을 확인하세요. 이대로 앱을 끄면 최근 변경이 사라질 수 있어요.",
            );
        }
    }

    /// runs.json을 씀. 성공하면 참 — 실패 알림은 잠금을 푼 뒤 [`Shared::warn_save_failed`]로.
    #[must_use = "잠금을 푼 뒤 warn_save_failed로 넘김"]
    pub(crate) fn save_runs(&self, c: &Core) -> bool {
        let list: Vec<SavedRun> = c.runs.iter().map(Run::saved).collect();
        Self::write_json(&self.runs_path(), &serde_json::to_value(list).unwrap_or_default())
    }

    /// prefs.json을 씀. 성공하면 참 — 실패 알림은 잠금을 푼 뒤 [`Shared::warn_save_failed`]로.
    #[must_use = "잠금을 푼 뒤 warn_save_failed로 넘김"]
    pub(crate) fn save_prefs(&self, c: &Core) -> bool {
        Self::write_json(
            &self.prefs_path(),
            &json!({
                "show_dev_log": c.show_dev_log,
                "close_to_tray": c.close_to_tray,
                // 키 이름은 옛 '매수 체결 알림' 그대로 — 켜 둔 · 끈 값을 이어받음.
                "notify_fills": c.notify,
                "last_version": self.version,
            }),
        )
    }

    pub(crate) fn load_saved(&self) {
        let mut c = self.lock();
        if let Ok(text) = std::fs::read_to_string(self.runs_path()) {
            let base = c.server_base.clone();
            let (runs, lost) = parse_saved_runs(&text);
            c.runs = runs.into_iter().map(|s| Run::from_saved(s, &base)).collect();
            if lost > 0 {
                // 읽지 못한 목록을 빈 목록으로 덮어쓰지 않게 원본을 남겨 둠.
                let backup = self.runs_path().with_extension(format!("json.corrupt-{}", now_ms()));
                let _ = std::fs::copy(self.runs_path(), &backup);
                let me = self.app.clone();
                let whole = serde_json::from_str::<Vec<Value>>(&text).is_err();
                let msg = if whole {
                    format!(
                        "저장된 실행 목록을 읽지 못해 빈 목록으로 열었어요. 원본은 {}에 남겨 두었어요.",
                        backup.display()
                    )
                } else {
                    format!(
                        "저장된 실행 목록 중 {lost}개를 읽지 못해 빼고 열었어요. 원본은 {}에 남겨 두었어요.",
                        backup.display()
                    )
                };
                tauri::async_runtime::spawn(async move {
                    // 화면이 준비된 뒤 알림.
                    tokio::time::sleep(std::time::Duration::from_secs(2)).await;
                    if let Some(s) = me.try_state::<S>() {
                        s.notice_from("storage", "warn", msg);
                    }
                });
            }
        }
        if let Ok(text) = std::fs::read_to_string(self.prefs_path())
            && let Ok(v) = serde_json::from_str::<Value>(&text)
        {
            c.show_dev_log = v.get("show_dev_log").and_then(Value::as_bool).unwrap_or(false);
            c.close_to_tray = v.get("close_to_tray").and_then(Value::as_bool).unwrap_or(true);
            c.notify = v.get("notify_fills").and_then(Value::as_bool).unwrap_or(true);
            let last = v.get("last_version").and_then(Value::as_str).unwrap_or("");
            if is_newer(&self.version, last) {
                c.update.updated_from = last.to_string();
                c.push_app_log("info", "update", &format!("v{last}에서 v{}으로 업데이트했어요", self.version), "");
            }
        }
        // 이번 버전을 기록해 둠(다음 실행에서 업데이트 여부를 앎).
        let saved = self.save_prefs(&c);
        drop(c);
        self.warn_save_failed(saved);
    }
}

/// runs.json을 항목마다 읽음 — 한 항목이 깨져도 나머지는 살림. (읽은 것, 읽지 못한 항목 수)
/// 파일 전체가 JSON 배열이 아니면 모두 읽지 못한 것으로 셈(최소 1).
pub fn parse_saved_runs(text: &str) -> (Vec<SavedRun>, usize) {
    let Ok(items) = serde_json::from_str::<Vec<Value>>(text) else {
        return (vec![], 1);
    };
    let mut lost = 0;
    let mut out = vec![];
    for item in items {
        match serde_json::from_value::<SavedRun>(item) {
            Ok(s) if s.macro_.is_object() => out.push(s),
            _ => lost += 1,
        }
    }
    (out, lost)
}

#[cfg(test)]
mod tests {
    #[test]
    fn a_bad_saved_run_does_not_drop_the_others() {
        let (runs, lost) = super::parse_saved_runs("[1, {\"bad\": true}]");
        assert_eq!((runs.len(), lost), (0, 2));
        let (runs, lost) = super::parse_saved_runs("not json");
        assert_eq!((runs.len(), lost), (0, 1));
        let (runs, lost) = super::parse_saved_runs("[]");
        assert_eq!((runs.len(), lost), (0, 0));
    }
}
