//! 화면에 보낼 상태 스냅샷 — Core를 camelCase DTO(src/types.ts v2)로 그림.

use super::*;

impl Core {
    pub fn render(&self, version: &str) -> AppStateDto {
        self.render_with(version, true)
    }

    /// `library`가 거짓이면 매크로 폴더 목록을 빼고 그림(화면은 지난 목록을 그대로 씀).
    pub fn render_with(&self, version: &str, library: bool) -> AppStateDto {
        AppStateDto {
            version: version.into(),
            runs: self.runs.iter().map(|r| self.render_run(r)).collect(),
            settings: self.render_settings(),
            library: library.then(|| LibraryDto {
                dir: self.library_dir.to_string_lossy().to_string(),
                files: self
                    .library
                    .iter()
                    .map(|e| LibraryFileDto {
                        file_name: e.file_name.clone(),
                        valid: e.valid,
                        error: e.error.clone(),
                        title: e.title.clone(),
                        exchange: Exchange::parse(&e.exchange_name),
                        exchange_label: e.exchange_label.clone(),
                        symbol: e.symbol.clone(),
                        side: e.side.clone(),
                        summary: e.summary.clone(),
                        strategy: e.strategy.clone(),
                        unsupported: e.unsupported.clone(),
                        signed: e.signed,
                        modified_at: e.modified_ms,
                        in_use: self
                            .runs
                            .iter()
                            .filter(|r| r.library_file.as_deref() == Some(e.file_name.as_str()))
                            .count(),
                    })
                    .collect(),
                skipped: self.library_skipped,
                truncated: self.library_truncated,
            }),
            wizard: self.render_wizard(),
            update: UpdateDto {
                status: self.update.status.into(),
                current: version.into(),
                version: self.update.version.clone(),
                notes: self.update.notes.clone(),
                progress: self.update.progress,
                downloaded: self.update.downloaded,
                total: self.update.total,
                error: self.update.error.clone(),
                checked_at: self.update.checked_at,
                blocked: self.update.blocked.clone(),
                updated_from: self.update.updated_from.clone(),
            },
            status: self.header_status(),
            claiming: self.claim_busy,
        }
    }

    /// 머리줄 상태 — 실행 중 수·확인 필요 수·불안정.
    /// - 웹 연결(claiming) 중 글자는 머리줄(Header.tsx)이 '웹에서 매크로를 받는 중…'으로 덮음. 여기서는 색(warn)만 정함.
    fn header_status(&self) -> StatusDto {
        let running = self.running_count();
        // 확인이 필요한 실행(실행 중 심각한 저하·오류로 끝남)과 잠깐 불안정한 실행을 따로 셈.
        let attention = self.runs.iter().filter(|r| r.needs_attention()).count();
        // 머리줄 색은 가장 심각한 것 — 잔고 부족만이면 노랑(주문만 못 함, 손절·정리는 됨).
        let urgent = self.runs.iter().any(|r| r.attention_tone() == Some("danger"));
        let shaky = self.runs.iter().filter(|r| r.degraded().is_some_and(|(_, tone)| tone == "warn")).count();
        let mut parts = vec![];
        if running > 0 {
            parts.push(format!("실행 중 {running}개"));
        }
        if attention > 0 {
            parts.push(format!("확인 필요 {attention}개"));
        } else if shaky > 0 {
            parts.push("연결 다시 시도하는 중".to_string());
        }
        let tone = if urgent {
            "danger"
        } else if attention > 0 || shaky > 0 || self.claim_busy {
            "warn"
        } else if !parts.is_empty() {
            "ok"
        } else {
            "muted"
        };
        let text = if !parts.is_empty() {
            parts.join(" · ")
        } else if self.runs.is_empty() {
            "매크로를 추가하세요".to_string()
        } else {
            "대기 중".to_string()
        };
        StatusDto { text, tone: tone.into() }
    }

