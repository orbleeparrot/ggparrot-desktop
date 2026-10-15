//! Tauri 앱 레이어 — 상태를 소유하고, 웹뷰 명령을 받고, 플랫폼 플러그인(트레이·딥링크·업데이트)을 이음.

mod commands;
mod shared;
mod state;
mod tray;
mod wizard;

use runner_core::credentials::{KeyringStore, MemoryStore, SecretStore};
use tauri::{DragDropEvent, Manager, RunEvent, WindowEvent};
use tauri_plugin_deep_link::DeepLinkExt;

use crate::shared::{Shared, S};

fn secret_store() -> (Box<dyn SecretStore>, bool) {
    // 개발 빌드 전용: 키를 OS 키체인 대신 메모리에만 둠(시연·스크린샷용 임시 홈에서 실제 키체인을 건드리지 않게).
    if runner_core::types::dev_env("GGP_MEMORY_KEYS").is_some_and(|v| v == "1") {
        return (Box::new(MemoryStore::new()), true);
    }
    if cfg!(any(target_os = "macos", target_os = "windows")) {
        (Box::new(KeyringStore::new()), true)
    } else {
        (Box::new(MemoryStore::new()), false)
    }
}

/// macOS 앱 메뉴 — 설정…(⌘,)·매크로 추가(⌘O)·편집(복사·붙여넣기)·윈도우.
/// Windows는 창 메뉴줄을 두지 않고 화면의 단축키(Ctrl+, 및 Ctrl+O)를 씀.
#[cfg(target_os = "macos")]
fn build_menu(app: &tauri::AppHandle) -> tauri::Result<tauri::menu::Menu<tauri::Wry>> {
    use tauri::menu::{AboutMetadata, Menu, MenuItem, PredefinedMenuItem, Submenu};
    let about = AboutMetadata {
        name: Some("껄무새".into()),
        version: Some(app.package_info().version.to_string()),
        comments: Some("GG Parrot — 껄무새 웹에서 만든 매크로를 이 컴퓨터에서 실행해요.".into()),
        ..Default::default()
    };
    let app_menu = Submenu::with_items(
        app,
        "껄무새",
        true,
        &[
            &PredefinedMenuItem::about(app, Some("껄무새 정보"), Some(about))?,
            &MenuItem::with_id(app, "check_update", "업데이트 확인…", true, None::<&str>)?,
            &MenuItem::with_id(app, "open_web", "껄무새 웹 열기", true, None::<&str>)?,
            &PredefinedMenuItem::separator(app)?,
            &MenuItem::with_id(app, "settings", "설정…", true, Some("CmdOrCtrl+,"))?,
            &PredefinedMenuItem::separator(app)?,
            &PredefinedMenuItem::hide(app, Some("껄무새 가리기"))?,
            &PredefinedMenuItem::hide_others(app, Some("기타 가리기"))?,
            &PredefinedMenuItem::show_all(app, Some("모두 보기"))?,
            &PredefinedMenuItem::separator(app)?,
            &PredefinedMenuItem::quit(app, Some("껄무새 종료"))?,
        ],
    )?;
    let file = Submenu::with_items(
        app,
        "파일",
        true,
        &[
            &MenuItem::with_id(app, "add_macro", "매크로 추가…", true, Some("CmdOrCtrl+O"))?,
            &MenuItem::with_id(app, "open_folder", "매크로 폴더 열기", true, None::<&str>)?,
        ],
    )?;
    let edit = Submenu::with_items(
        app,
        "편집",
        true,
        &[
            &PredefinedMenuItem::undo(app, Some("실행 취소"))?,
            &PredefinedMenuItem::redo(app, Some("실행 복귀"))?,
            &PredefinedMenuItem::separator(app)?,
            &PredefinedMenuItem::cut(app, Some("잘라내기"))?,
            &PredefinedMenuItem::copy(app, Some("복사하기"))?,
            &PredefinedMenuItem::paste(app, Some("붙여넣기"))?,
            &PredefinedMenuItem::select_all(app, Some("전체 선택"))?,
        ],
    )?;
    let window = Submenu::with_items(
        app,
        "윈도우",
        true,
        &[&PredefinedMenuItem::minimize(app, Some("최소화"))?, &PredefinedMenuItem::maximize(app, Some("확대/축소"))?],
    )?;
    Menu::with_items(app, &[&app_menu, &file, &edit, &window])
}

