//! 키 단계의 클립보드 받기 — 거래소에서 복사한 키를 칸에 넣음.

use runner_core::connection::{clipboard_key, clipboard_key_is_secret};
use std::time::Duration;

use crate::shared::{Shared, S};
use crate::state::WATCH_STAGES;

const CLIPBOARD_MS: u64 = 400;

impl Shared {
    /// 키 단계에서 400ms마다 클립보드를 봄. 도우미가 열린 뒤 새로 복사한, 키처럼 생긴 값만 받음.
    pub(super) fn spawn_clipboard_watch(self: &S, generation: u64) {
        let me = self.clone();
        tauri::async_runtime::spawn(async move {
            loop {
                tokio::time::sleep(Duration::from_millis(CLIPBOARD_MS)).await;
                // 창을 트레이로 숨겨 둔 동안은 클립보드를 보지 않음(도우미를 잊은 채 복사한 다른 값을 받지 않게).
                let (watching, exchange) = {
                    let c = me.lock();
                    match c.wizard.as_ref() {
                        Some(w) if w.generation == generation => (
                            WATCH_STAGES.contains(&w.stage.as_str()) && !c.hidden_to_tray,
                            w.slot().exchange().as_str(),
                        ),
                        _ => return,
                    }
                };
                if !watching {
                    continue;
                }
                let text = me.clipboard_text();
                let changed = {
                    let mut c = me.lock();
                    match c.wizard.as_mut() {
                        Some(w) if w.generation == generation && w.clip_seen != text => {
                            w.clip_seen = text.clone();
                            true
                        }
                        Some(w) if w.generation == generation => false,
                        _ => return,
                    }
                };
                if changed {
                    let key = clipboard_key(exchange, &text);
                    if !key.is_empty() {
                        let role = clipboard_key_is_secret(exchange, &key);
                        me.wizard_accept_key(key, role);
                    }
                }
            }
        });
    }
}
