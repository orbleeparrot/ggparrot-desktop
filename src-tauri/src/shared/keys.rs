//! 키 — 키체인에 기억한 키, 설정의 거래소 키 칸·회원 키.
//!
//! 키체인 읽기·쓰기는 Core 잠금 밖에서 함 — macOS 키체인 허용 창이 떠 있는 동안에도 메인 스레드(트레이 등)가
//! Core를 기다리며 멈추지 않게.
//! - 순서: 잠금 안에서 할 일 정하기 → 잠금 밖에서 키체인 → 잠금 안에서 결과 반영.
//! - 반영할 때 그사이 사용자가 고친 칸은 덮지 않고 '변경됨'을 다시 셈.
//! - 키체인 일끼리는 `key_io` 잠금으로 한 번에 하나씩만 함(읽고-고쳐-쓰기가 서로 덮지 않게).
//! - 잠금 순서는 늘 `key_io` → Core. Core를 든 채 `key_io`를 기다리지 않음.
//! - 알림은 잠금을 다 푼 뒤 띄움.

use super::*;
use std::collections::BTreeMap;

/// 키체인 일이 끝난 뒤 띄울 알림(수준, 글).
pub(crate) type Notes = Vec<(&'static str, String)>;

/// 키체인 일 — Core 잠금·키체인·키체인 순서 잠금만 씀(앱 핸들 없이 시험할 수 있게).
pub(crate) struct Vault<'a> {
    pub core: &'a Mutex<Core>,
    pub store: &'a dyn SecretStore,
    pub io: &'a Mutex<()>,
}

/// '연결됨' 확인 뒤 키체인에 남길 일 — Core 잠금 안에서 정하고 잠금 밖에서 함([`Vault::after_connected`]).
pub(crate) struct ConnectedSlot {
    pub slot: KeySlot,
    /// 실거래 기준 지문으로 다시 쓴 확인 결과(공인 IP 기록용).
    pub checked: PreflightResult,
}

