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
            // 받은 업데이트는 Core 잠금 밖에서 먼저 넣어 둠 — 'available'이 보이면 내려받기가 늘 찾을 수 있게.
            let found = match result {
                Ok(Some(update)) => {
                    let info = (update.version.clone(), update.body.clone().unwrap_or_default());
                    *lock_or_recover(&me.pending_update) = Some(update);
                    *lock_or_recover(&me.pending_bytes) = None;
                    Ok(Some(info))
                }
                Ok(None) => Ok(None),
                Err(e) => Err(e),
            };
            {
                let mut c = me.lock();
                c.update.checked_at = now_ms();
                match found {
                    Ok(Some((version, notes))) => {
                        // 같은 새 버전을 6시간마다 거듭 남기지 않음.
                        if c.update.status != "available" || c.update.version != version {
                            c.push_app_log("info", "update", &format!("새 버전 v{version}이 있어요"), "");
                        }
                        c.update.status = "available";
                        c.update.version = version;
                        c.update.notes = notes;
                    }
                    Ok(None) => {
                        c.update.status = "none";
                    }
                    Err(e) => {
                        // 자동 확인 실패(아직 공개한 릴리스가 없음·잠깐 끊김)는 정보로만 — 실행할 때마다 '주의'가 쌓이지 않게.
                        // 사용자가 직접 누른 확인이 실패했을 때만 주의로 남김.
                        if silent {
                            c.push_app_log(
                                "info",
                                "update",
                                "업데이트 정보를 받지 못했어요",
                                &format!("자동 확인 · {e}"),
                            );
                        } else {
                            c.push_app_log(
                                "warn",
                                "update",
                                "업데이트 정보를 확인하지 못했어요",
                                &format!("직접 확인 · {e}"),
                            );
                        }
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

    /// 지금 공개된 최신 버전을 다시 확인함 — 6시간마다 확인한 결과를 기억해 두면 그 사이 나온 더 새 버전을 놓쳐
    /// 업데이트를 두 번 해야 했음(2026-10-18). 못 받거나 새 버전이 없으면 None(호출자는 기억해 둔 것을 씀).
    async fn latest_update(&self) -> Option<tauri_plugin_updater::Update> {
        use tauri_plugin_updater::UpdaterExt;
        let updater = self.app.updater_builder().timeout(Duration::from_secs(20)).build().ok()?;
        updater.check().await.ok().flatten()
    }

    /// 기억해 둔 업데이트(`have`)보다 확인한 최신(`latest`)이 새로우면 그것으로 바꾸고 화면 버전·바뀐 점도 고침.
    /// 바꿨으면 참 — 받아 둔 설치 파일(옛 버전)은 버림.
    fn adopt_latest(&self, latest: Option<tauri_plugin_updater::Update>, have: &str) -> bool {
        let Some(latest) = latest else { return false };
        if !newer_than_pending(&latest.version, have) {
            return false;
        }
        let (version, notes) = (latest.version.clone(), latest.body.clone().unwrap_or_default());
        *lock_or_recover(&self.pending_update) = Some(latest);
        *lock_or_recover(&self.pending_bytes) = None;
        let mut c = self.lock();
        c.push_app_log(
            "info",
            "update",
            &format!("더 새 버전 v{version}이 나와 그것으로 받아요"),
            &format!("v{have} 대신"),
        );
        c.update.version = version;
        c.update.notes = notes;
        true
    }

    /// 새 버전 설치 파일을 내려받고 서명을 확인함. 설치는 하지 않음 — 끝나면 status=ready.
    /// 받기 직전에 최신을 다시 확인해 그 버전을 받음(확인한 뒤 또 나온 버전까지 한 번에).
    pub fn update_install(self: &S) -> Result<(), String> {
        lock_or_recover(&self.pending_update).as_ref().ok_or("먼저 업데이트를 확인하세요.")?;
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
            let have = me.lock().update.version.clone();
            let latest = me.latest_update().await;
            if me.adopt_latest(latest, &have) {
                me.emit();
            }
            let Some(update) = lock_or_recover(&me.pending_update).clone() else {
                me.finish_download(Err("no pending update".into()));
                return;
            };
            let result = me.download_watched(&update).await;
            me.finish_download(result);
        });
        Ok(())
    }

    /// 내려받기 — 크기에 따라 오래 걸릴 수 있어 전체 제한 대신, 1분 넘게 한 바이트도 오지 않으면 멈춘 것으로 봄.
    async fn download_watched(self: &S, update: &tauri_plugin_updater::Update) -> Result<Vec<u8>, String> {
        use std::sync::atomic::{AtomicU64, Ordering};
        let got = Arc::new(AtomicU64::new(0));
        let (me, got2) = (self.clone(), got.clone());
        let download = update.download(
            move |chunk, total| {
                let now = got2.fetch_add(chunk as u64, Ordering::Relaxed) + chunk as u64;
                me.note_download_progress(now, total.unwrap_or(0));
            },
            || {},
        );
        tokio::pin!(download);
        let (mut seen, mut idle) = (0u64, 0u64);
        loop {
            tokio::select! {
                r = &mut download => return r.map_err(|e| e.to_string()),
                _ = tokio::time::sleep(Duration::from_secs(15)) => {
                    let now = got.load(Ordering::Relaxed);
                    if now == seen {
                        idle += 15;
                        if idle >= 60 {
                            return Err("download stalled for 60s".to_string());
                        }
                    } else {
                        (seen, idle) = (now, 0);
                    }
                }
            }
        }
    }

    /// 내려받은 바이트를 상태에 씀. 0.5% 단위(크기를 모르면 256KB 단위)로만 다시 그림.
    fn note_download_progress(&self, got: u64, total: u64) {
        let progress = if total > 0 { (got as f64 / total as f64).min(1.0) } else { -1.0 };
        let step = |p: f64, b: u64| if p >= 0.0 { (p * 200.0).floor() as u64 } else { b / (256 * 1024) };
        let changed = {
            let mut c = self.lock();
            let before = step(c.update.progress, c.update.downloaded);
            c.update.progress = progress;
            c.update.downloaded = got;
            c.update.total = total;
            step(progress, got) != before
        };
        if changed {
            self.emit();
        }
    }

    /// 내려받기 결과를 상태에 씀. 받은 파일은 다시 시작할 때 설치함([`Shared::update_restart`]).
    fn finish_download(&self, result: Result<Vec<u8>, String>) {
        // 받은 파일은 Core 잠금 밖에서 먼저 넣어 둠 — 'ready'가 보이면 설치가 늘 찾을 수 있게.
        let result = result.map(|bytes| *lock_or_recover(&self.pending_bytes) = Some(bytes));
        {
            let mut c = self.lock();
            match result {
                Ok(()) => {
                    let title = format!("v{} 설치 파일을 받았어요", c.update.version);
                    c.push_app_log("info", "update", &title, "다시 시작하면 설치해요.");
                    c.update.status = "ready";
                    c.update.progress = 1.0;
                    c.update.downloaded = c.update.total.max(c.update.downloaded);
                }
                Err(e) => {
                    c.update.status = "error";
                    eprintln!("update download failed: {e}");
                    c.push_app_log("error", "update", "업데이트를 내려받지 못했어요", &e);
                    c.update.error = "업데이트를 내려받지 못했어요. 인터넷 연결을 확인한 뒤 다시 시도하세요.".into();
                }
            }
        }
        self.emit();
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
        lock_or_recover(&self.pending_update).as_ref().ok_or("먼저 업데이트를 내려받으세요.")?;
        lock_or_recover(&self.pending_bytes).as_ref().ok_or("먼저 업데이트를 내려받으세요.")?;
        self.lock().update.status = "installing";
        self.emit();
        let me = self.clone();
        tauri::async_runtime::spawn(async move {
            // 받아 둔 뒤 더 새 버전이 나왔으면 그것을 받아 바로 설치 — 설치 뒤 또 업데이트하지 않게.
            let have = me.lock().update.version.clone();
            let latest = me.latest_update().await;
            if me.adopt_latest(latest, &have) {
                me.lock().update.status = "downloading";
                me.emit();
                let update = lock_or_recover(&me.pending_update).clone();
                let result = match update {
                    Some(u) => me.download_watched(&u).await,
                    None => Err("no pending update".into()),
                };
                match result {
                    Ok(b) => *lock_or_recover(&me.pending_bytes) = Some(b),
                    Err(e) => {
                        me.finish_download(Err(e));
                        return;
                    }
                }
                me.lock().update.status = "installing";
                me.emit();
            }
            let (Some(update), Some(bytes)) =
                (lock_or_recover(&me.pending_update).clone(), lock_or_recover(&me.pending_bytes).take())
            else {
                me.finish_download(Err("no downloaded update".into()));
                return;
            };
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
                        c.push_app_log("error", "update", "업데이트를 설치하지 못했어요", &e.to_string());
                        c.update.error = "업데이트를 설치하지 못했어요. 앱을 응용 프로그램(설치) 폴더에서 실행했는지 확인한 뒤 다시 시도하세요.".into();
                    }
                    me.emit();
                }
            }
        });
        Ok(())
    }
}

