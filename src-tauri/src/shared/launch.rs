//! 시작 — 실전 확인·서버 세션 만들기·봇 띄우기.

use super::*;

/// 확인 창이 떠 있거나 시작하는 동안 설정이 바뀌어 시작하지 않을 때.
const PREFLIGHT_EXPIRED: &str = "확인 중 설정이 바뀌었거나 확인 유효 시간이 지났어요";
const PRESS_START_AGAIN: &str = "[시작]을 다시 누르면 새로 확인해요.";

/// 서버 세션을 만들고 봇을 띄울 때 쓰는 값 — [`Core::launch_job`]이 잠금 안에서 한 번 떠 둠.
pub(crate) struct LaunchJob {
    pub id: String,
    pub macro_: Value,
    pub mode: RunMode,
    pub creds: Credentials,
    /// 시작 전 확인 지문 — 시작 요청 동안 설정이 바뀌었는지 비교함.
    pub fp: String,
    /// 서버 시작 요청 본문.
    pub payload: Value,
    pub base: String,
    pub generation: u64,
    pub preflight: PreflightResult,
    /// 묶음의 한 레그로 시작하는 경우 그 자리표. 단일 종목은 None — 그 길은 한 글자도 안 바뀜.
    pub leg: Option<RunBundle>,
}

/// 묶음 한도가 걸린 묶음 — 서버도 같은 이유로 거절한다(app/runner_legs.py).
pub(crate) const BUNDLE_RISK_HEAD: &str = "이 묶음은 아직 돌릴 수 없어요";
pub(crate) const BUNDLE_RISK_BODY: &str =
    "묶음 한도(한 번에 n종목 · 총 노출)를 건 묶음은 종목마다 갈라 돌릴 수 없어요. 한도를 빼고 다시 시작해 주세요.";

/// 세션을 만든 뒤 봇을 띄우지 않고 끝내는 이유.
#[derive(Debug, PartialEq)]
pub(crate) enum LaunchCancel {
    /// 시작하는 중에 사용자가 종료를 누름.
    User,
    /// 설정·판이 바뀌었거나 앱을 닫는 중·웹 연결 중·같은 종목이 먼저 시작함.
    Changed,
}

/// 봇을 붙인 뒤 웹 가이드에 '시작함'을 알릴 곳.
pub(crate) struct BotAttached {
    reporter: Option<Arc<SetupProgressReporter>>,
    env: &'static str,
}

impl Shared {
    pub fn start_run(self: &S, id: &str) -> StartReply {
        let blocked = |m: &str| StartReply::Blocked { message: m.into() };
        let mut c = self.lock();
        if let Some(stop) = c.start_gate(id) {
            return stop;
        }
        let run = c.run(id).unwrap();
        let ex = run.exchange().unwrap();
        let label = ex.label();
        if run.mode == RunMode::Testnet && !ex.has_testnet() {
            let r = c.run_mut(id).unwrap();
            r.mode = RunMode::Mock;
            r.preflight = None;
            c.push_activity(
                id,
                "info",
                &format!("{label}에는 테스트넷이 없어 모의로 바꿨어요"),
                "",
                "",
                false,
                "muted",
            );
        }
        let run = c.run(id).unwrap();
        let slot = KeySlot::for_run(ex, run.mode, run.market());
        if run.mode != RunMode::Mock && c.slot_pair(slot).is_none() {
            // 키가 없으면 오류 창 대신 연결 도우미 — '키를 발급받은 적 있나요?'부터 이어서 마침.
            if let Some(target) = run.wizard_target() {
                drop(c);
                self.wizard_open(target, "ask");
                return StartReply::Wizard;
            }
            return blocked(&runner_core::types::exchange_key_needed_message(slot.label()));
        }
        if c.member_key.trim().is_empty() {
            return blocked(MEMBER_KEY_NEEDED);
        }
        let creds = c.run_credentials(run);
        let fp = c.run_fingerprint(run);
        let (mode, macro_) = (run.mode, run.macro_.clone());
        if mode == RunMode::Mock {
            let checked = mock_check(&macro_, &creds);
            if !checked.ok {
                return blocked(&checked.reason);
            }
            c.run_mut(id).unwrap().preflight = Some(checked);
        } else if !run.preflight.as_ref().is_some_and(|p| p.matches(&fp)) {
            drop(c);
            self.begin_run_check(id, true);
            return StartReply::Checking;
        }
        if mode == RunMode::Live {
            let side = macro_side(&macro_).as_str();
            let symbol = macro_.get("symbol").and_then(Value::as_str).unwrap_or("");
            let side = if side == "short" { "숏" } else { "롱" };
            let mut message = format!("실제 자금으로 주문해요.\n{symbol} · {side}");
            if let Some(why) = c.run(id).and_then(leftover_warning) {
                message.push_str(&format!(
                    "\n\n{why} 이 앱은 보유 없음으로 시작하므로 남은 포지션에는 손절이 걸리지 않아요. 거래소에서 먼저 확인하세요."
                ));
            }
            if let Some(r) = c.run_mut(id) {
                r.live_confirm_issued = true;
            }
            return StartReply::ConfirmLive { title: format!("{label} 실전으로 시작할까요?"), message };
        }
        drop(c);
        self.launch(id)
    }