    pub(crate) fn render_run(&self, r: &Run) -> RunDto {
        let m = &r.macro_;
        let name = r.exchange_name();
        let ex = r.exchange();
        let side = macro_side(m);
        let lev = macro_leverage(m).unwrap_or(1);
        let symbol = macro_symbol(m);
        let quote = quote_of(&symbol);
        let phase = r.phase();
        let (can_start, blocker, needs) = self.readiness(r);
        let status = r.status_line(phase, can_start);
        RunDto {
            id: r.id.clone(),
            macro_: MacroDto {
                title: r.title.clone(),
                source: if r.source == "web" { "web".into() } else { "file".into() },
                exchange_label: exchange_label(&name),
                exchange_name: name.clone(),
                exchange: ex,
                base_asset: base_asset(&symbol),
                quote: quote.into(),
                market: market_of(&name, side, lev).as_str().into(),
                symbol,
                side: side.as_str().into(),
                leverage: lev,
                summary: m.get("human_summary").and_then(Value::as_str).unwrap_or("").trim().to_string(),
                signature: r.signature().into(),
                strategy: runner_core::macro_info::rule_type(m)
                    .map(runner_core::macro_info::strategy_label)
                    .unwrap_or_default()
                    .into(),
                capital: runner_core::macro_info::initial_capital(m),
                capital_original: r.original_capital,
                capital_locked: if r.capital_locked() { CAPITAL_LOCKED.into() } else { String::new() },
                order_cap: runner_core::types::order_cap(quote),
                risk: runner_core::macro_info::risk_summary(m),
                exits: runner_core::exits::exit_rules(m),
            },
            mode: r.mode,
            mode_choices: r.mode_choices(),
            phase: phase.into(),
            key_slot: r.key_slot(),
            readiness: ReadinessDto { can_start, blocker, needs },
            run: r.data.as_ref().map(|d| render_run_data(d, side)),
            web_linked: r.web_linked,
            market_page: r.market_page().map(|p| p.label).unwrap_or_default(),
            web_run_page: r.web_run_url().is_some(),
            status: StatusDto { text: status.0, tone: status.1.into() },
        }
    }

    /// (시작 가능, 막힌 이유, 설정으로 데려갈 곳)
    pub(crate) fn readiness(&self, r: &Run) -> (bool, String, Option<NeedsDto>) {
        if r.busy() {
            return (false, String::new(), None);
        }
        if let Some(why) = runner_core::macro_info::unsupported_reason(&r.macro_) {
            return (false, why.to_string(), None);
        }
        let Some(ex) = r.exchange() else {
            return (false, runner_core::types::unsupported_exchange_message(&r.exchange_name()), None);
        };
        if self.conflicting_run(r).is_some() {
            return (false, Core::conflict_message(r), None);
        }
        if r.mode != RunMode::Mock {
            let slot = KeySlot::for_run(ex, r.mode, r.market());
            if self.slot_pair(slot).is_none() {
                return (
                    false,
                    runner_core::types::exchange_key_needed_message(slot.label()),
                    Some(NeedsDto::Key { slot }),
                );
            }
        }
        if self.member_key.trim().is_empty() {
            return (false, MEMBER_KEY_NEEDED.into(), Some(NeedsDto::Member));
        }
        if self.claim_busy {
            return (false, CLAIM_BUSY_WAIT.into(), None);
        }
        (true, String::new(), None)
    }