/// 확인한 최신(`latest`)으로 기억해 둔 업데이트(`have`)를 바꿀지 — 더 새로울 때만. 기억해 둔 버전을 모르면(빈 값) 바꿈.
pub fn newer_than_pending(latest: &str, have: &str) -> bool {
    have.trim().is_empty() || is_newer(latest, have)
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
    use super::{install_blocker_for, is_newer, newer_than_pending};

    #[test]
    fn a_later_release_replaces_the_remembered_update_so_one_update_reaches_the_latest() {
        // 0.20.10을 확인해 둔 뒤 0.20.11이 나왔으면 0.20.11로 받음(두 번 업데이트하지 않게)
        assert!(newer_than_pending("0.20.11", "0.20.10"));
        assert!(!newer_than_pending("0.20.10", "0.20.10"), "같은 버전이면 받아 둔 파일을 그대로 쓴다");
        assert!(!newer_than_pending("0.20.9", "0.20.10"));
        assert!(newer_than_pending("0.20.11", ""));
    }

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
        let app = "/Applications/GGParrot.app/Contents/MacOS/ggparrot-desktop";
        assert!(install_blocker_for(app, true).is_empty());
        assert!(!install_blocker_for("/Volumes/GGParrot/GGParrot.app/Contents/MacOS/x", true).is_empty());
        assert!(
            !install_blocker_for("/private/var/folders/x/AppTranslocation/ABC/d/a.app/Contents/MacOS/x", true)
                .is_empty()
        );
        assert!(install_blocker_for("/Volumes/x", false).is_empty(), "Windows 는 막지 않는다");
    }
}
