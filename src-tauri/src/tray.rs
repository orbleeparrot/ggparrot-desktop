//! 트레이(메뉴 막대·알림 영역) 아이콘과 백그라운드 실행.
//! 창을 닫으면(설정: 창을 닫으면 트레이에서 계속 실행) 창을 숨기고 매크로는 계속 돎.
//! macOS는 Dock 아이콘도 숨기고, Windows는 작업 표시줄에서 사라짐. 트레이에서 다시 엶.

use tauri::image::Image;
use tauri::menu::{Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Manager, Wry};

use crate::shared::{Shared, S};

pub const TRAY_ID: &str = "main";
const MAX_RUN_LINES: usize = 8;

/// 트레이 아이콘 — 배경 없이. macOS는 메뉴 막대 밝기를 따라 색이 바뀌는 템플릿(선 그림),
/// Windows는 색이 있는 새.
fn tray_image() -> Image<'static> {
    let bytes: &'static [u8] = if cfg!(target_os = "macos") {
        include_bytes!("../icons/tray-template@2x.png")
    } else {
        include_bytes!("../icons/tray@2x.png")
    };
    Image::from_bytes(bytes).expect("tray icon is a valid PNG")
}

/// 트레이에 보일 요약 — 실행 중인 매크로와, 오류로 멈춰 확인이 필요한 매크로.
struct TraySummary {
    /// 메뉴를 다시 만들지 판단하는 열쇠.
    key: String,
    lines: Vec<(String, String)>,
    tooltip: String,
    head: String,
    /// 메뉴에 올릴 매크로 전체 수(줄 수보다 많으면 '외 N개').
    total: usize,
}

fn summary(s: &Shared) -> TraySummary {
    let c = s.lock();
    let listed: Vec<_> = c.runs.iter().filter(|r| r.running() || r.needs_attention()).collect();
    let running = listed.iter().filter(|r| r.running()).count();
    let attention = listed.iter().filter(|r| r.needs_attention()).count();
    let mut lines = vec![];
    for r in listed.iter().take(MAX_RUN_LINES) {
        let state = if !r.running() {
            "오류로 멈춤 · 확인 필요".to_string()
        } else if r.stopping {
            "종료 처리 중".to_string()
        } else {
            r.degraded().map_or_else(|| "실행 중".to_string(), |(what, _)| what)
        };
        lines.push((r.id.clone(), format!("{} · {} · {}", r.symbol(), r.mode.label(), state)));
    }
    let mut parts = vec![];
    if running > 0 {
        parts.push(format!("실행 중 {running}개"));
    }
    if attention > 0 {
        parts.push(format!("확인 필요 {attention}개"));
    }
    let head = if parts.is_empty() { "실행 중인 매크로 없음".to_string() } else { parts.join(" · ") };
    let tooltip = if parts.is_empty() { "껄무새 · 대기 중".to_string() } else { format!("껄무새 · {head}") };
    let key = format!("{head}|{lines:?}");
    TraySummary { key, lines, tooltip, head, total: listed.len() }
}

fn build_menu(app: &AppHandle, t: &TraySummary) -> tauri::Result<Menu<Wry>> {
    let menu = Menu::new(app)?;
    menu.append(&MenuItem::with_id(app, "tray_open", "창 열기", true, None::<&str>)?)?;
    menu.append(&PredefinedMenuItem::separator(app)?)?;
    menu.append(&MenuItem::with_id(app, "tray_head", &t.head, false, None::<&str>)?)?;
    for (id, text) in &t.lines {
        menu.append(&MenuItem::with_id(app, format!("tray_run:{id}"), text, true, None::<&str>)?)?;
    }
    if t.total > t.lines.len() {
        menu.append(&MenuItem::with_id(
            app,
            "tray_more",
            format!("외 {}개", t.total - t.lines.len()),
            false,
            None::<&str>,
        )?)?;
    }
    menu.append(&PredefinedMenuItem::separator(app)?)?;
    menu.append(&MenuItem::with_id(app, "tray_web", "껄무새 웹 열기", true, None::<&str>)?)?;
    menu.append(&MenuItem::with_id(app, "tray_settings", "설정…", true, None::<&str>)?)?;
    menu.append(&MenuItem::with_id(app, "tray_quit", "껄무새 종료", true, None::<&str>)?)?;
    Ok(menu)
}

