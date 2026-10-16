//! 키 칸 연결 확인(주문 없음)과 이 기기의 공인 IP.

use super::*;

impl Shared {
    /// 바이낸스 실거래 칸의 지금 확인 지문 — 실행 목록이 바뀌기 전에 떠 두고 `recheck_live_slot_if_moved`에 넘김.
    pub(crate) fn live_probe_fingerprint(&self) -> String {
        self.lock().slot_fingerprint(KeySlot::Binance)
    }

    /// 실행 추가·빼기·모드 변경으로 실거래 칸의 확인 시장(`Core::probe_market`)이 바뀌었으면 그 시장으로 다시 확인.
    pub(crate) fn recheck_live_slot_if_moved(self: &S, before: &str) {
        let moved = {
            let c = self.lock();
            c.slot_pair(KeySlot::Binance).is_some() && c.slot_fingerprint(KeySlot::Binance) != before
        };
        if moved {
            self.schedule_slot_check(KeySlot::Binance, 700);
        }
    }

    pub fn schedule_slot_check(self: &S, slot: KeySlot, delay_ms: u64) {
        let timer = {
            let mut c = self.lock();
            let st = c.slot_mut(slot);
            st.timer += 1;
            st.scheduled = false;
            if c.closed || c.slot_pair(slot).is_none() {
                drop(c);
                self.emit();
                return;
            }
            // 이미 확인했거나 확인 중인 지문이면 예약하지 않음.
            let fp = c.slot_fingerprint(slot);
            if !matches!(c.slot_status_at(slot, &fp), SlotStatus::Unchecked) {
                return;
            }
            let st = c.slot_mut(slot);
            st.scheduled = true;
            st.timer
        };
        self.emit();
        let me = self.clone();
        tauri::async_runtime::spawn(async move {
            tokio::time::sleep(Duration::from_millis(delay_ms)).await;
            let due = {
                let mut c = me.lock();
                let st = c.slot_mut(slot);
                let due = st.timer == timer && st.scheduled;
                if due {
                    st.scheduled = false;
                }
                due
            };
            if due {
                me.begin_slot_check(slot, false);
            }
        });
    }

    /// 지금 칸의 키로 연결을 확인함. 시작했으면 참.
    pub fn begin_slot_check(self: &S, slot: KeySlot, force: bool) -> bool {
        let job = {
            let mut c = self.lock();
            {
                let st = c.slot_mut(slot);
                st.timer += 1;
                st.scheduled = false;
            }
            if c.closed || c.slot_pair(slot).is_none() {
                None
            } else {
                let (m, mode, creds, fp) = c.slot_check_snapshot(slot);
                // 이미 확인했거나 확인 중인 지문이면 다시 묻지 않음(force면 새로 확인).
                if !force && !matches!(c.slot_status_at(slot, &fp), SlotStatus::Unchecked) {
                    return true;
                }
                let st = c.slot_mut(slot);
                st.generation += 1;
                st.busy_fp = Some(fp.clone());
                Some((st.generation, m, mode, creds, fp))
            }
        };
        self.emit();
        let Some((generation, m, mode, creds, fp)) = job else {
            self.refresh_wizard();
            return false;
        };
        let me = self.clone();
        tauri::async_runtime::spawn(async move {
            let result = perform_connection_check(&m, &creds, mode, &me.check_http).await;
            me.accept_slot_check(slot, generation, fp, result);
        });
        true
    }

    pub(crate) fn accept_slot_check(self: &S, slot: KeySlot, generation: u64, fp: String, result: PreflightResult) {
        let (reschedule, connected) = {
            let mut c = self.lock();
            if c.closed || generation != c.slot(slot).generation {
                return;
            }
            c.slot_mut(slot).busy_fp = None;
            if fp != c.slot_fingerprint(slot) {
                (true, None)
            } else {
                (false, c.store_slot_check(slot, fp, &result))
            }
        };
        self.after_connected(connected);
        if reschedule {
            self.schedule_slot_check(slot, 700);
        }
        self.emit();
        self.refresh_wizard();
    }

    // --- 공인 IP -------------------------------------------------------------------
    pub fn begin_public_ip(self: &S, force: bool) {
        let token = {
            let mut c = self.lock();
            if c.closed {
                return;
            }
            match c.ip_cache.begin(force) {
                BeginOutcome::Cached | BeginOutcome::Shared(_) => None,
                BeginOutcome::Request(t) => Some(t),
            }
        };
        self.emit();
        let Some(token) = token else { return };
        let me = self.clone();
        tauri::async_runtime::spawn(async move {
            let value = connection::detect_public_ipv4().await;
            let expiry = {
                let mut c = me.lock();
                if c.closed {
                    return;
                }
                match c.ip_cache.finish(token, value) {
                    Some(r) if r.ok => {
                        let age = r.checked_at.elapsed().as_secs_f64();
                        Some((connection::PUBLIC_IP_MAX_AGE_SECS - age).max(0.0) + 0.05)
                    }
                    _ => None,
                }
            };
            me.emit();
            me.refresh_wizard();
            if let Some(secs) = expiry {
                tokio::time::sleep(Duration::from_secs_f64(secs)).await;
                me.emit();
                me.refresh_wizard();
            }
        });
    }

    /// 조회 당시 주소(120초 안)만 복사함. 복사했으면 참. 복사할 주소가 없으면 화면만 다시 그림.
    pub fn copy_public_ip(&self) -> bool {
        let address = {
            let c = self.lock();
            match c.ip_cache.result() {
                Some(r) if r.ok && r.fresh() && !c.ip_cache.busy() => Some(r.address.clone()),
                _ => None,
            }
        };
        match address {
            Some(a) => self.write_clipboard(&a),
            None => {
                self.emit();
                false
            }
        }
    }
}

impl Core {
    /// 칸 확인 결과를 남김. 연결됨이고 키를 기억할 수 있으면 키체인에 남길 일을 돌려줌 —
    /// 부르는 쪽이 Core 잠금을 푼 뒤 [`Shared::after_connected`]로 넘김.
    pub(crate) fn store_slot_check(
        &mut self,
        slot: KeySlot,
        fp: String,
        result: &PreflightResult,
    ) -> Option<ConnectedSlot> {
        let view = check_view(slot.exchange().as_str(), result.ok, &result.error_name);
        let connected = view.connected;
        self.slot_mut(slot).check =
            Some(KeyCheckRecord { fingerprint: fp, reason: result.reason.clone(), view, checked_at: now_ms() });
        if !connected || !self.remember_supported {
            return None;
        }
        let fp_live =
            connection::preflight_fingerprint(&slot.probe_macro(), RunMode::Live, &self.slot_credentials(slot));
        Some(ConnectedSlot { slot, checked: PreflightResult::new(true, result.reason.clone(), fp_live) })
    }
}
