//! 앱 업데이트 — 확인·내려받기·다시 시작해 설치.

use super::*;

impl Shared {
    // 확인 → 내려받기(매크로가 돌아도 됨) → 다시 시작할 때 설치. 설치는 실행 중인 매크로가 없을 때만 함.
    // Windows는 설치 파일을 띄우며 앱이 바로 꺼지므로, 내려받기와 설치를 나눠 매크로가 갑자기 멈추지 않게 함.

    /// 6시간마다 조용히 확인함(켤 때 한 번은 setup에서).
    pub fn start_update_timer(self: &S) {
        let me = self.clone();
        tauri::async_runtime::spawn(async move {
            loop {
                tokio::time::sleep(std::time::Duration::from_secs(6 * 60 * 60)).await;
                me.update_check(true);
            }
        });
    }

    pub fn update_check(self: &S, silent: bool) {
        use tauri_plugin_updater::UpdaterExt;
        {
            let mut c = self.lock();
            if matches!(c.update.status, "unsupported" | "downloading" | "checking" | "ready" | "installing") {
                return;
            }
            c.update.blocked = install_blocker();
            if !silent {
                c.update.status = "checking";
                c.update.error.clear();
            }
        }
        self.emit();
        let me = self.clone();
        tauri::async_runtime::spawn(async move {
            // 확인 요청에 제한 시간을 둠 — 없으면 응답이 멈췄을 때 '확인 중'에 머묾.
            let result = match me.app.updater_builder().timeout(Duration::from_secs(20)).build() {
                Ok(u) => u.check().await.map_err(|e| e.to_string()),
                Err(e) => Err(e.to_string()),
            };
            {
                let mut c = me.lock();
                c.update.checked_at = now_ms();
                match result {
                    Ok(Some(update)) => {
                        c.update.status = "available";
                        c.update.version = update.version.clone();
                        c.update.notes = update.body.clone().unwrap_or_default();
                        *me.pending_update.lock().unwrap() = Some(update);
                        *me.pending_bytes.lock().unwrap() = None;
                    }
                    Ok(None) => {
                        c.update.status = "none";
                    }
                    Err(e) => {
                        if silent {
                            if c.update.status != "available" {
                                c.update.status = "idle";
                            }
                        } else {
                            eprintln!("update check failed: {e}");
                            c.update.status = "error";
                            c.update.error =
                                "업데이트 정보를 확인하지 못했어요. 인터넷 연결을 확인한 뒤 다시 시도하세요.".into();
                        }
                    }
                }
            }
            me.emit();
        });
    }

    /// 새 버전 설치 파일을 내려받고 서명을 확인함. 설치는 하지 않음 — 끝나면 status=ready.
    pub fn update_install(self: &S) -> Result<(), String> {
        let update = self.pending_update.lock().unwrap().clone().ok_or("먼저 업데이트를 확인하세요.")?;
        {
            let mut c = self.lock();
            if !c.update.blocked.is_empty() {
                return Err(c.update.blocked.clone());
            }
            if matches!(c.update.status, "downloading" | "ready" | "installing") {
                return Ok(());
            }
            c.update.status = "downloading";
            c.update.progress = -1.0;
            c.update.downloaded = 0;
            c.update.total = 0;
            c.update.error.clear();
        }
        self.emit();
        let me = self.clone();
        tauri::async_runtime::spawn(async move {
            let got = Arc::new(Mutex::new(0u64));
            let me2 = me.clone();
            let got2 = got.clone();
            let download = update.download(
                move |chunk, total| {
                    let mut g = got2.lock().unwrap();
                    *g += chunk as u64;
                    let total = total.unwrap_or(0);
                    let progress = if total > 0 { (*g as f64 / total as f64).min(1.0) } else { -1.0 };
                    let changed = {
                        let mut c = me2.lock();
                        // 0.5% 단위(크기를 모르면 256KB 단위)로만 다시 그림.
                        let step =
                            |p: f64, b: u64| if p >= 0.0 { (p * 200.0).floor() as u64 } else { b / (256 * 1024) };
                        let before = step(c.update.progress, c.update.downloaded);
                        c.update.progress = progress;
                        c.update.downloaded = *g;
                        c.update.total = total;
                        step(progress, *g) != before
                    };
                    if changed {
                        me2.emit();
                    }
                },
                || {},
            );
            // 내려받기는 크기에 따라 오래 걸릴 수 있어 전체 제한 대신, 1분 넘게 한 바이트도 오지 않으면 멈춘 것으로 봄.
            tokio::pin!(download);
            let (mut seen, mut idle) = (0u64, 0u64);
            let result = loop {
                tokio::select! {
                    r = &mut download => break r.map_err(|e| e.to_string()),
                    _ = tokio::time::sleep(Duration::from_secs(15)) => {
                        let now = *got.lock().unwrap();
                        if now == seen {
                            idle += 15;
                            if idle >= 60 {
                                break Err("download stalled for 60s".to_string());
                            }
                        } else {
                            (seen, idle) = (now, 0);
                        }
                    }
                }
            };
            {
                let mut c = me.lock();
                match result {
                    Ok(bytes) => {
                        *me.pending_bytes.lock().unwrap() = Some(bytes);
                        c.update.status = "ready";
                        c.update.progress = 1.0;
                        c.update.downloaded = c.update.total.max(c.update.downloaded);
                    }
                    Err(e) => {
                        c.update.status = "error";
                        eprintln!("update download failed: {e}");
                        c.update.error =
                            "업데이트를 내려받지 못했어요. 인터넷 연결을 확인한 뒤 다시 시도하세요.".into();
                    }
                }
            }
            me.emit();
        });
        Ok(())
    }

