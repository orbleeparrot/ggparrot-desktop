//! 실행 하나의 건강 상태 — 마지막 스냅샷 읽기와 저하 상태(목록 카드·머리줄·트레이가 같이 쓰는 판단).

use super::*;

impl RunData {
    pub fn started(started_at: i64, session_id: Option<i64>) -> RunData {
        RunData {
            started_at,
            session_id,
            online: true,
            health: RunHealth { online: true, ..Default::default() },
            snapshot: Value::Object(Default::default()),
            updated_at: 0,
            finished: None,
            macro_origin: String::new(),
            entry_wait: None,
            chart: None,
            chart_live: None,
        }
    }

    fn flag(&self, key: &str) -> bool {
        self.snapshot.get(key).and_then(Value::as_bool).unwrap_or(false)
    }

    /// 마지막 스냅샷에서 포지션을 들고 있었는지 여부.
    pub fn holding(&self) -> bool {
        self.flag("in_position")
    }

    /// 마지막 스냅샷에서 주문 결과를 몰랐는지 여부(포지션 불확실).
    pub fn uncertain(&self) -> bool {
        self.flag("position_uncertain")
    }
}

impl Run {
    /// 실행 중 저하 상태 한 줄과 tone — 정상이면 None. 목록 카드·머리줄·트레이가 같은 판단을 씀.
    /// 앞쪽일수록 급함: 주문 결과 모름 → 정리 실패 → 시세 끊김(포지션 있음) → 회원 키 거절 → 서버 거절 →
    /// 잔고 부족(진입 주문 거절) → 시세 다시 받는 중 → 끊김.
    pub fn degraded(&self) -> Option<(String, &'static str)> {
        let (what, tone) = self.degraded_raw()?;
        // 모의는 실제 계정·주문이 없음 — 알리되 '확인 필요'(빨강)로 올리지 않음.
        let tone = if self.mode == RunMode::Mock && tone == "danger" { "warn" } else { tone };
        Some((what, tone))
    }

    fn degraded_raw(&self) -> Option<(String, &'static str)> {
        if !self.running() {
            return None;
        }
        let d = self.data.as_ref()?;
        let h = &d.health;
        let in_position = d.holding();
        let offline_min = |since: u64| (now_ms() as u64).saturating_sub(since) / 60_000;
        Some(if d.uncertain() {
            ("주문 결과 확인 필요".into(), "danger")
        } else if h.close_failures > 0 {
            (format!("정리 주문 실패 {}회", h.close_failures), "danger")
        } else if h.price_stale_since > 0 && in_position {
            ("시세 끊김 · 손절 확인 멈춤".into(), "danger")
        } else if h.auth_failed {
            ("회원 키 거절됨".into(), "danger")
        } else if h.rejected != 0 {
            (format!("서버가 요청 거절(HTTP {})", h.rejected), "danger")
        } else if h.funds_short {
            // 손절·정리는 그대로 됨 — 위험(빨강)은 아니지만 사람이 잔고를 채워야 풀림(→ needs_attention).
            ("잔고 부족 · 주문 못 함".into(), "warn")
        } else if h.price_stale_since > 0 {
            ("시세 다시 받는 중".into(), "warn")
        } else if !h.online || !d.online {
            match offline_min(h.offline_since) {
                m if h.offline_since > 0 && m >= 5 => (format!("서버 끊김 {m}분"), "danger"),
                _ => ("서버 재시도 중".into(), "warn"),
            }
        } else if d.updated_at == 0 {
            ("준비 중".into(), "muted")
        } else {
            return None;
        })
    }

    /// 사용자가 봐야 하는지 여부 — 실행 중 심각한 저하·잔고 부족(기다려도 풀리지 않음), 또는 오류로 끝나
    /// 아직 다시 시작하지 않은 실행.
    /// 확인이 필요한 실행의 심각도 — 위험 상태·오류로 끝남은 "danger", 잔고 부족만이면 "warn". 아니면 None.
    pub fn attention_tone(&self) -> Option<&'static str> {
        if !self.needs_attention() {
            return None;
        }
        let danger = !self.running() || self.degraded().is_some_and(|(_, tone)| tone == "danger");
        Some(if danger { "danger" } else { "warn" })
    }

    pub fn needs_attention(&self) -> bool {
        if self.running() {
            let funds_short = self.data.as_ref().is_some_and(|d| d.health.funds_short);
            return self.degraded().is_some_and(|(_, tone)| tone == "danger")
                || (funds_short && self.mode != RunMode::Mock);
        }
        // 모의가 오류로 끝나도 실제 포지션·주문은 없음.
        self.mode != RunMode::Mock
            && self.data.as_ref().and_then(|d| d.finished.as_ref()).is_some_and(|(st, _, _)| st == "error")
    }
}