    pub(crate) fn render_settings(&self) -> SettingsDto {
        SettingsDto {
            slots: KeySlot::ALL.iter().map(|s| self.render_slot(*s)).collect(),
            member_key_filled: !self.member_key.trim().is_empty(),
            member_key_pending: self.pending_member_key.as_deref().map(key_hint).unwrap_or_default(),
            member_in_use_by: self.running_count(),
            storage: StorageDto {
                status: if !self.remember_supported {
                    "unsupported".into()
                } else {
                    serde_json::to_value(self.load_status)
                        .ok()
                        .and_then(|v| v.as_str().map(str::to_string))
                        .unwrap_or_default()
                },
                note: self.load_failure_note(),
            },
            show_dev_log: self.show_dev_log,
            close_to_tray: self.close_to_tray,
            server_base: self.server_base.clone(),
            server_custom: !is_default_server(&self.server_base),
            os: crate::shared::os_summary().into(),
            public_ip: {
                let (status, address) = self.public_ip_view();
                PublicIpDto { status: status.into(), address }
            },
            server_link_level: runner_core::SERVER_LINK_LEVEL.into(),
        }
    }

    pub(crate) fn render_slot(&self, s: KeySlot) -> SlotDto {
        let st = self.slot(s);
        let ex = s.exchange();
        let check = self.slot_status(s);
        let (status, headline, mut note) = match check {
            SlotStatus::Empty => ("empty", "● 연결 안 됨".to_string(), String::new()),
            SlotStatus::Busy(_) => ("checking", "● 확인 중".to_string(), String::new()),
            SlotStatus::Done(c) if c.view.connected => {
                let n = if c.view.problem == "order_size" {
                    c.reason.clone()
                } else if now_ms() - c.checked_at > KEY_CHECK_STALE_MS {
                    "확인한 지 30분이 넘었어요. 매크로를 시작할 때 다시 확인해요.".to_string()
                } else {
                    String::new()
                };
                ("connected", "✓ 연결됨".to_string(), n)
            }
            SlotStatus::Done(c) => {
                // 막힌 곳을 머리줄에 바로 씀 — 고치는 길은 [연결 도우미]에 있음.
                // 바이낸스는 어느 권한을 켤지(선물은 키를 새로 만들어야 하는 경우까지)가 이유에 있어 아래에 둠.
                let note = if ex.is_domestic() { String::new() } else { c.reason.clone() };
                ("blocked", format!("● {}", c.view.headline), note)
            }
            SlotStatus::Unchecked => ("unchecked", "● 확인 전".to_string(), String::new()),
        };
        if note.is_empty() {
            let failure = self.load_failure_note();
            note = if !failure.is_empty() { failure } else { self.storage_error.clone() };
        }
        // 다시 확인하는 중에도 같은 지문의 지난 결과는 세 줄에 그대로 둠.
        let checks = match check {
            SlotStatus::Done(c) | SlotStatus::Busy(Some(c)) => {
                ChecksDto { auth: c.view.auth, ip: c.view.ip, order: c.view.order }
            }
            _ => ChecksDto::waiting(),
        };
        let (a, sec) = ex.field_labels();
        SlotDto {
            slot: s,
            exchange: ex,
            label: s.label().into(),
            field_labels: [a.into(), sec.into()],
            access_filled: !st.keys.api_key.trim().is_empty(),
            secret_filled: !st.keys.api_secret.trim().is_empty(),
            remembered: st.remember,
            remember_supported: self.remember_supported,
            status: status.into(),
            headline,
            note,
            checks,
            // 업비트·빗썸·바이낸스(실거래·테스트넷·데모 칸 모두) — 바이낸스는 칸이 곧 사이트임.
            wizard_available: true,
            in_use_by: self
                .runs
                .iter()
                .filter(|r| r.running() && r.key_slot() == Some(s) && r.mode != RunMode::Mock)
                .count(),
        }
    }

    /// 공인 IP 조회 결과(연결 도우미·정보 화면이 함께 씀) — 120초가 지난 주소는 보여 주지 않음.
    pub(crate) fn public_ip_view(&self) -> (&'static str, String) {
        if self.ip_cache.busy() {
            return ("loading", String::new());
        }
        match self.ip_cache.result() {
            Some(r) if r.ok && r.fresh() => ("ok", r.address.clone()),
            Some(r) if r.ok => ("expired", String::new()),
            Some(_) => ("error", String::new()),
            None => ("idle", String::new()),
        }
    }

