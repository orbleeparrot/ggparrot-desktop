//! 연결 도우미 상태기계 — 키 발급·붙여 넣기·연결 확인 단계. 매매 시작 권한은 없음.
//!
//! - 국내(업비트·빗썸): 허용 IP 등록 → 권한 선택·발급 → 키 복사.
//! - 바이낸스: 실행 모드·시장으로 고른 사이트(`connection::binance_env`)에서 키 만들기 → 키 복사,
//!   실거래만 그 뒤 허용 IP·주문 권한(Edit restrictions).

use runner_core::connection;

use crate::shared::{Shared, S};
use crate::state::{Wizard, WizardTarget, WATCH_STAGES};

mod clipboard;
mod flow;
use flow::{back_stage, flow, stage_allowed};

impl Shared {
    pub fn wizard_open(self: &S, target: WizardTarget, stage: &str) {
        let slot = target.slot;
        let stage = if stage_allowed(target.env, stage) { stage } else { "ask" };
        let clip = self.clipboard_text();
        let recheck = {
            let mut c = self.lock();
            if c.claim_busy {
                drop(c);
                self.notice(
                    "warn",
                    "웹에서 매크로를 연결하는 동안에는 연결 도우미를 열 수 없어요. 잠시 뒤 다시 시도하세요.",
                );
                return;
            }
            if let Some(w) = c.wizard.as_ref() {
                let same = w.slot == slot && w.env == target.env && w.market == target.market;
                drop(c);
                if same {
                    if stage != "ask" {
                        self.wizard_show(stage);
                    }
                    return;
                }
                self.wizard_close();
                c = self.lock();
            }
            c.wizard_generation += 1;
            let generation = c.wizard_generation;
            let before = c.slot_check_snapshot(slot).3;
            c.wizard = Some(Wizard {
                slot,
                env: target.env,
                market: target.market,
                stage: String::new(),
                path: if flow(target.env).contains(&stage) { "new".into() } else { String::new() },
                got_access: false,
                got_secret: false,
                advance_on_return: String::new(),
                clip_seen: clip,
                generation,
            });
            if let Some(h) = c.remembered.as_ref().and_then(|r| r.connection_history.get(slot.as_str())).cloned() {
                c.ip_cache.seed_previous(Some(&h));
            }
            // 바이낸스 실거래 칸은 열린 도우미의 시장으로 확인함(`Core::probe_market`) — 시장이 바뀌었으면 다시 확인.
            let changed = c.slot_check_snapshot(slot).3 != before;
            self.spawn_clipboard_watch(generation);
            changed && c.slot_pair(slot).is_some()
        };
        self.wizard_show(stage);
        if recheck {
            self.schedule_slot_check(slot, 0);
        }
        // 사용자가 연 도우미가 이 기기 주소를 조회하는 유일한 계기임. 허용 IP가 없는 곳(테스트넷·데모)은 조회하지 않음.
        if flow(target.env).contains(&"ip") {
            self.begin_public_ip(false);
        }
    }

    pub fn wizard_close(self: &S) {
        let recheck = {
            let mut c = self.lock();
            let Some(slot) = c.wizard.as_ref().map(|w| w.slot) else { return };
            let before = c.slot_check_snapshot(slot).3;
            c.wizard = None;
            c.wizard_generation += 1;
            // 도우미가 보던 시장에서 실행 목록의 시장으로 돌아가면 그 시장으로 다시 확인.
            (c.slot_check_snapshot(slot).3 != before && c.slot_pair(slot).is_some()).then_some(slot)
        };
        self.emit();
        if let Some(slot) = recheck {
            self.schedule_slot_check(slot, 700);
        }
    }