impl Vault<'_> {
    fn core(&self) -> MutexGuard<'_, Core> {
        lock_or_recover(self.core)
    }

    fn serial(&self) -> MutexGuard<'_, ()> {
        lock_or_recover(self.io)
    }

    /// 기억한 키를 읽어 칸에 채움. 읽는 동안 사용자가 고친 칸은 덮지 않음.
    pub fn load(&self) {
        let _io = self.serial();
        let before = {
            let c = self.core();
            if !c.remember_supported {
                return;
            }
            key_snapshot(&c)
        };
        let loaded = credentials::load_result(self.store);
        let mut c = self.core();
        c.load_status = loaded.status;
        if let Some(values) = loaded.values {
            apply_remembered(&mut c, &values, &before);
        }
    }

    /// 이 칸만 저장·삭제함. 다른 칸과 회원 키는 보존함.
    pub fn persist_slot(&self, slot: KeySlot) -> Notes {
        let _io = self.serial();
        self.persist_slot_held(slot)
    }

    /// `key_io`를 든 채로 부름.
    fn persist_slot_held(&self, slot: KeySlot) -> Notes {
        let (choice, values, keys) = {
            let c = self.core();
            if !c.remember_supported {
                return vec![];
            }
            (c.slot(slot).remember, c.storage_values(), c.slot(slot).keys.clone())
        };
        let result = credentials::apply_exchange_choice(self.store, slot, choice, &values);
        let mut c = self.core();
        match result {
            Ok(saved) => {
                c.load_status = if saved.is_some() { LoadStatus::Loaded } else { LoadStatus::Missing };
                c.remembered = saved;
                c.storage_error.clear();
                // 저장하는 동안 키를 고쳤으면 고친 키는 아직 저장되지 않음 — '변경됨'을 다시 셈.
                if c.slot(slot).keys == keys {
                    c.slot_mut(slot).dirty = false;
                } else {
                    refresh_dirty(&mut c, slot);
                }
                let note = format!(
                    "{} 키를 {}. 다른 키와 회원 키는 그대로예요. 키는 서버로 보내지 않아요.",
                    slot.label(),
                    if choice { "이 기기에 저장했어요" } else { "이 기기에서 지웠어요" }
                );
                vec![("info", note)]
            }
            Err(e) => {
                // 권한 안내는 키체인에 쓰지 못한 경우에만 덧붙임(길이 초과·입력 부족에는 맞지 않음).
                let hint = if e == credentials::SAVE_FAILED || e == credentials::CLEAR_FAILED {
                    format!(" {} 접근을 허용했는지 확인하세요.", credentials::KEY_STORE_NAME)
                } else {
                    String::new()
                };
                c.storage_error = format!("{e}{hint} 지금 입력한 키는 그대로 남아요.");
                vec![("warn", c.storage_error.clone())]
            }
        }
    }

    /// '연결됨' 확인 뒤 — 기억하기를 켠 칸의 바뀐 키를 저장하고, 이번 공인 IP 관측을 남김.
    pub fn after_connected(&self, done: &ConnectedSlot) -> Notes {
        let _io = self.serial();
        let save = {
            let c = self.core();
            let st = c.slot(done.slot);
            st.remember && st.dirty
        };
        let notes = if save { self.persist_slot_held(done.slot) } else { vec![] };
        self.record_ip_history_held(done.slot, &done.checked);
        notes
    }

    /// 실거래 연결이 확인된 국내 키라면 이번 공인 IP 관측을 저장 항목에 남김(등록 IP 증거가 아님).
    /// `key_io`를 든 채로 부름.
    fn record_ip_history_held(&self, slot: KeySlot, preflight: &PreflightResult) {
        let next = {
            let c = self.core();
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
            let mut next = remembered;
            next.connection_history = updated.connection_history;
            next
        };
        if credentials::save(self.store, &next).is_ok() {
            self.core().remembered = Some(next);
        }
    }

    /// 기억해 둔 키가 있으면 회원 키도 함께 저장함. 같으면 키체인을 건드리지 않고(입력할 때마다 묻지 않게),
    /// 실패하면 조용히 넘기지 않고 알림(다음 실행에 옛 회원 키가 돌아오지 않게).
    pub fn persist_member_key(&self) -> Notes {
        let _io = self.serial();
        let key = {
            let c = self.core();
            if !c.remember_supported {
                return vec![];
            }
            let Some(saved) = c.remembered.as_ref() else { return vec![] };
            let key = c.member_key.trim().to_string();
            if saved.member_key == key {
                return vec![];
            }
            key
        };
        let result = credentials::update_member_key(self.store, &key);
        let mut c = self.core();
        match result {
            Ok(Some(values)) => {
                c.remembered = Some(values);
                c.storage_error.clear();
                vec![]
            }
            Ok(None) => vec![],
            Err(e) => {
                c.storage_error = format!("회원 키를 기억해 두지 못했어요. {e}");
                vec![("warn", c.storage_error.clone())]
            }
        }
    }

    /// 읽지 못한 저장 항목을 지우고, '이 기기에 키 기억하기'를 켠 칸을 새로 저장함.
    pub fn recover(&self) -> Result<Notes, String> {
        let _io = self.serial();
        credentials::reset_unreadable(self.store)?;
        let slots: Vec<KeySlot> = {
            let mut c = self.core();
            c.remembered = None;
            c.load_status = LoadStatus::Missing;
            KeySlot::ALL.into_iter().filter(|s| c.slot(*s).remember && c.slot_pair(*s).is_some()).collect()
        };
        let mut notes = vec![];
        for s in slots {
            notes.extend(self.persist_slot_held(s));
        }
        Ok(notes)
    }
}

/// 칸마다 지금 키 — 키체인을 읽는 동안 사용자가 고쳤는지 비교할 때 씀.
fn key_snapshot(c: &Core) -> BTreeMap<KeySlot, KeyPair> {
    KeySlot::ALL.into_iter().map(|s| (s, c.slot(s).keys.clone())).collect()
}