    /// 문의용 정보(설정 › 정보 › [문의용 정보 복사]) — 버전·OS·공인 IP·서버·키 칸 상태·매크로 상태.
    /// 거래소 키·회원 키 값은 넣지 않음(입력 여부만). 끝에 최근 앱 기록 20줄.
    pub(crate) fn support_text(&self, version: &str, os: &str, now: chrono::DateTime<chrono::Local>) -> String {
        let when = |ms: i64| {
            chrono::DateTime::from_timestamp_millis(ms)
                .map(|d| d.with_timezone(&chrono::Local).format("%m-%d %H:%M").to_string())
                .unwrap_or_default()
        };
        let u = &self.update;
        let update = match u.status {
            "none" => format!("최신 버전 (마지막 확인 {})", when(u.checked_at)),
            "available" => format!("새 버전 v{} 있음", u.version),
            "downloading" => format!("v{} 내려받는 중", u.version),
            "ready" => format!("v{} 설치 대기", u.version),
            "installing" => format!("v{} 설치 중", u.version),
            "error" => format!("확인 실패 — {}", u.error),
            "unsupported" => "자동 업데이트 없음(개발용 빌드)".into(),
            "checking" => "확인 중".into(),
            _ => "확인 전".into(),
        };
        let ip = match self.public_ip_view() {
            ("ok", a) => a,
            ("loading", _) => "확인 중".into(),
            ("error", _) => "확인하지 못함".into(),
            _ => "확인 전".into(),
        };
        let server = if is_default_server(&self.server_base) {
            format!("{} (기본)", self.server_base)
        } else {
            format!("{} (기본 서버 아님)", self.server_base)
        };
        let mut lines = vec![
            format!("[껄무새 문의용 정보] {}", now.format("%Y-%m-%d %H:%M (%:z)")),
            format!("앱 버전: v{version}"),
            format!("서버 연동 레벨: {}", runner_core::SERVER_LINK_LEVEL),
            format!("업데이트: {update}"),
            format!("OS: {os}"),
            format!("공인 IP: {ip}"),
            format!("서버: {server}"),
            format!("회원 키: {}", if self.member_key.trim().is_empty() { "없음" } else { "입력됨" }),
            "거래소 연결:".into(),
        ];
        for s in KeySlot::ALL {
            let d = self.render_slot(s);
            lines.push(format!("- {}: {}", s.label(), d.headline.trim_start_matches(['●', '✓', ' '])));
        }
        lines.push(format!("매크로 {}개:", self.runs.len()));
        for r in &self.runs {
            let d = self.render_run(r);
            lines.push(format!(
                "- {} {} · {} · {} ({})",
                exchange_label(&r.exchange_name()),
                r.symbol(),
                r.mode.label(),
                d.status.text,
                r.title
            ));
        }
        // 최근 앱 기록 — 넣을 때 이미 키 값을 가렸음(Core::push_app_log).
        let recent = self.recent_app_log_lines(super::SUPPORT_APP_LOG_LINES);
        if !recent.is_empty() {
            lines.push("최근 앱 기록:".into());
            lines.extend(recent.into_iter().map(|l| format!("- {l}")));
        }
        lines.join("\n")
    }