/// 웹뷰는 앱 화면에서만 움직임 — 화면에 스크립트가 끼어들어도 외부 주소로 이동해 키를 실어 보내지 못하게.
/// 외부 페이지는 Rust의 허용 목록(opener)으로만 시스템 브라우저에서 엶.
fn navigation_guard() -> tauri::plugin::TauriPlugin<tauri::Wry> {
    tauri::plugin::Builder::new("navigation-guard").on_navigation(|_webview, url| allowed_navigation(url)).build()
}

fn allowed_navigation(url: &tauri::Url) -> bool {
    match url.scheme() {
        // macOS·Linux의 앱 화면(tauri://localhost)과 내부 통신
        "tauri" | "ipc" | "asset" | "about" => true,
        // Windows의 앱 화면(http(s)://tauri.localhost)·개발 서버(localhost:1420)
        "http" | "https" => {
            matches!(url.host_str(), Some("tauri.localhost" | "ipc.localhost" | "asset.localhost"))
                || (cfg!(debug_assertions) && matches!(url.host_str(), Some("localhost" | "127.0.0.1")))
        }
        _ => false,
    }
}

pub fn run() {
    let app = tauri::Builder::default()
        // 앱은 하나만 — 두 번째 실행(딥링크 포함)은 기존 창을 앞으로 가져오고 링크만 넘김.
        .plugin(tauri_plugin_single_instance::init(|app, _argv, _cwd| {
            if let Some(s) = app.try_state::<S>() {
                s.bring_to_front();
            }
        }))
        .plugin(tauri_plugin_deep_link::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_clipboard_manager::init())
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(navigation_guard())
        .setup(|app| {
            // Windows: 시스템 제목 줄을 없애고 머리줄에 직접 그린 최소화·최대화·닫기 단추를 씀.
            // (macOS는 tauri.conf.json의 titleBarStyle=Overlay — 신호등이 머리줄 안에 놓임.)
            #[cfg(windows)]
            if let Some(w) = app.get_webview_window("main") {
                let _ = w.set_decorations(false);
            }
            let (store, supported) = secret_store();
            let shared: S = Shared::new(app.handle().clone(), store, supported);
            app.manage(shared.clone());
            // 매크로 폴더 첫 목록 읽기는 백그라운드에서 함 — 파일이 많아도 창이 늦게 뜨지 않게.
            {
                let s = shared.clone();
                tauri::async_runtime::spawn_blocking(move || {
                    s.refresh_library_quiet();
                    s.emit();
                });
            }

            #[cfg(target_os = "macos")]
            {
                let menu = build_menu(app.handle())?;
                app.set_menu(menu)?;
                let s3 = shared.clone();
                app.on_menu_event(move |_app, event| match event.id().as_ref() {
                    "settings" => s3.navigate("settings"),
                    "check_update" => {
                        s3.navigate("settings:about");
                        s3.update_check(false);
                    }
                    "add_macro" => s3.navigate("library"),
                    "open_web" => {
                        if let Err(e) = s3.open_web() {
                            s3.notice("warn", e);
                        }
                    }
                    "open_folder" => {
                        if let Err(e) = s3.open_library_folder() {
                            s3.notice("warn", e);
                        }
                    }
                    _ => {}
                });
            }

            // Windows는 HKCU에 ggparrot:// 처리기를 (다시) 등록함 — 관리자 권한이 필요 없음.
            #[cfg(windows)]
            {
                if let Err(e) = app.deep_link().register_all() {
                    eprintln!("deep link register failed: {e}");
                    shared.notice("warn", "웹의 [실행기 열기]로 이 앱을 여는 설정을 하지 못했어요. 매크로 파일은 그대로 쓸 수 있어요. 앱을 다시 설치하면 고쳐질 수 있어요.");
                }
            }
            let s2 = shared.clone();
            app.deep_link().on_open_url(move |event| {
                for url in event.urls() {
                    s2.handle_deep_link(url.as_str());
                }
            });
            if let Ok(Some(urls)) = app.deep_link().get_current() {
                for url in urls {
                    shared.handle_deep_link(url.as_str());
                }
            }
            shared.create_tray()?;
            shared.update_check(true);
            shared.start_update_timer();
            shared.resume_outbox();
            Ok(())
        })
        .on_window_event(|window, event| {
            let Some(s) = window.app_handle().try_state::<S>().map(|s| s.inner().clone()) else { return };
            match event {
                WindowEvent::Focused(focused) => s.note_focus(*focused),
                WindowEvent::DragDrop(DragDropEvent::Drop { paths, .. }) => s.import_dropped(paths.clone()),
                WindowEvent::CloseRequested { api, .. } => {
                    api.prevent_close();
                    // 설정이 켜져 있으면 트레이로 숨기고 매크로는 계속 돎. 꺼져 있으면 원래 규칙(실행 중이면 거절).
                    if let tray::CloseAction::Quit = s.on_close_requested() {
                        let w = window.clone();
                        tauri::async_runtime::spawn(async move {
                            s.shutdown().await;
                            let _ = w.destroy();
                            w.app_handle().exit(0);
                        });
                    }
                }
                _ => {}
            }
        })
        .invoke_handler(tauri::generate_handler![
            commands::get_state,
            commands::get_activity,
            commands::add_macro_file,
            commands::add_run_from_library,
            commands::remove_run,
            commands::set_run_mode,
            commands::start_run,
            commands::start_run_confirmed,
            commands::stop_run,
            commands::copy_activity,
            commands::set_slot_key,
            commands::reveal_slot_key,
            commands::set_slot_remember,
            commands::check_slot,
            commands::set_member_key,
            commands::reveal_member_key,
            commands::set_close_to_tray,
            commands::set_show_dev_log,
            commands::recover_storage,
            commands::reload_storage,
            commands::set_run_capital,
            commands::resolve_member_key,
            commands::update_check,
            commands::update_install,
            commands::update_restart,
            commands::library_refresh,
            commands::library_open_folder,
            commands::open_web,
            commands::library_import,
            commands::wizard_open,
            commands::wizard_choose,
            commands::wizard_go,
            commands::wizard_back,
            commands::wizard_close,
            commands::wizard_refresh_ip,
            commands::wizard_copy_ip,
            commands::wizard_copy_and_open,
            commands::wizard_open_api_page,
            commands::wizard_open_home,
            commands::wizard_recheck,
            commands::wizard_recopy,
            commands::wizard_set_key,
            commands::wizard_set_remember,
        ])
        .build(tauri::generate_context!())
        .expect("error while building tauri application");

    app.run(|handle, event| {
        // macOS: Dock 아이콘을 누르면(창을 숨겼을 때 포함) 창을 다시 엶.
        #[cfg(target_os = "macos")]
        if let RunEvent::Reopen { .. } = event {
            if let Some(s) = handle.try_state::<S>() {
                s.inner().show_main_window();
            }
            return;
        }
        // ⌘Q·메뉴 종료도 창 닫기와 같은 규칙 — 실행 중인 매크로가 있으면 끄지 않음.
        if let RunEvent::ExitRequested { api, code, .. } = event {
            if code.is_none() {
                if let Some(s) = handle.try_state::<S>() {
                    let s = s.inner().clone();
                    api.prevent_exit();
                    if s.request_close() {
                        // 웹 가이드에 '닫힘'을 알리고(최대 1.5초) 끔. exit(0)은 code가 있어 다시 막히지 않음.
                        let h = handle.clone();
                        tauri::async_runtime::spawn(async move {
                            s.shutdown().await;
                            h.exit(0);
                        });
                    }
                }
            }
        }
    });
}

#[cfg(test)]
mod navigation_tests {
    use super::allowed_navigation;

    #[test]
    fn the_webview_stays_inside_the_app() {
        let ok = |u: &str| allowed_navigation(&tauri::Url::parse(u).unwrap());
        assert!(ok("tauri://localhost/index.html"));
        assert!(ok("http://tauri.localhost/"));
        assert!(ok("http://localhost:1420/"), "개발 빌드의 개발 서버");
        assert!(!ok("https://evil.example/?k=secret"));
        assert!(!ok("http://tauri.localhost.evil.example/"));
        assert!(!ok("javascript:alert(1)"));
        assert!(!ok("file:///etc/passwd"));
    }
}