impl Shared {
    pub fn create_tray(self: &S) -> tauri::Result<()> {
        let t = summary(self);
        let menu = build_menu(&self.app, &t)?;
        let me = self.clone();
        let me2 = self.clone();
        TrayIconBuilder::with_id(TRAY_ID)
            .icon(tray_image())
            .icon_as_template(cfg!(target_os = "macos"))
            .tooltip(&t.tooltip)
            .menu(&menu)
            // macOS는 메뉴 막대 아이콘을 누르면 메뉴가 열리는 것이 자연스러움. Windows는 왼쪽 클릭으로 창을 엶.
            .show_menu_on_left_click(cfg!(target_os = "macos"))
            .on_menu_event(move |_app, event| {
                let id = event.id().as_ref();
                match id {
                    "tray_open" => me.show_main_window(),
                    "tray_settings" => {
                        me.show_main_window();
                        me.navigate("settings");
                    }
                    "tray_web" => {
                        if let Err(e) = me.open_web() {
                            me.notice("warn", e);
                        }
                    }
                    "tray_quit" => me.quit_from_tray(),
                    _ => {
                        if let Some(run_id) = id.strip_prefix("tray_run:") {
                            me.show_main_window();
                            me.navigate(&format!("run:{run_id}"));
                        }
                    }
                }
            })
            .on_tray_icon_event(move |_tray, event| {
                if let TrayIconEvent::Click { button: MouseButton::Left, button_state: MouseButtonState::Up, .. } =
                    event
                {
                    if !cfg!(target_os = "macos") {
                        me2.show_main_window();
                    }
                }
            })
            .build(&self.app)?;
        *self.tray_key.lock().unwrap() = t.key;
        Ok(())
    }

    /// 실행 상태가 바뀌었으면 트레이 메뉴·툴팁을 다시 그림(같으면 그대로 — 열린 메뉴를 흔들지 않음).
    /// 메뉴·트레이 호출은 메인 스레드에서만 함. 다른 스레드에서 부르면 Tauri가 메인 스레드를 기다리며 막히는데,
    /// 그동안 메인 스레드가 같은 잠금을 기다리면 앱 전체가 멈춤 — 그래서 막히지 않는 run_on_main_thread로 넘김.
    pub fn refresh_tray(&self) {
        let app = self.app.clone();
        let _ = self.app.run_on_main_thread(move || {
            if let Some(s) = app.try_state::<S>() {
                s.refresh_tray_now();
            }
        });
    }

    fn refresh_tray_now(&self) {
        let Some(tray) = self.app.tray_by_id(TRAY_ID) else { return };
        let t = summary(self);
        let _ = tray.set_tooltip(Some(&t.tooltip));
        let mut last = self.tray_key.lock().unwrap();
        if *last == t.key {
            return;
        }
        if let Ok(menu) = build_menu(&self.app, &t) {
            let _ = tray.set_menu(Some(menu));
            *last = t.key;
        }
    }

    /// 창을 다시 보이고 앞으로. macOS는 Dock 아이콘도 되살림.
    pub fn show_main_window(&self) {
        #[cfg(target_os = "macos")]
        {
            let _ = self.app.set_dock_visibility(true);
        }
        if let Some(w) = self.app.get_webview_window("main") {
            let _ = w.show();
            let _ = w.unminimize();
            let _ = w.set_focus();
        }
        self.lock().hidden_to_tray = false;
    }

    /// 창을 숨기고 트레이로. macOS는 Dock 아이콘을, Windows는 작업 표시줄 단추를 없앰.
    pub fn hide_to_tray(&self) {
        if let Some(w) = self.app.get_webview_window("main") {
            let _ = w.hide();
        }
        #[cfg(target_os = "macos")]
        {
            let _ = self.app.set_dock_visibility(false);
        }
        self.lock().hidden_to_tray = true;
    }

    /// 트레이의 [종료] — 창 닫기와 같은 규칙(실행 중이면 끄지 않고 창을 보여 안내함).
    pub fn quit_from_tray(self: &S) {
        if self.lock().busy_count() > 0 {
            self.show_main_window();
            let _ = self.request_close();
            return;
        }
        let me = self.clone();
        tauri::async_runtime::spawn(async move {
            me.shutdown().await;
            me.app.exit(0);
        });
    }

    /// 창 닫기(빨간 단추·Alt+F4). 설정이 켜져 있으면 트레이로 숨기고, 꺼져 있으면 원래 규칙대로 끔.
    pub fn on_close_requested(self: &S) -> CloseAction {
        if self.lock().close_to_tray {
            self.hide_to_tray();
            return CloseAction::Hidden;
        }
        if self.request_close() {
            CloseAction::Quit
        } else {
            CloseAction::Refused
        }
    }
}

pub enum CloseAction {
    Hidden,
    Quit,
    Refused,
}