    /// 내려받은 새 버전을 설치하고 다시 시작함. 실행 중인 매크로가 있으면 거절함.
    /// macOS는 앱 묶음을 바꾼 뒤 다시 켜고, Windows는 설치 화면(진행 막대)이 뜨며 앱이 꺼졌다가 설치가 끝나면 다시 켜짐.
    pub fn update_restart(self: &S) -> Result<(), String> {
        let running = self.lock().busy_count();
        if running > 0 {
            return Err(format!(
                "실행 중이거나 시작하는 매크로가 {running}개 있어 지금은 설치할 수 없어요. 모두 종료한 뒤 다시 누르세요."
            ));
        }
        let update = self.pending_update.lock().unwrap().clone().ok_or("먼저 업데이트를 내려받으세요.")?;
        let bytes = self.pending_bytes.lock().unwrap().take().ok_or("먼저 업데이트를 내려받으세요.")?;
        self.lock().update.status = "installing";
        self.emit();
        let me = self.clone();
        tauri::async_runtime::spawn(async move {
            me.shutdown().await;
            // Windows는 여기서 설치 파일을 띄우고 프로세스가 끝남.
            match update.install(&bytes) {
                Ok(()) => me.app.restart(),
                Err(e) => {
                    // shutdown()이 닫아 둔 앱을 다시 엶(그대로면 시작·검사가 모두 막힘).
                    // 받은 파일은 버리고 다음 확인에서 새로 받음.
                    drop(bytes);
                    {
                        let mut c = me.lock();
                        c.closed = false;
                        c.update.status = "error";
                        eprintln!("update install failed: {e}");
                        c.update.error = "업데이트를 설치하지 못했어요. 앱을 응용 프로그램(설치) 폴더에서 실행했는지 확인한 뒤 다시 시도하세요.".into();
                    }
                    me.emit();
                }
            }
        });
        Ok(())
    }
}

/// a가 b보다 새 버전인지 여부(점으로 나눈 숫자를 차례로 비교, 비거나 못 읽으면 거짓).
pub fn is_newer(a: &str, b: &str) -> bool {
    let parse = |v: &str| -> Option<Vec<u64>> { v.trim().split('.').map(|p| p.parse().ok()).collect() };
    match (parse(a), parse(b)) {
        (Some(a), Some(b)) if !b.is_empty() => a > b,
        _ => false,
    }
}

/// 이 위치에서 실행 중이면 업데이트를 설치할 수 없음 — 이유를 돌려줌(가능하면 빈 문자열).
/// macOS는 디스크 이미지(.dmg)에서 바로 켰거나, 다운로드 폴더에서 켜서 격리 위치(App Translocation)로 옮겨졌을 때 앱을 바꿀 수 없음.
pub fn install_blocker() -> String {
    let exe = std::env::current_exe().map(|p| p.to_string_lossy().to_string()).unwrap_or_default();
    install_blocker_for(&exe, cfg!(target_os = "macos"))
}

pub fn install_blocker_for(exe: &str, mac: bool) -> String {
    if !mac {
        return String::new();
    }
    if exe.starts_with("/Volumes/") || exe.contains("/AppTranslocation/") {
        return "설치 파일(디스크 이미지)이나 다운로드 폴더에서 바로 실행 중이라 업데이트를 설치할 수 없어요. \
                앱을 응용 프로그램 폴더로 옮긴 뒤 다시 실행하세요."
            .into();
    }
    String::new()
}

#[cfg(test)]
mod tests {
    use super::{install_blocker_for, is_newer};

    #[test]
    fn newer_only_when_version_went_up() {
        assert!(is_newer("0.18.3", "0.18.2"));
        assert!(is_newer("0.18.10", "0.18.9"));
        assert!(!is_newer("0.18.2", "0.18.3"), "되돌린 것은 업데이트 알림을 띄우지 않는다");
        assert!(!is_newer("0.18.2", "0.18.2"));
        assert!(!is_newer("0.18.2", ""));
    }

    #[test]
    fn mac_blocks_disk_image_and_translocation() {
        let app = "/Applications/GG Parrot.app/Contents/MacOS/ggparrot-desktop";
        assert!(install_blocker_for(app, true).is_empty());
        assert!(!install_blocker_for("/Volumes/GG Parrot/GG Parrot.app/Contents/MacOS/x", true).is_empty());
        assert!(!install_blocker_for("/private/var/folders/x/AppTranslocation/ABC/d/a.app/Contents/MacOS/x", true)
            .is_empty());
        assert!(install_blocker_for("/Volumes/x", false).is_empty(), "Windows 는 막지 않는다");
    }
}