    pub(crate) fn render_wizard(&self) -> Option<WizardDto> {
        let w = self.wizard.as_ref()?;
        let result = self.ip_cache.result();
        let (ip_status, address) = self.public_ip_view();
        let slot = w.slot();
        let (state, view, reason) = self.wizard_check(slot);
        let show_view = matches!(w.stage.as_str(), "result_ok" | "result_fail") || state == "done";
        let ex = slot.exchange();
        let (a, s) = ex.field_labels();
        // 바이낸스는 키를 만드는 사이트 이름(칸 이름)으로 부름 — 테스트넷·데모·실거래.
        let home = match slot {
            KeySlot::BinanceTestnet => "테스트넷",
            KeySlot::BinanceDemo => "데모",
            _ => ex.label(),
        };
        Some(WizardDto {
            exchange: ex,
            slot,
            env: w.env().into(),
            market: w.target.market,
            label: slot.label().into(),
            home_label: home.into(),
            stage: w.stage.clone(),
            path: w.path.clone(),
            ip: IpDto {
                status: ip_status.into(),
                address,
                note: runner_core::connection::public_ip_status(result),
                changed: result.is_some_and(|r| r.changed),
            },
            slots: SlotsDto { access: w.got_access, secret: w.got_secret },
            field_labels: [a.into(), s.into()],
            checks: if show_view {
                ChecksDto { auth: view.auth, ip: view.ip, order: view.order }
            } else {
                ChecksDto::waiting()
            },
            headline: if show_view { view.headline.clone() } else { String::new() },
            notes: if show_view { view.notes.clone() } else { CheckNotes::default() },
            keys_ready: self.slot_pair(slot).is_some(),
            reason,
            problem: if show_view { view.problem.clone() } else { String::new() },
            watching_clipboard: WATCH_STAGES.contains(&w.stage.as_str()),
            remembered: self.slot(slot).remember,
            remember_supported: self.remember_supported,
        })
    }
}

impl Run {
    /// 매크로 출처 표시 — web·modified(파일을 고침)·signed·unsigned.
    pub(crate) fn signature(&self) -> &'static str {
        if self.source == "web" {
            "web"
        } else if self.modified || (self.sig.is_some() && self.original_capital.is_some()) {
            "modified"
        } else if self.sig.is_some() {
            "signed"
        } else {
            "unsigned"
        }
    }

    /// 화면의 단계 — running·stopping·starting·checking·finished·idle.
    pub(crate) fn phase(&self) -> &'static str {
        if self.running() {
            if self.stopping {
                "stopping"
            } else {
                "running"
            }
        } else if self.start_pending && self.cancel_requested.is_some() {
            "stopping"
        } else if self.start_pending {
            "starting"
        } else if self.checking {
            "checking"
        } else if self.data.as_ref().is_some_and(|d| d.finished.is_some()) {
            "finished"
        } else {
            "idle"
        }
    }

    /// 카드의 상태 한 줄과 tone — 단계(`phase`)와 시작 가능 여부로.
    pub(crate) fn status_line(&self, phase: &str, can_start: bool) -> (String, &'static str) {
        match phase {
            "running" => match self.degraded() {
                Some((what, tone)) => (format!("실행 중 · {what}"), tone),
                None => ("실행 중".to_string(), "ok"),
            },
            "stopping" => ("종료 처리 중…".to_string(), "warn"),
            "starting" => ("시작하는 중…".to_string(), "warn"),
            "checking" => ("연결 확인 중…".to_string(), "warn"),
            "finished" => {
                let (st, note, _) = self.data.as_ref().and_then(|d| d.finished.clone()).unwrap_or_default();
                let tone = if st == "error" { "danger" } else { "muted" };
                let what = if note.is_empty() { finish_label(&st).to_string() } else { note };
                (format!("종료됨 · {what}"), tone)
            }
            _ if can_start => ("대기 중".to_string(), "muted"),
            _ => ("시작 전 확인 필요".to_string(), "warn"),
        }
    }
}

/// 끝난 상태 이름(stopped·error)을 사람이 읽는 말로.
pub(crate) fn finish_label(status: &str) -> &'static str {
    if status == "error" {
        "오류로 멈춤"
    } else {
        "종료"
    }
}