    fn key_stage(path: &str) -> &'static str {
        if path == "existing" {
            "existing"
        } else {
            "keys"
        }
    }

    pub fn wizard_show(self: &S, stage: &str) {
        let clip = if WATCH_STAGES.contains(&stage) { Some(self.clipboard_text()) } else { None };
        let need_ip = {
            let mut c = self.lock();
            let Some(w) = c.wizard.as_mut() else { return };
            if !stage_allowed(w.env, stage) {
                return;
            }
            w.stage = stage.to_string();
            match stage {
                "ask" => w.path.clear(),
                "existing" => w.path = "existing".into(),
                s if flow(w.env).contains(&s) => w.path = "new".into(),
                _ => {}
            }
            if let Some(text) = clip {
                // 단계에 들어설 때 클립보드에 있던 값은 이번에 복사한 키가 아님.
                if stage != "keys" || !(w.got_access || w.got_secret) {
                    w.clip_seen = text;
                }
            }
            stage == "ip" && !c.ip_cache.busy() && !c.ip_cache.result().is_some_and(|r| r.fresh())
        };
        self.emit();
        if need_ip {
            self.begin_public_ip(false);
        }
    }

    pub fn wizard_choose(self: &S, path: &str) {
        if path == "existing" {
            self.wizard_show("existing");
            // 이미 발급한 키는 거래소 API 관리에 있음 — 바로 엶.
            self.wizard_open_page(true);
        } else {
            let env = self.lock().wizard.as_ref().map(|w| w.env).unwrap_or("");
            self.wizard_show(flow(env)[0]);
        }
    }

    pub fn wizard_back(self: &S) {
        let target = {
            let c = self.lock();
            let Some(w) = c.wizard.as_ref() else { return };
            back_stage(w.env, &w.stage, &w.path)
        };
        match target {
            Some(stage) => self.wizard_show(stage),
            None => self.wizard_close(),
        }
    }

    /// '검사' — 확인 화면으로 가고 지금 키로 바로 확인함.
    pub fn wizard_start_check(self: &S) {
        let slot = match self.lock().wizard.as_ref() {
            Some(w) => w.slot,
            None => return,
        };
        self.wizard_show("checking");
        if !self.begin_slot_check(slot, true) {
            self.refresh_wizard();
        }
    }

    pub fn wizard_go(self: &S, stage: &str) {
        if stage == "checking" {
            self.wizard_start_check();
        } else {
            self.wizard_show(stage);
        }
    }

    pub fn wizard_recopy(self: &S) {
        let path = {
            let mut c = self.lock();
            let Some(w) = c.wizard.as_mut() else { return };
            w.got_access = false;
            w.got_secret = false;
            w.path.clone()
        };
        self.wizard_show(Self::key_stage(&path));
    }

    /// 앱 상태(IP·연결 검사·키 칸)가 바뀌었을 때 지금 화면을 맞춤.
    pub fn refresh_wizard(self: &S) {
        let (stage, check_state, connected, need_ip) = {
            let c = self.lock();
            let Some(w) = c.wizard.as_ref() else { return };
            let (state, view, _) = c.wizard_check(w.slot);
            let need_ip =
                w.stage == "ip" && !c.ip_cache.busy() && c.ip_cache.result().is_some_and(|r| r.ok && !r.fresh());
            (w.stage.clone(), state, view.connected, need_ip)
        };
        if stage == "checking" {
            match check_state {
                "done" => self.wizard_show(if connected { "result_ok" } else { "result_fail" }),
                "idle" => self.wizard_show("result_fail"),
                _ => {}
            }
        }
        if need_ip {
            // 120초가 지난 주소는 보이지 않음 — 이 단계에 있으면 저절로 다시 봄.
            self.begin_public_ip(false);
        }
    }

    /// 거래소 브라우저에서 이 창으로 돌아옴.
    pub fn wizard_on_return(self: &S) {
        let (stage, target, ready) = {
            let mut c = self.lock();
            let Some(w) = c.wizard.as_mut() else { return };
            let (stage, target, slot) = (w.stage.clone(), std::mem::take(&mut w.advance_on_return), w.slot);
            (stage, target, c.slot_pair(slot).is_some())
        };
        if stage == "result_fail" {
            self.wizard_start_check();
        } else if target == "check" && stage == "ip" {
            // 바이낸스 실거래: 허용 IP·주문 권한을 걸고 돌아옴 — 키가 다 있으면 바로 확인함.
            if ready {
                self.wizard_start_check();
            }
        } else if !target.is_empty() && stage == "ip" {
            self.wizard_show(&target);
        }
    }

    /// 공식 허용 목록 주소만 엶(api=true면 API 관리, 아니면 거래소 홈). 바이낸스는 도우미의 사이트(env)로.
    pub fn wizard_open_page(self: &S, api: bool) -> bool {
        let (ex, env) = match self.lock().wizard.as_ref() {
            Some(w) => (w.slot.exchange(), w.env),
            None => return false,
        };
        let url = if api {
            connection::api_management_url(ex.as_str(), env)
        } else {
            connection::exchange_home_url(ex.as_str(), env)
        };
        let opened = url.map(|u| self.open_url(u)).unwrap_or(false);
        if !opened {
            self.notice(
                "warn",
                "기본 브라우저를 열지 못했어요. 거래소 공식 홈페이지의 API 관리 메뉴에서 직접 진행하세요.",
            );
        }
        opened
    }

    pub fn wizard_copy_ip(self: &S) -> bool {
        let ok = self.copy_public_ip();
        if ok {
            self.notice("info", "이 기기 주소를 복사했어요.");
        } else {
            self.notice("warn", "주소를 복사하지 못했어요. [다시 확인]으로 주소를 새로 받은 뒤 복사하세요.");
        }
        ok
    }

    /// 주소 복사와 공식 API 관리 열기를 한 번에. 거래소에서 등록하고 돌아오면 다음 단계로
    /// (바이낸스 실거래는 허용 IP가 마지막 단계라 돌아오면 바로 확인).
    pub fn wizard_copy_and_open(self: &S) {
        let (stage, env) = self.lock().wizard.as_ref().map(|w| (w.stage.clone(), w.env)).unwrap_or((String::new(), ""));
        let copied = self.copy_public_ip();
        if stage == "result_fail" {
            self.wizard_open_page(true);
            return;
        }
        if !copied {
            self.notice("warn", "주소를 복사하지 못했어요. [다시 확인]으로 주소를 새로 받은 뒤 다시 누르세요.");
            return;
        }
        if self.wizard_open_page(true) {
            if let Some(w) = self.lock().wizard.as_mut() {
                w.advance_on_return = if env.is_empty() { "perm" } else { "check" }.into();
            }
        }
    }

    pub fn wizard_set_key(self: &S, field: &str, value: String) {
        let slot = match self.lock().wizard.as_ref() {
            Some(w) => w.slot,
            None => return,
        };
        self.set_slot_key(slot, field, value);
    }

    pub fn wizard_set_remember(self: &S, remember: bool) {
        let slot = match self.lock().wizard.as_ref() {
            Some(w) => w.slot,
            None => return,
        };
        self.set_slot_remember(slot, remember);
    }

    /// 거래소 발급 창에서 복사한 키 하나. 들어온 순서대로 Access → Secret 칸에 넣음.
    /// 복사한 키를 칸에 넣음. `as_secret`이 있으면(빗썸처럼 모양으로 가릴 수 있으면) 그 칸에, 없으면 복사한
    /// 순서대로(처음 Access, 다음 Secret).
    pub fn wizard_accept_key(self: &S, key: String, as_secret: Option<bool>) {
        let (stage, both, slot, following) = {
            let mut c = self.lock();
            let Some(w) = c.wizard.as_mut() else { return };
            let slot = w.slot;
            if w.got_access && w.got_secret {
                // 다 받은 뒤 또 들어오면 새로 발급한 키로 보고 처음부터 받음.
                w.got_access = false;
                w.got_secret = false;
            }
            let stage = w.stage.clone();
            let secret = as_secret.unwrap_or(w.got_access);
            if !secret {
                let had_secret = w.got_secret;
                w.got_access = true;
                let keys = &mut c.slot_mut(slot).keys;
                if !had_secret {
                    keys.api_secret.clear(); // 옛 시크릿과 새 Access가 짝지어 검사되지 않게
                }
                keys.api_key = key;
            } else {
                let first = !w.got_access;
                if key == c.slot(slot).keys.api_key.trim() {
                    return;
                }
                if let Some(w) = c.wizard.as_mut() {
                    w.got_secret = true;
                }
                let keys = &mut c.slot_mut(slot).keys;
                if first {
                    keys.api_key.clear(); // 시크릿을 먼저 복사함 — 옛 Access와 짝지어 검사되지 않게
                }
                keys.api_secret = key;
            }
            Self::slot_changed(&mut c, slot);
            let both = c.wizard.as_ref().is_some_and(|w| w.got_access && w.got_secret);
            // 처음 발급 길에서 키 복사 뒤에도 단계가 남았으면(바이낸스 실거래의 허용 IP·주문 권한) 그리로 감.
            let following = c
                .wizard
                .as_ref()
                .filter(|w| w.path == "new")
                .and_then(|w| flow(w.env).iter().skip_while(|s| **s != "keys").nth(1).copied());
            (stage, both, slot, following)
        };
        if stage == "perm" {
            self.wizard_show("keys");
        } else {
            self.emit();
        }
        if both {
            // 시크릿이 클립보드에 남지 않게 비움(OS 클립보드 기록은 앱이 지우지 못함).
            // 그사이 사용자가 다른 것을 복사했으면 건드리지 않음.
            let secret = self.lock().slot(slot).keys.api_secret.trim().to_string();
            if !secret.is_empty() && self.clipboard_text().trim() == secret {
                self.write_clipboard("");
            }
            if let Some(w) = self.lock().wizard.as_mut() {
                w.clip_seen.clear();
            }
            match following {
                // 키를 받았으면 그 키에 허용 IP·주문 권한을 걸러 감. 확인은 그 뒤에.
                Some(next) => self.wizard_show(next),
                None => self.wizard_start_check(),
            }
        } else {
            self.schedule_slot_check(slot, 700);
        }
    }
}
