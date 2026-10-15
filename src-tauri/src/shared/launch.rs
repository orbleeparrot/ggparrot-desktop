//! 시작 — 실전 확인·서버 세션 만들기·봇 띄우기.

use super::*;

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
            return blocked("회원 키가 필요해요.\n\n설정 › 껄무새 계정에서 입력하세요.");
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
                return blocked("웹 매크로 연결이 끝날 때까지 잠시만 기다려 주세요.");
            }
            if run.running() || run.start_pending || run.checking {
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
                let r = c.run_mut(id).unwrap();
                r.generation += 1;
                r.preflight = None;
                c.push_activity(
                    id,
                    "info",
                    "확인 중 설정이 바뀌었거나 확인 유효 시간이 지났어요",
                    "[시작]을 다시 누르면 새로 확인해요.",
                    "확인 중 설정이 바뀌었거나 확인 유효 시간이 지났어요. [시작]을 다시 누르면 새로 확인해요.",
                    false,
                    "warn",
                );
                drop(c);
                self.emit();
                return StartReply::Blocked {
                    message: "확인 중 설정이 바뀌었거나 확인 유효 시간이 지났어요. [시작]을 다시 누르면 새로 확인해요."
                        .into(),
                };
            }
        }
        self.launch(id)
    }

    /// 서버에 세션을 만들고 봇을 띄움.
    pub(crate) fn launch(self: &S, id: &str) -> StartReply {
        let job = {
            let mut c = self.lock();
            let Some(run) = c.run(id) else {
                return StartReply::Blocked { message: "매크로를 찾지 못했어요.".into() };
            };
            // 같은 매크로를 두 번 띄우지 않음(검사 끝 자동 시작과 [시작]이 겹칠 때).
            if c.closed || run.start_pending || run.running() {
                return StartReply::Checking;
            }
            let Some(pre) = run.preflight.clone() else {
                return StartReply::Blocked {
                    message: "시작 전 확인 결과가 없어요. [시작]을 다시 눌러 주세요.".into(),
                };
            };
            let creds = c.run_credentials(run);
            let fp = c.run_fingerprint(run);
            let mut payload = session_payload(&run.macro_, run.mode != RunMode::Live, run.mode);
            if let Some(uid) = run.user_macro_id {
                payload["user_macro_id"] = json!(uid);
            }
            if let Some(sig) = &run.sig {
                payload["macro_sig"] = sig.clone();
            }
            payload["macro_source"] = json!(run.source);
            let job = (run.macro_.clone(), run.mode, creds, fp, payload, run.server_base.clone(), run.generation, pre);
            let r = c.run_mut(id).unwrap();
            r.start_pending = true;
            r.cancel_requested = None;
            job
        };
        self.emit();
        let me = self.clone();
        let id = id.to_string();
        tauri::async_runtime::spawn(async move {
            let (m, mode, creds, fp, payload, base, generation, pre) = job;
            me.launch_async(id, m, mode, creds, fp, payload, base, generation, pre).await;
        });
        StartReply::Started
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) async fn launch_async(
        self: S,
        id: String,
        m: Value,
        mode: RunMode,
        creds: Credentials,
        fp: String,
        payload: Value,
        base: String,
        generation: u64,
        preflight: PreflightResult,
    ) {
        let label = exchange_label(&runner_core::types::exchange_name_of(&m));
        let server = ServerClient::new(&creds.member_key, &base, self.outbox.clone(), self.http.clone());
        let started = match server.start(&payload).await {
            Ok(v) => v,
            Err(err) => {
                let (head, msg) = start_error_text(&err, &creds);
                {
                    let mut c = self.lock();
                    if let Some(r) = c.run_mut(&id) {
                        r.start_pending = false;
                        r.cancel_requested = None;
                    }
                    c.push_activity(&id, "error", head, &msg, "", false, "danger");
                }
                self.emit();
                self.notice("error", format!("{head}:\n{msg}"));
                return;
            }
        };
        let sid = server.session_id();
        {
            let mut c = self.lock();
            let symbol = payload["symbol"].as_str().unwrap_or("").to_string();
            let raw = format!(
                "세션 시작 (id={}) · {label} · {symbol} · {} · {}",
                sid.map(|s| s.to_string()).unwrap_or_else(|| "None".into()),
                payload["market"].as_str().unwrap_or(""),
                mode.label()
            );
            c.push_raw(&id, &raw);
            let origin = started.get("macro_origin_label").and_then(Value::as_str).unwrap_or("");
            if !origin.is_empty() {
                let digest = started.get("macro_digest").and_then(Value::as_str).filter(|d| !d.is_empty());
                c.push_raw(
                    &id,
                    &format!("매크로 출처: {origin}{}", digest.map(|d| format!(" · 지문 {d}")).unwrap_or_default()),
                );
            }
            if server.macro_origin() == "file_modified" {
                if let Some(r) = c.run_mut(&id) {
                    r.modified = true;
                }
                c.push_activity(
                    &id,
                    "info",
                    "웹에서 받은 원본과 다른 파일이에요",
                    "이 기기에서 고친 설정 그대로 실행해요. 껄무새 웹의 내 에이전트에는 '수정된 파일'로 표시돼요.",
                    "⚠ 이 매크로 파일은 웹에서 받은 원본과 달라요. 이 기기에서 고친 설정으로 실행돼요.",
                    false,
                    "warn",
                );
            }
        }
        // 시작 요청 동안 설정이 바뀌었으면 방금 만든 세션만 취소함(봇은 만들지 않음).
        let user_cancelled = self.lock().run(&id).is_some_and(|r| r.cancel_requested.is_some());
        let cancelled = {
            let c = self.lock();
            match c.run(&id) {
                None => true,
                Some(r) => {
                    r.cancel_requested.is_some()
                        || c.closed
                        || c.claim_busy
                        || r.generation != generation
                        || c.run_fingerprint(r) != fp
                        || c.member_key.trim() != creds.member_key
                        || c.conflicting_run(r).is_some()
                }
            }
        };
        if cancelled {
            let notified = if user_cancelled {
                server.stopped("stopped", "시작하는 중에 사용자가 종료해 주문 없이 끝냈어요.", None).await
            } else {
                server
                    .stopped("error", "시작하는 중에 설정이 바뀌거나 확인 유효 시간이 지나 주문 없이 취소했어요.", None)
                    .await
            };
            {
                let mut c = self.lock();
                if let Some(r) = c.run_mut(&id) {
                    r.start_pending = false;
                    r.cancel_requested = None;
                    r.generation += 1;
                    r.preflight = None;
                }
                if user_cancelled {
                    c.push_activity(
                        &id,
                        "stop",
                        "시작을 취소했어요",
                        "주문은 하나도 내지 않았어요.",
                        "시작하는 중에 종료해 주문 없이 끝냈어요.",
                        false,
                        "muted",
                    );
                } else {
                    c.push_activity(
                        &id,
                        "error",
                        "시작 중 설정이 바뀌어 매매를 시작하지 않았어요",
                        "다시 시작하면 새로 확인해요.",
                        "시작 설정이 바뀌어 매매를 시작하지 않았어요. [시작]을 다시 누르면 새로 확인해요.",
                        false,
                        "warn",
                    );
                }
                if !notified {
                    c.push_activity(
                        &id,
                        "connection",
                        "서버에 취소를 알리지 못했어요",
                        "껄무새 웹의 내 에이전트에서 상태를 확인하세요.",
                        "",
                        false,
                        "warn",
                    );
                }
            }
            self.emit();
            return;
        }
        let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
        let handle = spawn_bot(m.clone(), creds, mode, server, Some(preflight), self.http.clone(), tx);
        let reporter = {
            let mut c = self.lock();
            let exchange = runner_core::types::exchange_name_of(&m);
            let symbol = runner_core::types::macro_symbol(&m);
            let Some(r) = c.run_mut(&id) else { return };
            let env = r.binance_env();
            let mut data = RunData::started(now_ms(), sid);
            // 서버가 알려 준 매크로 출처 — 상세 머리줄에 보임.
            data.macro_origin = started
                .get("macro_origin_label")
                .and_then(Value::as_str)
                .unwrap_or("")
                .trim()
                .chars()
                .take(60)
                .collect();
            r.data = Some(data);
            // 취소 확인과 봇 띄우기 사이에 누른 종료도 놓치지 않음.
            let late_cancel = r.cancel_requested.take();
            if let Some(mode) = late_cancel {
                handle.command(mode);
            }
            r.bot = Some(handle);
            r.stopping = late_cancel.is_some();
            r.start_pending = false;
            r.preflight = None;
            let reporter = r.reporter.take();
            c.push_activity(
                &id,
                "start",
                &format!("실행을 시작했어요 · {}", mode.label()),
                &format!("{} · {symbol}", exchange_label(&exchange)),
                "",
                false,
                "ok",
            );
            // 실행 중이라는 표시를 저장해 둠 — 앱이 도중에 꺼지면 다음에 켤 때 알 수 있게.
            self.save_runs(&c);
            reporter.map(|rep| (rep, exchange, env))
        };
        if let Some((rep, exchange, env)) = reporter {
            if let Ok(p) = runner_core::setup_progress::progress_payload(
                "started",
                &exchange,
                env,
                "",
                (false, false),
                None,
                "",
                false,
            ) {
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
            return blocked("웹 매크로 연결이 끝날 때까지 잠시만 기다려 주세요.");
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
