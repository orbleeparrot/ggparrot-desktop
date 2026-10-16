//! 매크로 폴더 — 목록·가져오기·끌어다 놓기.

use super::*;

impl Shared {
    /// 폴더는 상태 잠금 밖에서 읽음 — 파일이 많아도 실행 중인 매크로의 이벤트를 붙잡지 않게.
    /// 크기·수정 시각이 그대로인 파일은 캐시에서 꺼내고 다시 열지 않음.
    fn scan_library(&self) -> Result<(), String> {
        let dir = self.lock().library_dir.clone();
        let scan = {
            let mut cache = lock_or_recover(&self.library_cache);
            library::scan_cached(&dir, &mut cache)
        };
        let mut c = self.lock();
        match scan {
            Ok(s) => {
                c.library = s.files;
                c.library_skipped = s.skipped;
                c.library_truncated = s.truncated;
                c.library_rev += 1;
                c.log_library_problems(None);
                Ok(())
            }
            Err(e) => {
                c.library.clear();
                c.log_library_problems(Some(&e));
                Err(e)
            }
        }
    }

    pub(crate) fn refresh_library_quiet(&self) {
        let _ = self.scan_library();
    }

    pub fn refresh_library(self: &S) -> Result<(), String> {
        let result = self.scan_library();
        self.emit();
        result
    }

    pub fn library_dir(&self) -> PathBuf {
        self.lock().library_dir.clone()
    }

    /// 여러 파일을 매크로 폴더로 가져옴. (가져온 파일 이름들, 실패 이유들)
    pub fn import_paths(self: &S, paths: &[PathBuf]) -> (Vec<String>, Vec<String>) {
        let dir = self.library_dir();
        let mut ok = vec![];
        let mut errors = vec![];
        for p in paths {
            let name = p.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
            match library::import(&dir, p) {
                Ok(n) => ok.push(n),
                Err(e) => errors.push(format!("{name}: {e}")),
            }
        }
        let _ = self.refresh_library();
        (ok, errors)
    }

    /// 창에 끌어다 놓은 파일 — 폴더로 가져오고 알림.
    pub fn import_dropped(self: &S, paths: Vec<PathBuf>) {
        let (ok, errors) = self.import_paths(&paths);
        if !ok.is_empty() {
            self.notice_from(
                "library",
                "info",
                format!("매크로 파일 {}개를 매크로 폴더로 가져왔어요. [매크로 추가]에서 고르세요.", ok.len()),
            );
            self.navigate("library");
        }
        self.warn_import_failures(&errors);
    }

    /// 가져오지 못한 파일과 이유를 한 알림으로(없으면 아무것도 하지 않음).
    pub fn warn_import_failures(&self, errors: &[String]) {
        if !errors.is_empty() {
            self.notice_from("library", "warn", format!("가져오지 못한 파일이 있어요.\n\n{}", errors.join("\n")));
        }
    }

    pub fn open_library_folder(self: &S) -> Result<(), String> {
        let dir = self.library_dir();
        std::fs::create_dir_all(&dir).map_err(|_| "매크로 폴더를 만들지 못했어요.".to_string())?;
        self.app
            .opener()
            .open_path(dir.to_string_lossy().to_string(), None::<&str>)
            .map_err(|_| "매크로 폴더를 열지 못했어요.".to_string())
    }
}