pub(crate) fn render_run_data(d: &RunData, side: Side) -> RunStatusDto {
    let s = &d.snapshot;
    let f = |k: &str| s.get(k).and_then(Value::as_f64).unwrap_or(0.0);
    let in_position = d.holding();
    let (price, entry, qty) = (f("last_price"), f("entry_price"), f("position_qty"));
    let opt = |k: &str| s.get(k).and_then(Value::as_f64).filter(|v| v.is_finite());
    let gross = if in_position && price > 0.0 && entry > 0.0 { pnl_amount(qty, entry, price, side) } else { 0.0 };
    // 화면의 평가손익은 봇이 보낸 수수료 반영 값(지금 정리하면). 수수료를 모르면 null이 와서 수수료 전 값을 씀.
    let (net, net_pct) = (opt("unrealized_net"), opt("unrealized_net_pct"));
    let fee_aware = in_position && net.is_some() && net_pct.is_some();
    RunStatusDto {
        started_at: d.started_at,
        session_id: d.session_id,
        online: d.online && d.health.online,
        auth_failed: d.health.auth_failed,
        rejected: d.health.rejected,
        offline_since: d.health.offline_since as i64,
        price_stale_since: d.health.price_stale_since as i64,
        close_failures: d.health.close_failures,
        funds_short: d.health.funds_short,
        updated_at: d.updated_at,
        price,
        in_position,
        uncertain: d.uncertain(),
        entry_price: entry,
        qty,
        unrealized_pct: if fee_aware { net_pct.unwrap_or(0.0) } else { f("unrealized_pct") },
        unrealized: if fee_aware { net.unwrap_or(0.0) } else { gross },
        unrealized_fee_aware: fee_aware,
        invested: match s.get("invested").and_then(Value::as_f64).filter(|v| v.is_finite()) {
            Some(v) => v,
            None if in_position && entry > 0.0 => qty * entry,
            None => 0.0,
        },
        realized: f("realized_pnl"),
        realized_net: opt("realized_net"),
        fees: opt("fees_paid").filter(|v| *v >= 0.0),
        day_change_pct: s.get("day_change").and_then(Value::as_f64).filter(|v| v.is_finite()).map(|v| v * 100.0),
        macro_origin: d.macro_origin.clone(),
        note: s.get("note").and_then(Value::as_str).unwrap_or("").to_string(),
        finished: d.finished.as_ref().map(|(st, n, at)| FinishedDto { status: st.clone(), note: n.clone(), at: *at }),
    }
}

// --- DTO (camelCase, src/types.ts v2) --------------------------------------------------

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Notice {
    pub level: String,
    pub title: String,
    pub message: String,
    /// 이 알림을 남긴 앱 기록 번호 — 화면이 [자세히]로 앱 기록을 열 때 씀.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub log_id: Option<u64>,
}