    /// 실전 확인 뒤. 확인 창이 떠 있는 동안 설정이 바뀌었으면 시작하지 않음.
    pub fn start_run_confirmed(self: &S, id: &str) -> StartReply {
        {
            let mut c = self.lock();
            let blocked = |m: &str| StartReply::Blocked { message: m.into() };
            let Some(run) = c.run(id) else { return blocked("매크로를 찾지 못했어요.") };
            if c.closed {
                return blocked("앱을 닫는 중이라 시작하지 않았어요.");
            }
            if c.claim_busy {
                return blocked(CLAIM_BUSY_WAIT);
            }
            if run.busy() {
                return StartReply::Checking;
            }
            // 이 실행에 실전 확인을 띄운 적이 있어야 함(한 번만 씀).
            if !run.live_confirm_issued {
                return blocked("실전 시작 확인을 다시 받아야 해요. [실전으로 시작]을 다시 눌러 주세요.");
            }
            c.run_mut(id).unwrap().live_confirm_issued = false;
            let run = c.run(id).unwrap();
            if c.conflicting_run(run).is_some() {
                return StartReply::Blocked { message: Core::conflict_message(run) };
            }
            let fp = c.run_fingerprint(run);
            if run.mode != RunMode::Live || !run.preflight.as_ref().is_some_and(|p| p.matches(&fp)) {
                c.run_mut(id).unwrap().invalidate_preflight();
                let full = format!("{PREFLIGHT_EXPIRED}. {PRESS_START_AGAIN}");
                c.push_activity(id, "info", PREFLIGHT_EXPIRED, PRESS_START_AGAIN, &full, false, "warn");
                drop(c);
                self.emit();
                return StartReply::Blocked { message: full };
            }
        }
        self.launch(id)
    }

    /// 서버에 세션을 만들고 봇을 띄움. 묶음이면 레그마다 한 줄씩 갈라 띄움.
    pub(crate) fn launch(self: &S, id: &str) -> StartReply {
        let legs = {
            let c = self.lock();
            c.run(id).map(|r| macro_leg_symbols(&r.macro_)).unwrap_or_default()
        };
        if legs.len() > 1 {
            return self.launch_bundle(id, legs);
        }
        let job = match self.lock().launch_job(id) {
            Ok(job) => job,
            Err(reply) => return reply,
        };
        self.emit();
        let me = self.clone();
        tauri::async_runtime::spawn(async move { me.launch_async(job).await });
        StartReply::Started
    }

