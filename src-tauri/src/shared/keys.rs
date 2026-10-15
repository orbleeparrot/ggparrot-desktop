//! 키 — 키체인에 기억한 키, 설정의 거래소 키 칸·회원 키.

use super::*;

impl Shared {
    pub(crate) fn load_remembered(&self) {
        let mut c = self.lock();
        if !c.remember_supported {
            return;
        }
        let loaded = credentials::load_result(self.store.as_ref());
        c.load_status = loaded.status;
        if let Some(values) = loaded.values {
            Self::apply_remembered(&mut c, &values);
        }
    }

    pub(crate) fn apply_remembered(c: &mut Core, remembered: &Credentials) {
        c.remembered = Some(remembered.clone());
        for slot in KeySlot::ALL {
            let pair = remembered.exchanges.get(slot.as_str()).cloned();
            let complete = pair.as_ref().is_some_and(|p| !p.api_key.is_empty() && !p.api_secret.is_empty());
            let st = c.slot_mut(slot);
            if let Some(p) = pair {
                st.keys = p;
            }
            st.remember = complete;
            st.dirty = false;
        }
        if !remembered.member_key.is_empty() && c.member_key.trim().is_empty() {
            c.member_key = remembered.member_key.clone();
        }
    }

    /// 이 칸만 저장·삭제함. 다른 칸과 회원 키는 보존함.
    pub(crate) fn persist_slot(&self, c: &mut Core, slot: KeySlot) -> bool {
        if !c.remember_supported {
            return false;
        }
        let choice = c.slot(slot).remember;
        let values = c.storage_values();
        match credentials::apply_exchange_choice(self.store.as_ref(), slot, choice, &values) {
            Ok(saved) => {
                c.load_status = if saved.is_some() { LoadStatus::Loaded } else { LoadStatus::Missing };
                c.remembered = saved;
                c.slot_mut(slot).dirty = false;
                c.storage_error.clear();
                let note = format!(
                    "{} 키를 {}. 다른 키와 회원 키는 그대로예요. 키는 서버로 보내지 않아요.",
                    slot.label(),
                    if choice { "이 기기에 저장했어요" } else { "이 기기에서 지웠어요" }
                );
                self.notice("info", note);
                true
            }
            Err(e) => {
                // 권한 안내는 키체인에 쓰지 못한 경우에만 덧붙임(길이 초과·입력 부족에는 맞지 않음).
                let hint = if e == credentials::SAVE_FAILED || e == credentials::CLEAR_FAILED {
                    format!(" {} 접근을 허용했는지 확인하세요.", credentials::KEY_STORE_NAME)
                } else {
                    String::new()
                };
                c.storage_error = format!("{e}{hint} 지금 입력한 키는 그대로 남아요.");
                self.notice("warn", c.storage_error.clone());
                false
            }
        }
    }

    /// 실거래 연결이 확인된 국내 키라면 이번 공인 IP 관측을 저장 항목에 남김(등록 IP 증거가 아님).
    pub(crate) fn record_ip_history(&self, c: &mut Core, slot: KeySlot, preflight: &PreflightResult) {
        if !c.remember_supported || !slot.exchange().is_domestic() || !c.slot(slot).remember {
            return;
        }
        let (Some(remembered), Some(ip)) = (c.remembered.clone(), c.ip_cache.result().cloned()) else { return };
        let m = slot.probe_macro();
        let current = c.slot_credentials(slot);
        let mut remembered_view = remembered.clone();
        if let Some(p) = remembered.exchanges.get(slot.as_str()) {
            remembered_view.exchanges.insert(slot.exchange().as_str().into(), p.clone());
        }
        let Some(updated) =
            connection::with_successful_ip_history(&remembered_view, &m, RunMode::Live, &current, &ip, preflight)
        else {
            return;
        };
        let mut next = remembered.clone();
        next.connection_history = updated.connection_history;
        if credentials::save(self.store.as_ref(), &next).is_ok() {
            c.remembered = Some(next);
        }
    }

    /// 키체인을 다시 읽음(접근을 허용했거나 잠금을 푼 뒤). 지우지 않음.
    pub fn reload_storage(self: &S) -> Result<(), String> {
        self.load_remembered();
        let failed = self.lock().load_status == LoadStatus::ReadError;
        self.emit();
        if failed {
            return Err(format!(
                "아직 {store}에서 키를 읽지 못했어요. {store} 잠금을 풀고 접근을 허용한 뒤 다시 누르세요.",
                store = credentials::KEY_STORE_NAME
            ));
        }
        Ok(())
    }

    pub fn recover_storage(self: &S) -> Result<(), String> {
        {
            let mut c = self.lock();
            credentials::reset_unreadable(self.store.as_ref())?;
            c.remembered = None;
            c.load_status = LoadStatus::Missing;
            let slots: Vec<KeySlot> =
                KeySlot::ALL.into_iter().filter(|s| c.slot(*s).remember && c.slot_pair(*s).is_some()).collect();
            for s in slots {
                self.persist_slot(&mut c, s);
            }
        }
        self.notice("info", "읽지 못한 저장 항목을 지웠어요. '이 기기에 키 기억하기'를 켠 키는 새로 저장했어요.");
        self.emit();
        Ok(())
    }

