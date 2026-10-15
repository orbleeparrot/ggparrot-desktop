//! 설정 › 정보의 문의용 정보 — OS 이름·버전과 [문의용 정보 복사].

use super::*;
use std::sync::OnceLock;

/// "macOS 15.6.1 (arm64)"·"Windows 11 Pro 10.0.26100 (x86_64)" — 한 번만 읽음.
pub fn os_summary() -> &'static str {
    static OS: OnceLock<String> = OnceLock::new();
    OS.get_or_init(|| {
        let info = os_info::get();
        let name = match info.os_type() {
            os_info::Type::Macos => "macOS".to_string(),
            other => info.edition().map(str::to_string).unwrap_or_else(|| other.to_string()),
        };
        let arch = info.architecture().unwrap_or(std::env::consts::ARCH);
        format!("{name} {} ({arch})", info.version())
    })
}

impl Shared {
    /// 문의용 정보를 클립보드로 — 공인 IP는 지금 아는 값(조회 중·지난 값이면 그 상태)으로 적음.
    pub fn copy_support_info(&self) -> bool {
        let text = self.lock().support_text(&self.version, os_summary(), chrono::Local::now());
        self.write_clipboard(&text)
    }
}