    /// 묶음 — 레그마다 **줄 하나 · 세션 하나 · 봇 하나**. 레그 하나는 지금까지 돌려 온
    /// 단일 종목 실행과 똑같다.
    ///
    /// 서버에는 **원본 매크로를 그대로** 보내고 몇 번째 레그인지만 덧붙인다. 여기서
    /// 쪼개 보내면 서명이 깨져 모든 묶음이 "고쳐서 돌린 파일" 로 기록된다. 봇을 세울
    /// 레그 매크로는 시작 응답이 준다 — 자금 비중 셈을 양쪽에서 하면 레그 금액의 합이
    /// 묶음 금액과 어긋난다.
    pub(crate) fn launch_bundle(self: &S, id: &str, legs: Vec<String>) -> StartReply {
        let base_job = match self.lock().launch_job(id) {
            Ok(job) => job,
            Err(reply) => return reply,
        };
        if macro_has_bundle_risk(&base_job.macro_) {
            self.lock().start_failed(id, BUNDLE_RISK_HEAD, BUNDLE_RISK_BODY);
            self.emit();
            return StartReply::Blocked { message: format!("{BUNDLE_RISK_HEAD}
{BUNDLE_RISK_BODY}") };
        }
        let key = format!("bun-{}", &uuid::Uuid::new_v4().simple().to_string()[..12]);
        let size = legs.len();
        // 부모 줄은 여기서 '시작하는 중'을 푼다 — 실제로 도는 것은 아래 레그 줄들이다.
        {
            let mut c = self.lock();
            if let Some(r) = c.run_mut(id) {
                r.start_pending = false;
            }
            c.push_activity(
                id,
                "info",
                "묶음을 나눠 시작해요",
                &format!("{size}종목을 종목마다 한 줄씩 돌려요."),
                &format!("묶음 {size}종목 — 종목마다 한 줄씩 시작해요: {}", legs.join(" · ")),
                false,
                "info",
            );
        }
        for (index, symbol) in legs.iter().enumerate() {
            let bundle = RunBundle { key: key.clone(), index, size };
            let leg_id = match self.add_leg_run(&base_job, symbol, &bundle) {
                Some(leg_id) => leg_id,
                None => continue,
            };
            let mut payload = base_job.payload.clone();
            payload["leg_index"] = json!(index);
            payload["bundle_key"] = json!(key);
            // 요청의 종목도 레그로 — 서버가 요청과 매크로가 어긋나면 막는다.
            payload["symbol"] = json!(symbol);
            let job = LaunchJob {
                id: leg_id,
                payload,
                leg: Some(bundle),
                macro_: base_job.macro_.clone(),
                mode: base_job.mode,
                creds: base_job.creds.clone(),
                fp: base_job.fp.clone(),
                base: base_job.base.clone(),
                generation: base_job.generation,
                preflight: base_job.preflight.clone(),
            };
            let me = self.clone();
            tauri::async_runtime::spawn(async move { me.launch_async(job).await });
        }
        self.emit();
        StartReply::Started
    }

    /// 레그 한 줄을 목록에 더한다. 매크로는 일단 원본 — 시작 응답이 레그 매크로를 주면
    /// 그때 바꾼다(`launch_async`). 제목만 레그 종목으로 둬 목록이 헷갈리지 않게 한다.
    fn add_leg_run(self: &S, base: &LaunchJob, symbol: &str, bundle: &RunBundle) -> Option<String> {
        let leg_id = format!("run-{}", &uuid::Uuid::new_v4().simple().to_string()[..12]);
        let mut c = self.lock();
        let parent = c.run(&base.id)?;
        let mut run = Run::new(
            leg_id.clone(),
            base.macro_.clone(),
            format!("{} · {}/{}", symbol, bundle.index + 1, bundle.size),
            &parent.source.clone(),
            base.base.clone(),
        );
        run.sig = parent.sig.clone();
        run.user_macro_id = parent.user_macro_id;
        run.library_file = parent.library_file.clone();
        run.web_linked = parent.web_linked;
        run.mode = base.mode;
        run.preflight = Some(base.preflight.clone());
        run.bundle = Some(bundle.clone());
        run.start_pending = true;
        c.runs.push(run);
        Some(leg_id)
    }

    /// 세션 시작 → (실패면 알림) → 세션 기록 → 그사이 바뀌었으면 세션만 취소 → 봇 띄우기.
    pub(crate) async fn launch_async(self: S, job: LaunchJob) {
        let exchange = runner_core::types::exchange_name_of(&job.macro_);
        let label = exchange_label(&exchange);
        let server = ServerClient::new(&job.creds.member_key, &job.base, self.outbox.clone(), self.http.clone());
        let started = match server.start(&job.payload).await {
            Ok(v) => v,
            Err(err) => {
                let (head, msg) = start_error_text(&err, &job.creds);
                self.lock().start_failed(&job.id, head, &msg);
                self.emit();
                self.notice_from("server", "error", format!("{head}:\n{msg}"));
                return;
            }
        };
        let sid = server.session_id();
        // 서버가 알려 준 매크로 출처 — 원문 로그와 상세 머리줄에 씀.
        let origin = started.get("macro_origin_label").and_then(Value::as_str).unwrap_or("");
        let digest = started.get("macro_digest").and_then(Value::as_str).unwrap_or("");
        let file_modified = server.macro_origin() == "file_modified";
        self.lock().note_session_start(&job, &label, sid, origin, digest, file_modified);
        // 시작 요청 동안 설정이 바뀌었으면 방금 만든 세션만 취소함(봇은 만들지 않음).
        let cancel = self.lock().launch_cancel(&job);
        if let Some(why) = cancel {
            let notified = match why {
                LaunchCancel::User => {
                    server.stopped("stopped", "시작하는 중에 사용자가 종료해 주문 없이 끝냈어요.", None).await
                }
                LaunchCancel::Changed => {
                    server
                        .stopped(
                            "error",
                            "시작하는 중에 설정이 바뀌거나 확인 유효 시간이 지나 주문 없이 취소했어요.",
                            None,
                        )
                        .await
                }
            };
            self.lock().launch_cancelled(&job.id, why, notified);
            self.emit();
            return;
        }
        // 묶음이면 봇이 돌릴 것은 **서버가 준 레그 매크로**다 — 비중 셈은 서버가 한다.
        let leg_macro = started.get("leg_macro").filter(|v| v.is_object()).cloned();
        let LaunchJob { id, macro_, mode, creds, preflight, .. } = job;
        let macro_ = leg_macro.unwrap_or(macro_);
        if let Some(r) = self.lock().run_mut(&id) {
            r.macro_ = macro_.clone();
        }
        let symbol = runner_core::types::macro_symbol(&macro_);
        let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
        let handle = spawn_bot(macro_, creds, mode, server, Some(preflight), self.http.clone(), tx);
        let (attached, saved) = {
            let mut c = self.lock();
            let Some(attached) = c.attach_bot(&id, mode, handle, sid, origin, &label, &symbol) else { return };
            // 실행 중이라는 표시를 저장해 둠 — 앱이 도중에 꺼지면 다음에 켤 때 알 수 있게.
            (attached, self.save_runs(&c))
        };
        self.warn_save_failed(saved);
        if let Some(rep) = attached.reporter {
            if let Ok(p) = GuideStep::bare("started", &exchange, attached.env).payload() {
                rep.report(p);
            }
            rep.close();
        }
        self.emit();
        self.pump_bot_events(id, rx);
    }
}

/// 지난 실행이 포지션을 남겼을 수 있으면 그 이유. 이 앱은 시작할 때 보유를 0으로 보므로 실전 확인에 덧붙임.
pub(crate) fn leftover_warning(r: &Run) -> Option<String> {
    let d = r.data.as_ref()?;
    let (_, note, _) = d.finished.as_ref()?;
    if note == crate::state::INTERRUPTED_NOTE {
        return Some("지난 실행이 정상적으로 끝나지 않아 남은 포지션을 이 앱이 몰라요.".into());
    }
    (d.holding() || d.uncertain()).then(|| format!("지난 실행이 포지션을 남긴 채 끝났어요({note})."))
}

const MEMBER_KEY_REJECTED: &str = "회원 키가 맞지 않아요. 설정 › 껄무새 계정에서 확인하세요.";

/// 세션 시작 실패를 (제목, 설명)으로 — 키 값은 가림.
pub(crate) fn start_error_text(err: &StartError, creds: &Credentials) -> (&'static str, String) {
    let msg = match err {
        StartError::Server(e) if e.status == 401 => MEMBER_KEY_REJECTED.to_string(),
        StartError::Server(e) if e.status == 429 || e.status == 503 => format!(
            "{}\n\n몇 번 다시 보냈지만 서버가 계속 바빠요. 잠시 뒤 다시 시작해 주세요.",
            connection::safe_preflight_reason(&e.to_string(), creds)
        ),
        StartError::Server(e) => connection::safe_preflight_reason(&e.to_string(), creds),
        // 네트워크 오류 원문(주소·영어 파싱 오류)은 보이지 않음. 서버 연결 문제라 거래소 키 안내도 하지 않음.
        StartError::Network(e) if e.starts_with(runner_core::server::START_TIMEOUT_TAG) => {
            "서버 응답이 늦어 시작하지 못했어요. 서버에는 실행이 만들어졌을 수 있어요 — 웹의 [내 에이전트]에 실행 중으로 보이면 웹에서 종료한 뒤 다시 시작하세요.".into()
        }
        StartError::Network(_) => "서버에 연결하지 못했어요. 인터넷 연결을 확인한 뒤 다시 시작하세요.".into(),
    };
    let head = if matches!(err, StartError::Server(_)) {
        "매크로를 시작하지 못했어요"
    } else {
        "서버에 연결하지 못했어요"
    };
    (head, msg)
}

impl Core {
    /// 시작할 값을 떠 두고 '시작하는 중'으로 표시함. 시작하지 않으면 그때의 답.
    pub(crate) fn launch_job(&mut self, id: &str) -> Result<LaunchJob, StartReply> {
        let Some(run) = self.run(id) else {
            return Err(StartReply::Blocked { message: "매크로를 찾지 못했어요.".into() });
        };
        // 같은 매크로를 두 번 띄우지 않음(검사 끝 자동 시작과 [시작]이 겹칠 때).
        if self.closed || run.start_pending || run.running() {
            return Err(StartReply::Checking);
        }
        let Some(preflight) = run.preflight.clone() else {
            return Err(StartReply::Blocked {
                message: "시작 전 확인 결과가 없어요. [시작]을 다시 눌러 주세요.".into(),
            });
        };
        let mut payload = session_payload(&run.macro_, run.mode != RunMode::Live, run.mode);
        if let Some(uid) = run.user_macro_id {
            payload["user_macro_id"] = json!(uid);
        }
        if let Some(sig) = &run.sig {
            payload["macro_sig"] = sig.clone();
        }
        payload["macro_source"] = json!(run.source);
        let job = LaunchJob {
            id: id.to_string(),
            macro_: run.macro_.clone(),
            mode: run.mode,
            creds: self.run_credentials(run),
            fp: self.run_fingerprint(run),
            payload,
            base: run.server_base.clone(),
            generation: run.generation,
            preflight,
            leg: None,
        };
        let r = self.run_mut(id).unwrap();
        r.start_pending = true;
        r.cancel_requested = None;
        Ok(job)
    }

    /// 서버가 세션 시작을 거절했거나 닿지 않음 — '시작하는 중'을 풀고 활동에 남김.
    pub(crate) fn start_failed(&mut self, id: &str, head: &str, msg: &str) {
        if let Some(r) = self.run_mut(id) {
            r.start_pending = false;
            r.cancel_requested = None;
        }
        self.push_activity(id, "error", head, msg, "", false, "danger");
    }

    /// 만든 세션을 원문 로그에 남김. 웹 원본과 다른 파일이면 '수정됨'으로 표시하고 알림.
    pub(crate) fn note_session_start(
        &mut self,
        job: &LaunchJob,
        label: &str,
        sid: Option<i64>,
        origin: &str,
        digest: &str,
        file_modified: bool,
    ) {
        let id = job.id.as_str();
        let raw = format!(
            "세션 시작 (id={}) · {label} · {} · {} · {}",
            sid.map(|s| s.to_string()).unwrap_or_else(|| "None".into()),
            job.payload["symbol"].as_str().unwrap_or(""),
            job.payload["market"].as_str().unwrap_or(""),
            job.mode.label()
        );
        self.push_raw(id, &raw);
        if !origin.is_empty() {
            let digest = if digest.is_empty() { String::new() } else { format!(" · 지문 {digest}") };
            self.push_raw(id, &format!("매크로 출처: {origin}{digest}"));
        }
        if file_modified {
            if let Some(r) = self.run_mut(id) {
                r.modified = true;
            }
            self.push_activity(
                id,
                "info",
                "웹에서 받은 원본과 다른 파일이에요",
                "이 기기에서 고친 설정 그대로 실행해요. 껄무새 웹의 내 에이전트에는 '수정된 파일'로 표시돼요.",
                "⚠ 이 매크로 파일은 웹에서 받은 원본과 달라요. 이 기기에서 고친 설정으로 실행돼요.",
                false,
                "warn",
            );
        }
    }

    /// 세션을 만든 뒤 봇을 띄우지 않을 이유. 띄워도 되면 None.
    pub(crate) fn launch_cancel(&self, job: &LaunchJob) -> Option<LaunchCancel> {
        let Some(r) = self.run(&job.id) else { return Some(LaunchCancel::Changed) };
        if r.cancel_requested.is_some() {
            return Some(LaunchCancel::User);
        }
        let changed = self.closed
            || self.claim_busy
            || r.generation != job.generation
            || self.run_fingerprint(r) != job.fp
            || self.member_key.trim() != job.creds.member_key
            || self.conflicting_run(r).is_some();
        changed.then_some(LaunchCancel::Changed)
    }

    /// 세션만 취소하고 끝냄 — '시작하는 중'을 풀고 시작 전 확인도 버림. `notified`는 서버에 알렸는지.
    pub(crate) fn launch_cancelled(&mut self, id: &str, why: LaunchCancel, notified: bool) {
        if let Some(r) = self.run_mut(id) {
            r.start_pending = false;
            r.cancel_requested = None;
            r.invalidate_preflight();
        }
        match why {
            LaunchCancel::User => self.push_activity(
                id,
                "stop",
                "시작을 취소했어요",
                "주문은 하나도 내지 않았어요.",
                "시작하는 중에 종료해 주문 없이 끝냈어요.",
                false,
                "muted",
            ),
            LaunchCancel::Changed => self.push_activity(
                id,
                "error",
                "시작 중 설정이 바뀌어 매매를 시작하지 않았어요",
                "다시 시작하면 새로 확인해요.",
                "시작 설정이 바뀌어 매매를 시작하지 않았어요. [시작]을 다시 누르면 새로 확인해요.",
                false,
                "warn",
            ),
        }
        if !notified {
            self.push_activity(
                id,
                "connection",
                "서버에 취소를 알리지 못했어요",
                "껄무새 웹의 내 에이전트에서 상태를 확인하세요.",
                "",
                false,
                "warn",
            );
        }
    }

    /// 띄운 봇을 실행에 붙임. 그사이 실행이 목록에서 빠졌으면 None.
    /// 취소 확인과 봇 띄우기 사이에 누른 종료도 바로 봇에 넘김.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn attach_bot(
        &mut self,
        id: &str,
        mode: RunMode,
        handle: BotHandle,
        sid: Option<i64>,
        origin: &str,
        label: &str,
        symbol: &str,
    ) -> Option<BotAttached> {
        let r = self.run_mut(id)?;
        let env = r.binance_env();
        let mut data = RunData::started(now_ms(), sid);
        // 서버가 알려 준 매크로 출처 — 상세 머리줄에 보임.
        data.macro_origin = origin.trim().chars().take(60).collect();
        r.data = Some(data);
        let late_cancel = r.cancel_requested.take();
        if let Some(stop) = late_cancel {
            handle.command(stop);
        }
        r.bot = Some(handle);
        r.stopping = late_cancel.is_some();
        r.start_pending = false;
        r.preflight = None;
        let reporter = r.reporter.take();
        self.push_activity(
            id,
            "start",
            &format!("실행을 시작했어요 · {}", mode.label()),
            &format!("{label} · {symbol}"),
            "",
            false,
            "ok",
        );
        Some(BotAttached { reporter, env })
    }

    /// 키·연결 확인 전에 시작을 멈출 이유. 멈추지 않으면 None — 그때는 매크로가 있고 거래소도 앎.
    /// 앱 상태(창·서버)가 없어도 판단할 수 있게 [`Shared::start_run`]에서 떼어 냄.
    pub(crate) fn start_gate(&self, id: &str) -> Option<StartReply> {
        let blocked = |m: &str| Some(StartReply::Blocked { message: m.into() });
        if self.closed {
            return Some(StartReply::Checking);
        }
        let Some(run) = self.run(id) else { return blocked("매크로를 찾지 못했어요.") };
        if run.start_pending || run.checking {
            return Some(StartReply::Checking);
        }
        if run.running() {
            return blocked("이미 실행 중이에요.");
        }
        if self.claim_busy {
            return blocked(CLAIM_BUSY_WAIT);
        }
        // 목록에 이미 있던 매크로도 서버가 거절할 유형이면 서버에 보내기 전에 막음(화면의 막힌 이유와 같은 문구).
        if let Some(why) = runner_core::macro_info::unsupported_reason(&run.macro_) {
            return blocked(why);
        }
        if run.exchange().is_none() {
            return blocked(&runner_core::types::unsupported_exchange_message(&run.exchange_name()));
        }
        if self.conflicting_run(run).is_some() {
            return blocked(&Core::conflict_message(run));
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use runner_core::server::ServerRequestError;

    #[test]
    fn start_errors_read_clearly_and_hide_keys() {
        let creds = Credentials { member_key: "ggp_member_secret_value".into(), ..Default::default() };
        let server =
            |status: u16, detail: &str| StartError::Server(ServerRequestError { status, detail: detail.into() });
        assert_eq!(
            start_error_text(&server(401, ""), &creds),
            ("매크로를 시작하지 못했어요", MEMBER_KEY_REJECTED.into())
        );
        let (head, msg) = start_error_text(&server(503, "busy"), &creds);
        assert_eq!(head, "매크로를 시작하지 못했어요");
        assert!(msg.contains("서버가 계속 바빠요"));
        let (head, msg) =
            start_error_text(&StartError::Network("key ggp_member_secret_value timed out".into()), &creds);
        assert_eq!(head, "서버에 연결하지 못했어요");
        assert!(!msg.contains("ggp_member_secret_value"), "{msg}");
    }

    fn core_with(macros: &[(&str, Value)]) -> Core {
        let mut c = Core::new("https://example.test".into(), true, PathBuf::from("/tmp/x"));
        for (id, m) in macros {
            c.runs.push(Run::new((*id).into(), m.clone(), (*id).into(), "file", "https://example.test".into()));
        }
        c
    }

    fn blocked_text(reply: Option<StartReply>) -> String {
        match reply {
            Some(StartReply::Blocked { message }) => message,
            Some(_) => panic!("막힘이 아닌 다른 답"),
            None => panic!("막히지 않았다"),
        }
    }

    #[test]
    fn unsupported_macros_are_blocked_before_the_server() {
        let c = core_with(&[
            ("dca", json!({"symbol": "ETHUSDT", "exchange": "binance", "rule_type": "C"})),
            ("sar", json!({"symbol": "BTCUSDT", "exchange": "binance", "rule_type": "K"})),
            (
                "multi",
                json!({"symbol": "BTCUSDT", "exchange": "binance", "rule_type": "A", "symbols": ["BTCUSDT", "ETHUSDT"]}),
            ),
            ("kraken", json!({"symbol": "XBTUSD", "exchange": "kraken", "rule_type": "A"})),
        ]);
        for id in ["dca", "sar", "multi"] {
            let why = blocked_text(c.start_gate(id));
            let expected = runner_core::macro_info::unsupported_reason(&c.run(id).unwrap().macro_).unwrap();
            assert_eq!(why, expected, "{id}: 화면의 막힌 이유와 같은 문구");
        }
        assert!(blocked_text(c.start_gate("kraken")).contains("kraken"));
        assert_eq!(blocked_text(c.start_gate("nope")), "매크로를 찾지 못했어요.");
    }

    /// 확인을 마친 실전 실행 하나(바이낸스 BTCUSDT).
    fn checked_core() -> Core {
        let mut c = core_with(&[("a", json!({"symbol": "BTCUSDT", "exchange": "binance", "rule_type": "F"}))]);
        c.member_key = "member".into();
        c.slot_mut(KeySlot::Binance).keys = KeyPair { api_key: "k".into(), api_secret: "s".into() };
        let fp = {
            let r = c.run_mut("a").unwrap();
            r.mode = RunMode::Live;
            r.user_macro_id = Some(7);
            r.sig = Some(json!({"v": 4}));
            r.generation = 3;
            c.run_fingerprint(c.run("a").unwrap())
        };
        c.run_mut("a").unwrap().preflight = Some(PreflightResult::new(true, String::new(), fp));
        c
    }

    #[test]
    fn a_launch_job_takes_the_checked_settings_and_marks_starting() {
        let mut c = checked_core();
        let job = c.launch_job("a").ok().unwrap();
        assert_eq!((job.mode, job.generation, job.creds.member_key.as_str()), (RunMode::Live, 3, "member"));
        assert_eq!(job.fp, c.run_fingerprint(c.run("a").unwrap()));
        assert_eq!(
            (job.payload["user_macro_id"].clone(), job.payload["macro_source"].clone()),
            (json!(7), json!("file"))
        );
        assert_eq!(job.payload["macro_sig"], json!({"v": 4}));
        assert!(c.run("a").unwrap().start_pending);
        assert!(matches!(c.launch_job("a"), Err(StartReply::Checking)), "두 번 띄우지 않음");
        assert!(matches!(c.launch_job("nope"), Err(StartReply::Blocked { .. })));
        let mut c = checked_core();
        c.run_mut("a").unwrap().preflight = None;
        assert!(matches!(c.launch_job("a"), Err(StartReply::Blocked { message }) if message.contains("시작 전 확인")));
    }

    #[test]
    fn a_failed_start_clears_starting_and_logs_why() {
        let mut c = checked_core();
        let _job = c.launch_job("a").ok().unwrap();
        c.run_mut("a").unwrap().cancel_requested = Some(StopMode::StopOnly);
        c.start_failed("a", "서버에 연결하지 못했어요", "인터넷 연결을 확인하세요");
        let r = c.run("a").unwrap();
        assert!(!r.start_pending && r.cancel_requested.is_none());
        let last = r.activity.last().unwrap();
        assert_eq!((last.kind.as_str(), last.tone.as_str()), ("error", "danger"));
    }

    #[test]
    fn the_session_log_names_the_origin_and_marks_modified_files() {
        let mut c = checked_core();
        let job = c.launch_job("a").ok().unwrap();
        c.note_session_start(&job, "바이낸스", Some(42), "원본 파일", "ab12", true);
        let r = c.run("a").unwrap();
        assert!(r.modified, "웹 원본과 다른 파일");
        let raws: Vec<&str> = r.activity.iter().map(|a| a.raw.as_str()).collect();
        assert!(raws[0].starts_with("세션 시작 (id=42) · 바이낸스 · BTCUSDT"), "{raws:?}");
        assert_eq!(raws[1], "매크로 출처: 원본 파일 · 지문 ab12");
        let mut c = checked_core();
        let job = c.launch_job("a").ok().unwrap();
        c.note_session_start(&job, "바이낸스", None, "", "", false);
        let r = c.run("a").unwrap();
        assert!(!r.modified && r.activity.len() == 1 && r.activity[0].raw.contains("(id=None)"));
    }

    #[test]
    fn a_launch_is_cancelled_when_the_user_stops_or_settings_change() {
        let mut c = checked_core();
        let job = c.launch_job("a").ok().unwrap();
        assert_eq!(c.launch_cancel(&job), None, "그대로면 봇을 띄움");

        c.run_mut("a").unwrap().cancel_requested = Some(StopMode::StopOnly);
        c.claim_busy = true;
        assert_eq!(c.launch_cancel(&job), Some(LaunchCancel::User), "사용자 종료가 먼저");
        c.run_mut("a").unwrap().cancel_requested = None;
        assert_eq!(c.launch_cancel(&job), Some(LaunchCancel::Changed), "웹 연결 중");
        c.claim_busy = false;

        c.member_key = "other".into();
        assert_eq!(c.launch_cancel(&job), Some(LaunchCancel::Changed), "회원 키가 바뀜");
        c.member_key = "member".into();
        c.slot_mut(KeySlot::Binance).keys.api_key = "k2".into();
        assert_eq!(c.launch_cancel(&job), Some(LaunchCancel::Changed), "키가 바뀜");
        c.slot_mut(KeySlot::Binance).keys.api_key = "k".into();
        c.run_mut("a").unwrap().generation += 1;
        assert_eq!(c.launch_cancel(&job), Some(LaunchCancel::Changed), "판이 바뀜");
        c.runs.clear();
        assert_eq!(c.launch_cancel(&job), Some(LaunchCancel::Changed), "목록에서 빠짐");

        let mut c = checked_core();
        let _job = c.launch_job("a").ok().unwrap();
        c.launch_cancelled("a", LaunchCancel::User, false);
        let r = c.run("a").unwrap();
        assert!(!r.start_pending && r.preflight.is_none() && r.generation == 4, "다음 시작은 새로 확인");
        let titles: Vec<&str> = r.activity.iter().map(|a| a.title.as_str()).collect();
        assert_eq!(titles, ["시작을 취소했어요", "서버에 취소를 알리지 못했어요"]);
    }

    #[tokio::test]
    async fn an_attached_bot_runs_and_a_late_stop_reaches_it() {
        let mut c = checked_core();
        let _job = c.launch_job("a").ok().unwrap();
        c.run_mut("a").unwrap().cancel_requested = Some(StopMode::CloseAndStop);
        let handle = runner_core::bot::BotHandle::idle_for_tests("BTCUSDT");
        let attached =
            c.attach_bot("a", RunMode::Live, handle, Some(9), "  웹 저장본  ", "바이낸스", "BTCUSDT").unwrap();
        assert!(attached.reporter.is_none() && attached.env == "live");
        let r = c.run("a").unwrap();
        assert!(r.running() && r.stopping, "시작하는 동안 누른 종료를 봇에 넘김");
        assert!(!r.start_pending && r.preflight.is_none() && r.cancel_requested.is_none());
        let d = r.data.as_ref().unwrap();
        assert_eq!((d.session_id, d.macro_origin.as_str()), (Some(9), "웹 저장본"));
        assert_eq!(r.activity.last().unwrap().title, "실행을 시작했어요 · 실전");
        c.runs.clear();
        let handle = runner_core::bot::BotHandle::idle_for_tests("BTCUSDT");
        assert!(c.attach_bot("a", RunMode::Live, handle, None, "", "바이낸스", "BTCUSDT").is_none());
    }

    #[test]
    fn supported_macros_pass_the_gate_unless_busy_or_conflicting() {
        let rsi = json!({"symbol": "BTCUSDT", "exchange": "binance", "rule_type": "F"});
        let mut c = core_with(&[("a", rsi.clone()), ("b", rsi)]);
        assert!(c.start_gate("a").is_none(), "지원하는 매크로는 키 · 연결 확인으로 넘어간다");

        c.runs[0].checking = true;
        assert!(matches!(c.start_gate("a"), Some(StartReply::Checking)));
        c.runs[0].checking = false;

        c.runs[0].start_pending = true;
        assert!(blocked_text(c.start_gate("b")).contains("바이낸스 BTCUSDT"), "같은 종목은 하나만");
        c.runs[0].start_pending = false;

        c.claim_busy = true;
        assert!(blocked_text(c.start_gate("a")).contains("웹 매크로 연결"));
        c.claim_busy = false;

        c.closed = true;
        assert!(matches!(c.start_gate("a"), Some(StartReply::Checking)), "앱을 닫는 중에는 새로 시작하지 않는다");
    }
}