    // --- 설정: 키 칸·회원 키 --------------------------------------------------------------
    pub fn set_slot_key(self: &S, slot: KeySlot, field: &str, value: String) {
        if !self.key_length_ok(&value) {
            return;
        }
        {
            let mut c = self.lock();
            let st = c.slot_mut(slot);
            match field {
                "access" => st.keys.api_key = value,
                "secret" => st.keys.api_secret = value,
                _ => return,
            }
            Self::slot_changed(&mut c, slot);
        }
        self.emit();
        self.schedule_slot_check(slot, 700);
    }

    /// 저장본과 다르면 '변경됨', 이 칸을 쓰는 슬롯의 시작 전 검사는 무효.
    pub fn slot_changed(c: &mut Core, slot: KeySlot) {
        let cur = c.slot(slot).keys.clone();
        let cur = KeyPair { api_key: cur.api_key.trim().into(), api_secret: cur.api_secret.trim().into() };
        let saved = c.remembered.as_ref().and_then(|r| r.exchanges.get(slot.as_str()).cloned()).unwrap_or_default();
        c.slot_mut(slot).dirty = cur != saved;
        for r in c.runs.iter_mut().filter(|r| !r.running() && r.key_slot() == Some(slot)) {
            r.generation += 1;
            r.preflight = None;
        }
    }

    pub fn reveal_slot_key(&self, slot: KeySlot, field: &str) -> String {
        let c = self.lock();
        let k = &c.slot(slot).keys;
        if field == "access" {
            k.api_key.clone()
        } else {
            k.api_secret.clone()
        }
    }

    pub fn set_member_key(self: &S, value: String) {
        if !self.key_length_ok(&value) {
            return;
        }
        {
            let mut c = self.lock();
            c.member_key = value;
            for r in c.runs.iter_mut().filter(|r| !r.running()) {
                r.generation += 1;
                r.preflight = None;
            }
            self.persist_member_key(&mut c);
        }
        self.emit();
    }

    /// 잘못 붙여 넣은 긴 글(인증서·JSON 등)은 키 칸에 넣지 않음.
    pub(crate) fn key_length_ok(&self, value: &str) -> bool {
        if value.trim().chars().count() > credentials::MAX_KEY_CHARS {
            self.notice(
                "warn",
                format!(
                    "키가 너무 길어요(최대 {}자). 거래소나 웹에서 복사한 키가 맞는지 확인하세요.",
                    credentials::MAX_KEY_CHARS
                ),
            );
            return false;
        }
        true
    }

    /// 기억해 둔 키가 있으면 회원 키도 함께 저장함. 같으면 키체인을 건드리지 않고(입력할 때마다 묻지 않게),
    /// 실패하면 조용히 넘기지 않고 알림(다음 실행에 옛 회원 키가 돌아오지 않게).
    pub(crate) fn persist_member_key(&self, c: &mut Core) {
        if !c.remember_supported {
            return;
        }
        let Some(saved) = c.remembered.as_ref() else { return };
        let key = c.member_key.trim().to_string();
        if saved.member_key == key {
            return;
        }
        match credentials::update_member_key(self.store.as_ref(), &key) {
            Ok(Some(values)) => {
                c.remembered = Some(values);
                c.storage_error.clear();
            }
            Ok(None) => {}
            Err(e) => {
                c.storage_error = format!("회원 키를 기억해 두지 못했어요. {e}");
                self.notice("warn", c.storage_error.clone());
            }
        }
    }

    /// 웹 연결이 가져온 다른 회원 키를 쓸지 정함.
    pub fn resolve_member_key(self: &S, accept: bool) {
        let incoming = self.lock().pending_member_key.take();
        match incoming {
            Some(key) if accept => self.set_member_key(key),
            _ => self.emit(),
        }
    }

    pub fn reveal_member_key(&self) -> String {
        self.lock().member_key.clone()
    }

    /// '이 기기에 키 기억하기' — 켜면 바로 저장(키가 다 찼을 때), 끄면 이 칸만 지움.
    pub fn set_slot_remember(self: &S, slot: KeySlot, remember: bool) {
        {
            let mut c = self.lock();
            if !c.remember_supported {
                return;
            }
            c.slot_mut(slot).remember = remember;
            if !(remember && c.slot_pair(slot).is_none()) {
                self.persist_slot(&mut c, slot);
            }
        }
        self.emit();
    }

    pub fn set_close_to_tray(self: &S, value: bool) {
        {
            let mut c = self.lock();
            c.close_to_tray = value;
            self.save_prefs(&c);
        }
        self.emit();
    }

    pub fn set_show_dev_log(self: &S, show: bool) {
        {
            let mut c = self.lock();
            c.show_dev_log = show;
            self.save_prefs(&c);
        }
        self.emit();
    }
}
