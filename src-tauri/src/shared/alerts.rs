//! OS 알림 — 창이 앞에 없을 때 위험한 상황을 알림. 같은 원인으로 여러 실행이 같은 알림을 띄우지 않게 묶음.
//! 진입 체결 알림('사면 소리 · 알림으로 알려 주기')은 창이 앞에 있어도 소리와 함께 띄움(Windows 오른쪽 아래 토스트).

use super::*;

/// 제목 하나의 OS 알림 묶음. 첫 알림은 바로, 1분 안에 다른 실행에서 온 같은 알림은 모아서 한 번 더.
#[derive(Default)]
pub struct AlertSlot {
    /// 마지막으로 띄운 시각(ms).
    pub at: i64,
    /// 이 묶음에서 이미 알린 실행.
    pub shown: std::collections::HashSet<String>,
    /// 모아 둔 (실행, 종목 이름) — 묶음 시간이 지나면 '외 N개'로 띄움.
    pub pending: Vec<(String, String)>,
}

/// 같은 제목의 OS 알림을 묶는 시간.
const ALERT_WINDOW_MS: i64 = 60_000;

/// 시스템 기본 알림음 이름 — Windows 토스트는 "Default"(ms-winsoundevent:Notification.Default), macOS 는
/// NSUserNotificationDefaultSoundName. 그 밖(리눅스 개발 빌드)은 소리 이름 힌트만 보냄.
#[cfg(target_os = "macos")]
const NOTIFY_SOUND: &str = "NSUserNotificationDefaultSoundName";
#[cfg(not(target_os = "macos"))]
const NOTIFY_SOUND: &str = "Default";

impl Shared {
    /// 창이 앞에 없을 때 OS 알림을 띄움. 같은 실행·같은 제목은 1분에 한 번, 다른 실행의 같은 제목(서버 끊김처럼
    /// 원인이 하나인 경우)은 1분 동안 모아 '외 N개' 한 번으로.
    pub(crate) fn alert_os(self: &S, run_id: &str, title: &str, body: &str) {
        let watching = self
            .app
            .get_webview_window("main")
            .is_some_and(|w| w.is_visible().unwrap_or(false) && w.is_focused().unwrap_or(false));
        if watching {
            return;
        }
        // 트레이에 숨겨 둔 사람이 모의인지 알 수 있게 실전이 아니면 모드를 붙임.
        let symbol = self
            .lock()
            .run(run_id)
            .map(|r| if r.mode == RunMode::Live { r.symbol() } else { format!("{} · {}", r.symbol(), r.mode.label()) })
            .unwrap_or_default();
        let now = now_ms();
        let flush_in = {
            let mut alerts = lock_or_recover(&self.os_alerts);
            let slot = alerts.entry(title.to_string()).or_default();
            if now - slot.at >= ALERT_WINDOW_MS {
                *slot = AlertSlot { at: now, shown: [run_id.to_string()].into(), pending: vec![] };
                None
            } else if slot.shown.contains(run_id) || slot.pending.iter().any(|(id, _)| id == run_id) {
                return;
            } else {
                slot.pending.push((run_id.to_string(), symbol.clone()));
                // 처음 모은 것만 내보내기를 예약함(나머지는 같은 묶음에 얹힘).
                (slot.pending.len() == 1).then_some(ALERT_WINDOW_MS - (now - slot.at))
            }
        };
        match flush_in {
            None => {
                let head = if symbol.is_empty() { title.to_string() } else { format!("{symbol} · {title}") };
                self.show_os_notification(&head, body, false);
            }
            Some(wait) => {
                let (me, title, body) = (self.clone(), title.to_string(), body.to_string());
                tauri::async_runtime::spawn(async move {
                    tokio::time::sleep(std::time::Duration::from_millis(wait.max(0) as u64)).await;
                    let names = {
                        let mut alerts = lock_or_recover(&me.os_alerts);
                        let Some(slot) = alerts.get_mut(&title) else { return };
                        let pending = std::mem::take(&mut slot.pending);
                        slot.at = now_ms();
                        slot.shown.extend(pending.iter().map(|(id, _)| id.clone()));
                        pending.into_iter().map(|(_, name)| name).collect::<Vec<_>>()
                    };
                    if let Some(first) = names.first() {
                        let more = if names.len() > 1 { format!(" 외 {}개", names.len() - 1) } else { String::new() };
                        me.show_os_notification(&format!("{first}{more} · {title}"), &body, false);
                    }
                });
            }
        }
    }

    /// 기다리던 진입이 체결됨 — 창이 앞에 있어도(실행기를 보고 있어도) 소리와 함께 OS 알림. 묶지 않음(실행마다 다른 체결).
    /// 끄는 곳은 설정 › 창 · 화면 '매수 체결 알림' 하나(소리만 끄려면 OS 알림 설정에서).
    pub(crate) fn alert_entry_fill(self: &S, run_id: &str, fill: &str) {
        let (symbol, short) = {
            let c = self.lock();
            let Some(r) = c.run(run_id) else { return };
            let symbol =
                if r.mode == RunMode::Live { r.symbol() } else { format!("{} · {}", r.symbol(), r.mode.label()) };
            (symbol, macro_side(&r.macro_) == runner_core::types::Side::Short)
        };
        let title = if short {
            "기다리던 조건이 와서 공매도했어요"
        } else {
            "기다리던 조건이 와서 샀어요"
        };
        self.show_os_notification(&format!("{symbol} · {title}"), fill, true);
    }

    /// OS 알림. `sound` 면 시스템 기본 알림음(Windows 토스트 'Default' · macOS 기본 소리) — 아니면 Windows 토스트는 소리 없음.
    fn show_os_notification(&self, title: &str, body: &str, sound: bool) {
        use tauri_plugin_notification::NotificationExt;
        let mut n = self.app.notification().builder().title(title).body(body);
        if sound {
            n = n.sound(NOTIFY_SOUND);
        }
        if let Err(e) = n.show() {
            eprintln!("os notification failed: {e}");
        }
    }
}