/// 기억한 키를 칸에 채움. `before`와 달라진 칸(읽는 동안 고친 칸)은 키를 덮지 않고 '변경됨'만 다시 셈.
fn apply_remembered(c: &mut Core, remembered: &Credentials, before: &BTreeMap<KeySlot, KeyPair>) {
    c.remembered = Some(remembered.clone());
    for slot in KeySlot::ALL {
        let pair = remembered.exchanges.get(slot.as_str()).cloned();
        let complete = pair.as_ref().is_some_and(|p| !p.api_key.is_empty() && !p.api_secret.is_empty());
        let edited = before.get(&slot) != Some(&c.slot(slot).keys);
        let st = c.slot_mut(slot);
        st.remember = complete;
        if edited {
            refresh_dirty(c, slot);
            continue;
        }
        if let Some(p) = pair {
            st.keys = p;
        }
        st.dirty = false;
    }
    if !remembered.member_key.is_empty() && c.member_key.trim().is_empty() {
        c.member_key = remembered.member_key.clone();
    }
}

/// 칸의 키가 저장본과 다른지('변경됨') 다시 셈.
fn refresh_dirty(c: &mut Core, slot: KeySlot) {
    let cur = &c.slot(slot).keys;
    let cur = KeyPair { api_key: cur.api_key.trim().into(), api_secret: cur.api_secret.trim().into() };
    let saved = c.remembered.as_ref().and_then(|r| r.exchanges.get(slot.as_str()).cloned()).unwrap_or_default();
    c.slot_mut(slot).dirty = cur != saved;
}

