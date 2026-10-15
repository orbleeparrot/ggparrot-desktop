//! 화면에 보낼 상태 스냅샷 — Core를 camelCase DTO(src/types.ts v2)로 그림.

use super::*;

impl Core {
    pub fn render(&self, version: &str) -> AppStateDto {
        self.render_with(version, true)
    }

    /// `library`가 거짓이면 매크로 폴더 목록을 빼고 그림(화면은 지난 목록을 그대로 씀).
    pub fn render_with(&self, version: &str, library: bool) -> AppStateDto {
        let running = self.running_count();
        // 확인이 필요한 실행(실행 중 심각한 저하·오류로 끝남)과 잠깐 불안정한 실행을 따로 셈.
        let attention = self.runs.iter().filter(|r| r.needs_attention()).count();
        let shaky = self.runs.iter().filter(|r| r.degraded().is_some_and(|(_, tone)| tone == "warn")).count();
        let mut parts = vec![];
        if self.claim_busy {
            parts.push("웹 연결 확인 중…".to_string());
        }
        if running > 0 {
            parts.push(format!("실행 중 {running}개"));
        }
        if attention > 0 {
            parts.push(format!("확인 필요 {attention}개"));
        } else if shaky > 0 {
            parts.push("연결 다시 시도하는 중".to_string());
        }
        let status = if !parts.is_empty() {
            let tone = if attention > 0 {
                "danger"
            } else if shaky > 0 || self.claim_busy {
                "warn"
            } else {
                "ok"
            };
            StatusDto { text: parts.join(" · "), tone: tone.into() }
        } else if self.runs.is_empty() {
            StatusDto { text: "매크로를 추가하세요".into(), tone: "muted".into() }
        } else {
            StatusDto { text: "대기 중".into(), tone: "muted".into() }
        };
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
            status,
            claiming: self.claim_busy,
        }
    }

    pub(crate) fn render_run(&self, r: &Run) -> RunDto {
        let m = &r.macro_;
        let name = r.exchange_name();
        let ex = r.exchange();
        let side = macro_side(m);
        let lev = macro_leverage(m).unwrap_or(1);
        let symbol = macro_symbol(m);
        let quote = quote_of(&symbol);
        let signature = if r.source == "web" {
            "web"
        } else if r.modified || (r.sig.is_some() && r.original_capital.is_some()) {
            "modified"
        } else if r.sig.is_some() {
            "signed"
        } else {
            "unsigned"
        };
        let phase = if r.running() {
            if r.stopping {
                "stopping"
            } else {
                "running"
            }
        } else if r.start_pending && r.cancel_requested.is_some() {
            "stopping"
        } else if r.start_pending {
            "starting"
        } else if r.checking {
            "checking"
        } else if r.data.as_ref().is_some_and(|d| d.finished.is_some()) {
            "finished"
        } else {
            "idle"
        };
        let (can_start, blocker, needs) = self.readiness(r);
        let status = match phase {
            "running" => match r.degraded() {
                Some((what, tone)) => (format!("실행 중 · {what}"), tone),
                None => ("실행 중".to_string(), "ok"),
            },
            "stopping" => ("종료 처리 중…".to_string(), "warn"),
            "starting" => ("시작하는 중…".to_string(), "warn"),
            "checking" => ("연결 확인 중…".to_string(), "warn"),
            "finished" => {
                let (st, note, _) = r.data.as_ref().and_then(|d| d.finished.clone()).unwrap_or_default();
                let tone = if st == "error" { "danger" } else { "muted" };
                let what = if note.is_empty() { finish_label(&st).to_string() } else { note };
                (format!("종료됨 · {what}"), tone)
            }
            _ => {
                if can_start {
                    ("대기 중".to_string(), "muted")
                } else {
                    ("시작 전 확인 필요".to_string(), "warn")
                }
            }
        };
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
                signature: signature.into(),
                strategy: runner_core::macro_info::rule_type(m)
                    .map(runner_core::macro_info::strategy_label)
                    .unwrap_or_default()
                    .into(),
                capital: runner_core::macro_info::initial_capital(m),
                capital_original: r.original_capital,
                capital_locked: if r.source == "web" || r.user_macro_id.is_some() {
                    "웹에서 연결한 매크로는 여기서 투자 금액을 바꿀 수 없어요. 웹에서 고친 뒤 다시 연결하세요.".into()
                } else {
                    String::new()
                },
                order_cap: runner_core::types::order_cap(quote),
                risk: runner_core::macro_info::risk_summary(m),
            },
            mode: r.mode,
            mode_choices: r.mode_choices(),
            phase: phase.into(),
            key_slot: r.key_slot(),
            readiness: ReadinessDto { can_start, blocker, needs },
            run: r.data.as_ref().map(|d| render_run_data(d, side)),
            web_linked: r.web_linked,
            status: StatusDto { text: status.0, tone: status.1.into() },
        }
    }

    /// (시작 가능, 막힌 이유, 설정으로 데려갈 곳)
    pub(crate) fn readiness(&self, r: &Run) -> (bool, String, Option<NeedsDto>) {
        if r.running() || r.start_pending || r.checking {
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
            let slot = KeySlot::for_run(ex, r.mode);
            if self.slot_pair(slot).is_none() {
                return (
                    false,
                    runner_core::types::exchange_key_needed_message(slot.label()),
                    Some(NeedsDto::Key { slot }),
                );
            }
        }
        if self.member_key.trim().is_empty() {
            return (false, "회원 키가 필요해요. 설정에서 입력하세요.".into(), Some(NeedsDto::Member));
        }
        if self.claim_busy {
            return (false, "웹 매크로 연결이 끝날 때까지 잠시만 기다려 주세요.".into(), None);
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
            protocol_version: PROTOCOL_VERSION.into(),
        }
    }

    pub(crate) fn render_slot(&self, s: KeySlot) -> SlotDto {
        let st = self.slot(s);
        let ex = s.exchange();
        let (status, headline, mut note) = if self.slot_pair(s).is_none() {
            ("empty", "● 연결 안 됨".to_string(), String::new())
        } else if self.slot_busy(s) {
            ("checking", "● 확인 중".to_string(), String::new())
        } else if let Some(c) = self.current_slot_check(s) {
            if c.view.connected {
                let n = if c.view.problem == "order_size" {
                    c.reason.clone()
                } else if now_ms() - c.checked_at > KEY_CHECK_STALE_MS {
                    "확인한 지 30분이 넘었어요. 매크로를 시작할 때 다시 확인해요.".to_string()
                } else {
                    String::new()
                };
                ("connected", "✓ 연결됨".to_string(), n)
            } else if ex.is_domestic() {
                ("blocked", format!("● {}", c.view.headline), String::new())
            } else {
                ("blocked", "● 연결 안 됨".to_string(), c.reason.clone())
            }
        } else {
            ("unchecked", "● 확인 전".to_string(), String::new())
        };
        if note.is_empty() {
            let failure = self.load_failure_note();
            note = if !failure.is_empty() { failure } else { self.storage_error.clone() };
        }
        let checks = self
            .current_slot_check(s)
            .map(|c| ChecksDto { auth: c.view.auth, ip: c.view.ip, order: c.view.order })
            .unwrap_or(ChecksDto { auth: Check::Wait, ip: Check::Wait, order: Check::Wait });
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
            wizard_available: ex.is_domestic(),
            in_use_by: self
                .runs
                .iter()
                .filter(|r| r.running() && r.key_slot() == Some(s) && r.mode != RunMode::Mock)
                .count(),
        }
    }

    pub(crate) fn render_wizard(&self) -> Option<WizardDto> {
        let w = self.wizard.as_ref()?;
        let result = self.ip_cache.result();
        let busy = self.ip_cache.busy();
        let (ip_status, address) = if busy {
            ("loading", String::new())
        } else {
            match result {
                Some(r) if r.ok && r.fresh() => ("ok", r.address.clone()),
                Some(r) if r.ok => ("expired", String::new()),
                Some(_) => ("error", String::new()),
                None => ("idle", String::new()),
            }
        };
        let (state, view, reason) = self.wizard_check(w.slot);
        let show_view = matches!(w.stage.as_str(), "result_ok" | "result_fail") || state == "done";
        let ex = w.slot.exchange();
        let (a, s) = ex.field_labels();
        Some(WizardDto {
            exchange: ex,
            label: ex.label().into(),
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
                ChecksDto { auth: Check::Wait, ip: Check::Wait, order: Check::Wait }
            },
            headline: if show_view { view.headline.clone() } else { String::new() },
            reason,
            problem: if show_view { view.problem.clone() } else { String::new() },
            watching_clipboard: WATCH_STAGES.contains(&w.stage.as_str()),
            remembered: self.slot(w.slot).remember,
            remember_supported: self.remember_supported,
        })
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
    RunStatusDto {
        started_at: d.started_at,
        session_id: d.session_id,
        online: d.online && d.health.online,
        auth_failed: d.health.auth_failed,
        rejected: d.health.rejected,
        offline_since: d.health.offline_since as i64,
        price_stale_since: d.health.price_stale_since as i64,
        close_failures: d.health.close_failures,
        updated_at: d.updated_at,
        price,
        in_position,
        uncertain: d.uncertain(),
        entry_price: entry,
        qty,
        unrealized_pct: f("unrealized_pct"),
        unrealized: if in_position && price > 0.0 && entry > 0.0 { pnl_amount(qty, entry, price, side) } else { 0.0 },
        realized: f("realized_pnl"),
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
    /// 이 앱이 로컬에서 지키는 안전 설정(예: "손절 3%"). 없으면 [].
    risk: Vec<String>,
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
    updated_at: i64,
    price: f64,
    in_position: bool,
    uncertain: bool,
    entry_price: f64,
    qty: f64,
    unrealized_pct: f64,
    unrealized: f64,
    realized: f64,
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
    protocol_version: String,
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
    label: String,
    stage: String,
    path: String,
    ip: IpDto,
    slots: SlotsDto,
    field_labels: [String; 2],
    checks: ChecksDto,
    headline: String,
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