#[derive(Serialize, Clone)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum StartReply {
    Started,
    ConfirmLive { title: String, message: String },
    Checking,
    Blocked { message: String },
    Wizard,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppStateDto {
    version: String,
    runs: Vec<RunDto>,
    settings: SettingsDto,
    /// 바뀌었을 때만 실음(get_state는 늘). 없으면 화면은 지난 목록을 씀.
    #[serde(skip_serializing_if = "Option::is_none")]
    library: Option<LibraryDto>,
    wizard: Option<WizardDto>,
    update: UpdateDto,
    status: StatusDto,
    claiming: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct MacroDto {
    title: String,
    source: String,
    exchange_name: String,
    exchange: Option<Exchange>,
    exchange_label: String,
    symbol: String,
    base_asset: String,
    quote: String,
    market: String,
    side: String,
    leverage: u32,
    summary: String,
    signature: String,
    /// 웹과 같은 전략 이름. 모르면 "".
    strategy: String,
    /// 투자 금액(params.initial_capital). 없으면 null.
    capital: Option<f64>,
    /// 사용자가 바꾸기 전 금액. 바꾸지 않았으면 null.
    capital_original: Option<f64>,
    /// 투자 금액을 바꿀 수 없는 이유(웹 연결 매크로). 바꿀 수 있으면 "".
    capital_locked: String,
    /// 1회 주문 상한(견적 통화). 투자 금액이 커도 한 번에 이보다 많이 주문하지 않음.
    order_cap: f64,
    /// 이 앱이 로컬에서 지키는 안전 설정(예: "손절 평단 대비 가격 -3%"). 없으면 [].
    risk: Vec<String>,
    /// 청산 기준(익절·손절·트레일링, 평단 대비 가격 %) — 상태 카드의 청산 기준 막대·요약.
    exits: runner_core::exits::ExitRules,
}

#[derive(Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub(crate) enum NeedsDto {
    Key { slot: KeySlot },
    Member,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ReadinessDto {
    can_start: bool,
    blocker: String,
    needs: Option<NeedsDto>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct RunDto {
    id: String,
    #[serde(rename = "macro")]
    macro_: MacroDto,
    mode: RunMode,
    mode_choices: Vec<RunMode>,
    phase: String,
    key_slot: Option<KeySlot>,
    readiness: ReadinessDto,
    run: Option<RunStatusDto>,
    web_linked: bool,
    /// 거래소 종목 화면 단추 글자("업비트에서 보기" 등). 열 화면이 없으면 "". 주소는 `open_market_page`가 만듦.
    market_page: String,
    /// 껄무새 웹에서 이 실행 기록을 열 수 있는지 여부. 주소는 `open_run_web_page`가 만듦.
    web_run_page: bool,
    status: StatusDto,
}

#[derive(Serialize)]
pub(crate) struct FinishedDto {
    status: String,
    note: String,
    at: i64,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct RunStatusDto {
    started_at: i64,
    session_id: Option<i64>,
    online: bool,
    auth_failed: bool,
    rejected: u16,
    offline_since: i64,
    price_stale_since: i64,
    close_failures: u32,
    /// 진입 주문이 잔고(증거금) 부족으로 거절된 뒤 아직 진입 주문이 체결되지 않음 — 목록 카드·상태 카드·머리줄이 알림.
    funds_short: bool,
    updated_at: i64,
    price: f64,
    in_position: bool,
    uncertain: bool,
    entry_price: f64,
    qty: f64,
    /// 평가손익 %(지금 정리하면) — 목록 카드·상태 카드가 함께 씀. 수수료를 알면 투입금 대비 수수료 반영 %,
    /// 모르면 평단 대비 가격 %(수수료 전, 서버 unrealized_pct와 같음).
    unrealized_pct: f64,
    /// 평가손익 금액(견적 통화) — `unrealized_pct`와 같은 기준.
    unrealized: f64,
    /// 평가손익이 수수료 반영 값인지 여부. 거짓이면 화면이 '수수료 제외 전'을 붙임.
    unrealized_fee_aware: bool,
    /// 투입금 — 지금 포지션에 들어간 돈(수량×평단 + 남은 진입 수수료, 수수료를 모르면 수량×평단). 보유가 없으면 0.
    invested: f64,
    /// 서버에 보고하는 실현손익 그대로(수수료를 빼지 않음).
    realized: f64,
    /// 이번 실행 실현손익(수수료 반영). 수수료를 모르면 null.
    realized_net: Option<f64>,
    /// 이번 실행에서 낸 수수료(견적 통화). 모르면 null — 선물·BNB로 낸 수수료가 끼면 모름.
    fees: Option<f64>,
    /// 전일 대비 등락률(%) — 업비트·빗썸 시세에 함께 옴. 모르면 null.
    day_change_pct: Option<f64>,
    /// 서버가 알려 준 매크로 출처(예: "웹 저장본"). 없으면 "".
    macro_origin: String,
    note: String,
    finished: Option<FinishedDto>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ChecksDto {
    auth: Check,
    ip: Check,
    order: Check,
}

impl ChecksDto {
    /// 아직 결과가 없는 세 줄(모두 기다림).
    fn waiting() -> Self {
        ChecksDto { auth: Check::Wait, ip: Check::Wait, order: Check::Wait }
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SlotDto {
    slot: KeySlot,
    exchange: Exchange,
    label: String,
    field_labels: [String; 2],
    access_filled: bool,
    secret_filled: bool,
    remembered: bool,
    remember_supported: bool,
    status: String,
    headline: String,
    note: String,
    checks: ChecksDto,
    wizard_available: bool,
    in_use_by: usize,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct StorageDto {
    status: String,
    note: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PublicIpDto {
    /// "idle"·"loading"·"ok"·"expired"·"error"
    status: String,
    address: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SettingsDto {
    slots: Vec<SlotDto>,
    member_key_filled: bool,
    /// 웹 연결이 가져온 다른 회원 키의 끝 네 글자(없으면 빈 문자열) — 키 자체는 보내지 않음.
    member_key_pending: String,
    member_in_use_by: usize,
    storage: StorageDto,
    show_dev_log: bool,
    close_to_tray: bool,
    server_base: String,
    /// 기본 운영 서버가 아닌 곳(개발 서버·환경 변수)에 연결됨 — 정보 화면에 서버 주소를 보여 줌.
    server_custom: bool,
    /// "macOS 15.6.1 (arm64)" 등 — 정보 화면·문의용 정보.
    os: String,
    /// 공인 IP(정보 화면) — 연결 도우미와 같은 조회 결과.
    public_ip: PublicIpDto,
    /// 서버 연동 레벨(`runner_core::SERVER_LINK_LEVEL`, "19").
    server_link_level: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct LibraryFileDto {
    file_name: String,
    valid: bool,
    error: String,
    title: String,
    exchange: Option<Exchange>,
    exchange_label: String,
    symbol: String,
    side: String,
    summary: String,
    strategy: String,
    unsupported: String,
    signed: bool,
    modified_at: i64,
    in_use: usize,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct LibraryDto {
    dir: String,
    files: Vec<LibraryFileDto>,
    /// 매크로 형식 이름(.ggm.json)이 아니라 목록에서 뺀 .json 수.
    skipped: usize,
    /// 너무 많아 목록에 올리지 않은 수.
    truncated: usize,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct UpdateDto {
    status: String,
    current: String,
    version: String,
    notes: String,
    progress: f64,
    downloaded: u64,
    total: u64,
    error: String,
    checked_at: i64,
    blocked: String,
    updated_from: String,
}

#[derive(Serialize)]
pub(crate) struct IpDto {
    status: String,
    address: String,
    note: String,
    changed: bool,
}

#[derive(Serialize)]
pub(crate) struct SlotsDto {
    access: bool,
    secret: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct WizardDto {
    exchange: Exchange,
    /// 키를 넣는 칸(바이낸스는 실거래·테스트넷·데모).
    slot: KeySlot,
    /// 바이낸스 사이트(testnet·demo·live). 국내는 "".
    env: String,
    market: Market,
    /// 도우미 이름에 쓰는 사이트 이름(바이낸스 테스트넷·바이낸스 데모·바이낸스·업비트·빗썸).
    label: String,
    /// 머리줄 '… 열기' 단추의 이름.
    home_label: String,
    stage: String,
    path: String,
    ip: IpDto,
    slots: SlotsDto,
    field_labels: [String; 2],
    checks: ChecksDto,
    headline: String,
    /// 세 줄 오른쪽 글자(바이낸스). 빈 값은 화면의 기본 글자.
    notes: CheckNotes,
    /// 키 칸 두 개가 다 찼는지(복사로 받았든 직접 넣었든).
    keys_ready: bool,
    reason: String,
    problem: String,
    watching_clipboard: bool,
    remembered: bool,
    remember_supported: bool,
}

#[derive(Serialize)]
pub(crate) struct StatusDto {
    text: String,
    tone: String,
}