impl Shared {
    pub(crate) fn vault(&self) -> Vault<'_> {
        Vault { core: &self.core, store: self.store.as_ref(), io: &self.key_io }
    }

    fn notify_all(&self, notes: Notes) {
        for (level, message) in notes {
            self.notice(level, message);
        }
    }

    pub(crate) fn load_remembered(&self) {
        self.vault().load();
    }

    /// '연결됨' 확인 뒤 키체인 일([`Vault::after_connected`]) — Core 잠금을 푼 뒤 부름.
    pub(crate) fn after_connected(&self, done: Option<ConnectedSlot>) {
        if let Some(done) = done {
            let notes = self.vault().after_connected(&done);
            self.notify_all(notes);
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
        let notes = self.vault().recover()?;
        self.notify_all(notes);
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
        refresh_dirty(c, slot);
        for r in c.runs.iter_mut().filter(|r| !r.running() && r.key_slot() == Some(slot)) {
            r.invalidate_preflight();
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
                r.invalidate_preflight();
            }
        }
        let notes = self.vault().persist_member_key();
        self.notify_all(notes);
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
        let persist = {
            let mut c = self.lock();
            if !c.remember_supported {
                return;
            }
            c.slot_mut(slot).remember = remember;
            !(remember && c.slot_pair(slot).is_none())
        };
        if persist {
            let notes = self.vault().persist_slot(slot);
            self.notify_all(notes);
        }
        self.emit();
    }

    pub fn set_close_to_tray(self: &S, value: bool) {
        self.set_pref(|c| c.close_to_tray = value);
    }

    pub fn set_show_dev_log(self: &S, show: bool) {
        self.set_pref(|c| c.show_dev_log = show);
    }

    /// 설정 하나를 바꾸고 prefs.json에 저장한 뒤 화면을 다시 그림.
    fn set_pref(self: &S, f: impl FnOnce(&mut Core)) {
        let saved = {
            let mut c = self.lock();
            f(&mut c);
            self.save_prefs(&c)
        };
        self.warn_save_failed(saved);
        self.emit();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use runner_core::credentials::MemoryStore;
    use runner_core::credentials::StoreError;
    use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

    type Hook = Box<dyn Fn(&mut Core) + Send + Sync>;

    /// 부를 때마다 Core 잠금이 비어 있는지 확인하는 키체인(메모리). 훅으로 '키체인을 기다리는 동안의 사용자 입력'을 흉내 냄.
    struct GuardStore {
        core: Arc<Mutex<Core>>,
        inner: MemoryStore,
        calls: AtomicUsize,
        fail_writes: AtomicBool,
        on_read: Mutex<Option<Hook>>,
        on_write: Mutex<Option<Hook>>,
    }

    impl GuardStore {
        fn new(core: Arc<Mutex<Core>>) -> Self {
            GuardStore {
                core,
                inner: MemoryStore::new(),
                calls: AtomicUsize::new(0),
                fail_writes: AtomicBool::new(false),
                on_read: Mutex::new(None),
                on_write: Mutex::new(None),
            }
        }

        /// Core 잠금이 비어 있어야 함 — 비어 있으면 훅을 돌림(훅이 Core를 잠가도 막히지 않음).
        fn enter(&self, hook: &Mutex<Option<Hook>>) {
            self.calls.fetch_add(1, Ordering::SeqCst);
            let guard = self.core.try_lock();
            assert!(guard.is_ok(), "키체인을 부르는 동안 Core 잠금을 들고 있음");
            let mut c = guard.unwrap();
            if let Some(f) = hook.lock().unwrap().take() {
                f(&mut c);
            }
        }
    }

    impl SecretStore for GuardStore {
        fn read(&self) -> Result<Option<String>, StoreError> {
            self.enter(&self.on_read);
            self.inner.read()
        }
        fn write(&self, value: &str) -> Result<(), StoreError> {
            self.enter(&self.on_write);
            if self.fail_writes.load(Ordering::SeqCst) {
                return Err(StoreError("write failed".into()));
            }
            self.inner.write(value)
        }
        fn delete(&self) -> Result<(), StoreError> {
            self.enter(&self.on_write);
            if self.fail_writes.load(Ordering::SeqCst) {
                return Err(StoreError("delete failed".into()));
            }
            self.inner.delete()
        }
    }

    struct Rig {
        core: Arc<Mutex<Core>>,
        store: GuardStore,
        io: Mutex<()>,
    }

    impl Rig {
        fn new() -> Rig {
            let mut c = Core::new("https://example.test".into(), true, PathBuf::from("/tmp/x"));
            c.member_key = "member".into();
            let core = Arc::new(Mutex::new(c));
            Rig { store: GuardStore::new(core.clone()), core, io: Mutex::new(()) }
        }
        fn vault(&self) -> Vault<'_> {
            Vault { core: &self.core, store: &self.store, io: &self.io }
        }
        fn core(&self) -> MutexGuard<'_, Core> {
            self.core.lock().unwrap()
        }
        fn type_keys(&self, slot: KeySlot, key: &str, secret: &str) {
            let mut c = self.core();
            c.slot_mut(slot).keys = KeyPair { api_key: key.into(), api_secret: secret.into() };
            c.slot_mut(slot).remember = true;
            Shared::slot_changed(&mut c, slot);
        }
    }

    fn pair(key: &str, secret: &str) -> KeyPair {
        KeyPair { api_key: key.into(), api_secret: secret.into() }
    }

    #[test]
    fn keychain_calls_never_hold_the_core_lock_and_results_apply() {
        let rig = Rig::new();
        rig.type_keys(KeySlot::Upbit, "access", "secret");
        assert!(rig.core().slot(KeySlot::Upbit).dirty);

        let notes = rig.vault().persist_slot(KeySlot::Upbit);
        assert_eq!(notes.len(), 1);
        assert_eq!(notes[0].0, "info");
        {
            let c = rig.core();
            assert!(!c.slot(KeySlot::Upbit).dirty, "저장한 키는 '변경됨'이 아님");
            assert_eq!(c.load_status, LoadStatus::Loaded);
            assert_eq!(c.remembered.as_ref().unwrap().exchanges["upbit"], pair("access", "secret"));
        }

        rig.core().member_key = "member-2".into();
        assert!(rig.vault().persist_member_key().is_empty());
        assert_eq!(rig.core().remembered.as_ref().unwrap().member_key, "member-2");

        // 새로 켠 앱처럼 비운 뒤 다시 읽음.
        {
            let mut c = rig.core();
            c.slot_mut(KeySlot::Upbit).keys = KeyPair::default();
            c.slot_mut(KeySlot::Upbit).remember = false;
            c.member_key.clear();
            c.remembered = None;
        }
        rig.vault().load();
        {
            let c = rig.core();
            assert_eq!(c.slot(KeySlot::Upbit).keys, pair("access", "secret"));
            assert!(c.slot(KeySlot::Upbit).remember && !c.slot(KeySlot::Upbit).dirty);
            assert_eq!(c.member_key, "member-2");
        }

        let checked = PreflightResult::new(true, String::new(), String::new());
        let notes = rig.vault().after_connected(&ConnectedSlot { slot: KeySlot::Upbit, checked });
        assert!(notes.is_empty(), "저장본과 같으면 다시 저장하지 않음");

        // 깨진 저장 항목을 지우고 기억하기를 켠 칸을 새로 저장함.
        rig.store.inner.write("not json").unwrap();
        let notes = rig.vault().recover().unwrap();
        assert_eq!(notes.len(), 1);
        assert_eq!(rig.core().remembered.as_ref().unwrap().exchanges["upbit"], pair("access", "secret"));
        assert!(rig.store.calls.load(Ordering::SeqCst) >= 8, "키체인을 실제로 불렀음");
    }

    #[test]
    fn keys_typed_while_the_keychain_is_read_are_kept() {
        let rig = Rig::new();
        rig.type_keys(KeySlot::Upbit, "old", "old-secret");
        rig.vault().persist_slot(KeySlot::Upbit);
        *rig.store.on_read.lock().unwrap() = Some(Box::new(|c: &mut Core| {
            c.slot_mut(KeySlot::Upbit).keys = pair("typed", "typed-secret");
        }));
        rig.vault().load();
        let c = rig.core();
        assert_eq!(c.slot(KeySlot::Upbit).keys, pair("typed", "typed-secret"), "읽는 동안 넣은 키를 덮지 않음");
        assert!(c.slot(KeySlot::Upbit).dirty, "새 키는 아직 저장되지 않음");
        assert_eq!(c.remembered.as_ref().unwrap().exchanges["upbit"], pair("old", "old-secret"));
    }

    #[test]
    fn a_key_changed_while_saving_stays_dirty() {
        let rig = Rig::new();
        rig.type_keys(KeySlot::Bithumb, "first", "first-secret");
        *rig.store.on_write.lock().unwrap() = Some(Box::new(|c: &mut Core| {
            c.slot_mut(KeySlot::Bithumb).keys = pair("second", "second-secret");
        }));
        rig.vault().persist_slot(KeySlot::Bithumb);
        let c = rig.core();
        assert_eq!(c.remembered.as_ref().unwrap().exchanges["bithumb"], pair("first", "first-secret"));
        assert!(c.slot(KeySlot::Bithumb).dirty, "저장 중에 바뀐 키는 다음 확인에서 다시 저장함");
    }

    #[test]
    fn a_failed_save_keeps_the_typed_keys_and_warns() {
        let rig = Rig::new();
        rig.type_keys(KeySlot::Upbit, "access", "secret");
        rig.store.fail_writes.store(true, Ordering::SeqCst);
        let notes = rig.vault().persist_slot(KeySlot::Upbit);
        assert_eq!(notes.len(), 1);
        assert_eq!(notes[0].0, "warn");
        let c = rig.core();
        assert!(c.storage_error.contains("지금 입력한 키는 그대로 남아요"), "{}", c.storage_error);
        assert!(c.slot(KeySlot::Upbit).dirty);
        assert_eq!(c.slot(KeySlot::Upbit).keys, pair("access", "secret"));
    }
}
