//! 앱 저장 위치. 데이터 폴더는 `ggparrot-desktop` 하나.

use std::path::PathBuf;

/// 데이터 폴더 이름(macOS `~/Library/Application Support/…`, Windows `%LOCALAPPDATA%\…`).
pub const DATA_DIR_NAME: &str = "ggparrot-desktop";

/// 데이터 폴더들이 들어가는 곳.
pub fn data_base() -> PathBuf {
    #[cfg(target_os = "macos")]
    {
        if let Some(home) = dirs::home_dir() {
            return home.join("Library").join("Application Support");
        }
    }
    #[cfg(windows)]
    {
        if let Some(local) = std::env::var_os("LOCALAPPDATA").filter(|v| !v.is_empty()) {
            return PathBuf::from(local);
        }
    }
    dirs::data_local_dir().unwrap_or_else(std::env::temp_dir)
}

pub fn data_dir() -> PathBuf {
    data_base().join(DATA_DIR_NAME)
}

/// 매크로 폴더 — 앱 데이터 폴더 아래(`ggparrot-desktop/macros`). 설정·실행 목록과 같은 곳에 두고 하위
/// 폴더로 나눔. 문서 폴더가 아니라서 macOS가 접근 허용을 묻지 않음.
pub fn macros_dir() -> PathBuf {
    data_dir().join("macros")
}
