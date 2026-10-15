//! 앱 컨트롤러 — 키 확인·저장, 실행 슬롯 여러 개의 시작·종료, 웹 연결.
//! 상태·출력은 여기, 나머지는 관심사별 하위 모듈(persist·keys·checks·library_folder·runs·launch·
//! bot_events·run_check·deep_link·updates·lifecycle)의 `impl Shared`에 둠.
//! 락은 await를 넘어 들고 있지 않음. 바뀐 상태는 `emit()`으로 웹뷰에 스냅샷을 보냄.

use runner_core::bot::{spawn_bot, BotEvent, StopMode};
use runner_core::check::{check_http_client, mock_check, perform_connection_check};
use runner_core::connection::{self, check_view, BeginOutcome, PreflightResult};
use runner_core::credentials::{self, LoadStatus, SecretStore};
use runner_core::library;
use runner_core::outbox::EventOutbox;
use runner_core::protocol::{parse_protocol_url, Environment};
use runner_core::server::{self, ServerClient, StartError};
use runner_core::setup_progress::SetupProgressReporter;
use runner_core::types::{
    exchange_label, macro_side, server_base, session_payload, Credentials, Exchange, KeyPair, KeySlot, RunMode,
    LOCAL_SERVER_BASE,
};
use runner_core::APP_TITLE;
use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::Duration;
use tauri::{AppHandle, Emitter, Manager};
use tauri_plugin_clipboard_manager::ClipboardExt;
use tauri_plugin_opener::OpenerExt;

/// 껄무새 웹 주소 — 매크로를 만들고 받는 곳.
pub const WEB_URL: &str = "https://gg-parrot.vercel.app/";

use crate::state::{now_ms, Core, KeyCheckRecord, Notice, Run, RunData, SavedRun, StartReply, WizardTarget};

pub const EV_STATE: &str = "runner:state";
pub const EV_ACTIVITY: &str = "runner:activity";
pub const EV_NOTICE: &str = "runner:notice";
pub const EV_CONFIRM: &str = "runner:confirm";
pub const EV_NAVIGATE: &str = "runner:navigate";

pub struct Shared {
    core: Mutex<Core>,
    pub app: AppHandle,
    pub http: reqwest::Client,
    pub check_http: reqwest::Client,
    pub outbox: Arc<EventOutbox>,
    pub store: Box<dyn SecretStore>,
    pub data_dir: PathBuf,
    pub pending_update: Mutex<Option<tauri_plugin_updater::Update>>,
    /// 내려받아 서명까지 확인한 설치 파일. 다시 시작할 때 설치함(매크로 실행 중에는 설치하지 않음).
    pub pending_bytes: Mutex<Option<Vec<u8>>>,
    pub tray_key: Mutex<String>,
    /// 매크로 폴더 읽기 캐시(바뀐 파일만 다시 엶). 상태 잠금과 따로 둠.
    pub library_cache: Mutex<library::LibraryCache>,
    /// 저장 실패를 이미 알렸는지 여부(같은 알림을 반복하지 않게).
    pub save_warned: std::sync::atomic::AtomicBool,
    /// 앱 버전 — tauri.conf.json의 version(업데이터가 비교하는 값과 같음).
    pub version: String,
    /// 화면에 마지막으로 보낸 매크로 폴더 목록의 판(Core::library_rev). 같으면 목록을 싣지 않음.
    pub sent_library_rev: std::sync::atomic::AtomicU64,
    /// 그리기부터 화면으로 보내기까지 한 번에 하나만 — 먼저 그린 상태가 나중에 도착해 화면을 되돌리지 않게.
    /// Core 잠금과 따로 둠(보내는 동안 다른 일이 Core를 기다리지 않게).
    pub emit_lock: Mutex<()>,
    /// 봇이 보낸 상태 갱신을 모아 보내는 중인지 여부([`Shared::emit_soon`]).
    pub emit_queued: std::sync::atomic::AtomicBool,
    /// OS 알림 묶음(제목별) — 같은 원인으로 여러 실행이 같은 알림을 띄우지 않게.
    pub os_alerts: Mutex<std::collections::HashMap<String, AlertSlot>>,
}

pub type S = Arc<Shared>;

pub fn data_dir() -> PathBuf {
    runner_core::paths::data_dir()
}

impl Shared {
    pub fn new(app: AppHandle, store: Box<dyn SecretStore>, remember_supported: bool) -> S {
        let library_dir =
            runner_core::types::dev_env("GGP_MACRO_DIR").map(PathBuf::from).unwrap_or_else(library::default_dir);
        let core = Core::new(server_base(), remember_supported, library_dir);
        let version = app.package_info().version.to_string();
        let shared = Arc::new(Shared {
            core: Mutex::new(core),
            app,
            http: runner_core::net::http_client(),
            check_http: check_http_client(),
            outbox: Arc::new(EventOutbox::open(None)),
            store,
            data_dir: data_dir(),
            pending_update: Mutex::new(None),
            pending_bytes: Mutex::new(None),
            version,
            tray_key: Mutex::new(String::new()),
            library_cache: Mutex::new(library::LibraryCache::default()),
            save_warned: std::sync::atomic::AtomicBool::new(false),
            emit_lock: Mutex::new(()),
            sent_library_rev: std::sync::atomic::AtomicU64::new(u64::MAX),
            emit_queued: std::sync::atomic::AtomicBool::new(false),
            os_alerts: Mutex::new(Default::default()),
        });
        shared.load_remembered();
        shared.load_saved();
        // 매크로 폴더는 창을 띄운 뒤 백그라운드에서 읽음(lib.rs setup).
        shared
    }

    pub fn lock(&self) -> MutexGuard<'_, Core> {
        self.core.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// 창을 앞으로 — 트레이로 숨겨 둔 창(Dock 아이콘 포함)도 되살림.
    pub fn bring_to_front(&self) {
        self.show_main_window();
    }

    pub fn clipboard_text(&self) -> String {
        self.app.clipboard().read_text().unwrap_or_default()
    }

    pub fn write_clipboard(&self, text: &str) -> bool {
        self.app.clipboard().write_text(text.to_string()).is_ok()
    }

    pub fn open_url(&self, url: &str) -> bool {
        self.app.opener().open_url(url, None::<&str>).is_ok()
    }

    /// 껄무새 웹을 기본 브라우저로 엶(머리줄·메뉴·트레이·매크로 추가 창).
    pub fn open_web(&self) -> Result<(), String> {
        if self.open_url(WEB_URL) {
            Ok(())
        } else {
            Err(format!("브라우저를 열지 못했어요. 주소창에 {WEB_URL} 을 입력하세요."))
        }
    }
}

mod alerts;
mod bot_events;
mod checks;
mod support;
pub use support::os_summary;
mod deep_link;
mod keys;
mod launch;
pub use alerts::AlertSlot;
#[cfg(test)]
pub(crate) use launch::leftover_warning;
mod library_folder;
mod lifecycle;
mod output;
mod persist;
mod run_check;
mod runs;
mod updates;

use updates::is_newer;
