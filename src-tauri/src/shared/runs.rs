//! 실행 슬롯 — 추가·빼기·모드·활동 기록·종료 요청.

use super::*;

impl Shared {
    /// 매크로 하나로 새 슬롯을 만듦. 만든 id.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn add_run(
        self: &S,
        macro_: Value,
        title: &str,
        source: &str,
        library_file: Option<String>,
        base: Option<String>,
    ) -> Result<String, String> {
        if !macro_.is_object() || runner_core::types::macro_symbol(&macro_).is_empty() {
            return Err(library::NOT_A_MACRO.into());
        }
        // 서버가 시작을 거절할 매크로(적립식·SAR·여러 종목)는 목록에 넣기 전에 알림.
        if let Some(why) = runner_core::macro_info::unsupported_reason(&macro_) {
            return Err(why.into());
        }
        runner_core::types::macro_leverage(&macro_).map_err(|_| library::NOT_A_MACRO.to_string())?;
        let id = format!("run-{}", &uuid::Uuid::new_v4().simple().to_string()[..12]);
        let before = self.live_probe_fingerprint();
        {
            let mut c = self.lock();
            let mut clean = macro_.clone();
            let sig = clean.as_object_mut().and_then(|o| o.remove("_sig")).filter(Value::is_object);
            let base = base.unwrap_or_else(|| c.server_base.clone());
            let mut run = Run::new(id.clone(), clean, title.to_string(), source, base);
            run.sig = if source == "file" { sig } else { None };
            run.library_file = library_file;
            let unsigned = source == "file" && run.sig.is_none();
            c.runs.push(run);
            c.library_rev += 1;
            if unsigned {
                c.push_activity(
                    &id,
                    "info",
                    "서명이 없는 예전 파일이에요",
                    "실행은 되지만 웹에서 받은 원본인지 확인할 수 없어요. 서버에는 '서명 없는 파일'로 남아요.",
                    "이 매크로 파일엔 서명이 없어요(옛 파일). 서버에는 '서명 없는 파일'로 남아요.",
                    false,
                    "warn",
                );
            }
            self.save_runs(&c);
        }
        self.emit();
        self.recheck_live_slot_if_moved(&before);
        Ok(id)
    }

    pub fn add_run_from_library(self: &S, file_name: &str) -> Result<String, String> {
        let dir = self.library_dir();
        let m = library::read(&dir, file_name)?;
        let title = file_name.trim_end_matches(".json").trim_end_matches(".ggm").to_string();
        self.add_run(m, &title, "file", Some(file_name.to_string()), None)
    }

    /// 목록 순서 바꾸기(끌어서 옮기기·Alt+↑↓) — 실행 중이어도 됨. 저장해서 다시 켜도 유지.
    pub fn move_run(self: &S, id: &str, to: usize) {
        {
            let mut c = self.lock();
            let Some(from) = c.runs.iter().position(|r| r.id == id) else { return };
            let to = to.min(c.runs.len() - 1);
            if from == to {
                return;
            }
            let r = c.runs.remove(from);
            c.runs.insert(to, r);
            self.save_runs(&c);
        }
        self.emit();
    }

    pub fn remove_run(self: &S, id: &str) -> Result<(), String> {
        let before = self.live_probe_fingerprint();
        let closing = {
            let mut c = self.lock();
            let Some(r) = c.run(id) else { return Ok(()) };
            if r.running() || r.start_pending {
                return Err("실행 중인 매크로는 목록에서 뺄 수 없어요. 먼저 종료하세요.".into());
            }
            // 앱을 닫을 때처럼 웹 가이드에 '닫힘'을 알림 — 빼고 나면 가이드가 '진행 중'에 멈춰 있지 않게.
            let closing = c.run_mut(id).and_then(Run::take_closing_report);
            c.runs.retain(|r| r.id != id);
            c.library_rev += 1;
            self.save_runs(&c);
            closing
        };
        if let Some((rep, payload)) = closing {
            tauri::async_runtime::spawn(lifecycle::finish_report(rep, payload));
        }
        self.emit();
        self.recheck_live_slot_if_moved(&before);
        Ok(())
    }

    pub fn set_run_mode(self: &S, id: &str, mode: RunMode) {
        let before = self.live_probe_fingerprint();
        {
            let mut c = self.lock();
            let Some(r) = c.run_mut(id) else { return };
            if r.running() || r.start_pending || r.checking {
                return;
            }
            let allowed = r.exchange().is_none_or(Exchange::has_testnet);
            r.mode = if mode == RunMode::Testnet && !allowed { RunMode::Mock } else { mode };
            r.generation += 1;
            r.preflight = None;
            self.save_runs(&c);
        }
        self.emit();
        self.recheck_live_slot_if_moved(&before);
    }

    pub fn activity_of(&self, id: &str) -> Vec<crate::state::ActivityDto> {
        self.lock().run(id).map(|r| r.activity.clone()).unwrap_or_default()
    }

    pub fn copy_activity(&self, id: &str) -> bool {
        let text = {
            let c = self.lock();
            let Some(r) = c.run(id) else { return false };
            let head = format!(
                "[껄무새 활동 기록] {} {} · {} · v{}",
                runner_core::types::exchange_label(&r.exchange_name()),
                r.symbol(),
                r.mode.label(),
                self.version
            );
            crate::state::activity_copy_text(&head, &r.activity, crate::state::ACTIVITY_COPY_CHARS)
        };
        self.write_clipboard(&text)
    }

    pub fn stop_run(self: &S, id: &str, mode: StopMode) {
        {
            let mut c = self.lock();
            let Some(r) = c.run_mut(id) else { return };
            let Some(bot) = r.bot.as_ref() else {
                // 아직 시작하는 중 — 세션이 만들어지는 대로 주문 없이 끝냄.
                if r.start_pending && r.cancel_requested.is_none() {
                    r.cancel_requested = Some(mode);
                    c.push_activity(
                        id,
                        "stop",
                        "시작을 취소하는 중이에요",
                        "서버 연결이 끝나는 대로 주문 없이 멈춰요",
                        "",
                        false,
                        "muted",
                    );
                    drop(c);
                    self.emit();
                }
                return;
            };
            bot.command(mode);
            r.stopping = true;
            // 화면의 종료 단추와 같은 말 — 모의는 실제 청산이 없고, 보유가 없으면 두 방법이 같음.
            let holding = r.data.as_ref().is_some_and(RunData::holding);
            let (title, detail) = match (r.mode, holding, mode) {
                (RunMode::Mock, true, StopMode::CloseAndStop) => {
                    ("종료 요청을 보냈어요", "모의 보유분은 지금 가격으로 정리한 것으로 계산해요")
                }
                (RunMode::Mock, ..) | (_, false, _) => ("종료 요청을 보냈어요", ""),
                (_, true, StopMode::CloseAndStop) => {
                    ("청산 후 종료 요청을 보냈어요", "보유 포지션을 시장가로 정리한 뒤 끝내요")
                }
                (_, true, StopMode::StopOnly) => {
                    ("매크로만 종료 요청을 보냈어요", "열린 포지션은 그대로 두고 매크로만 멈춰요")
                }
            };
            c.push_activity(id, "stop", title, detail, "", false, "muted");
        }
        self.emit();
    }

    /// 투자 금액(`params.initial_capital`)을 바꿈. `None`이면 원본으로 되돌림.
    /// 파일에서 연 매크로만 — 웹에서 연결한 매크로는 서버가 웹 저장본과 다르면 시작을 거절함(409).
    /// 바꾸면 서명과 달라져 웹의 내 에이전트에는 '수정된 파일'로 남음. 시작 전 확인은 새로 함.
    pub fn set_run_capital(self: &S, id: &str, amount: Option<f64>) -> Result<(), String> {
        {
            let mut c = self.lock();
            let r = c.run_mut(id).ok_or("매크로를 찾지 못했어요.")?;
            if r.running() || r.start_pending || r.checking {
                return Err("실행 중에는 투자 금액을 바꿀 수 없어요. 종료한 뒤 바꾸세요.".into());
            }
            if r.source == "web" || r.user_macro_id.is_some() {
                return Err(
                    "웹에서 연결한 매크로는 여기서 투자 금액을 바꿀 수 없어요. 웹에서 고친 뒤 다시 연결하세요.".into(),
                );
            }
            let current = runner_core::macro_info::initial_capital(&r.macro_);
            let target = match amount {
                Some(a) => a,
                None => match r.original_capital {
                    Some(o) => o,
                    None => return Ok(()),
                },
            };
            if r.original_capital.is_none() {
                r.original_capital = current;
            }
            runner_core::macro_info::set_initial_capital(&mut r.macro_, target)?;
            if r.original_capital.is_some_and(|o| (o - target).abs() < 1e-9) {
                r.original_capital = None; // 원본과 같아짐
            }
            r.generation += 1;
            r.preflight = None;
            let quote = runner_core::types::quote_of(&r.symbol());
            let money = |v: f64| runner_core::money::price_text(v, quote);
            let detail = match current {
                Some(before) => format!("{} → {} · 시작하면 연결을 새로 확인해요.", money(before), money(target)),
                None => format!("{} · 시작하면 연결을 새로 확인해요.", money(target)),
            };
            c.push_activity(id, "info", "투자 금액을 바꿨어요", &detail, "", false, "muted");
            self.save_runs(&c);
        }
        self.emit();
        Ok(())
    }
}
