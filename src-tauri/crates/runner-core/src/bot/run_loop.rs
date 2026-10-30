//! 메인 루프 — 종료 확인·시세·안전망·하트비트·명령·대기. 시세는 price.rs, 진입 전 확인은 entry_guard.rs,
//! 서버 보고는 report.rs, 종료 처리는 finish.rs.

use super::*;

impl Bot {
    /// 메인 루프. 끝나면 서버에 종료를 보고하고 Finished를 보냄.
    pub async fn run(mut self) {
        let (status, mut note) = match self.run_inner().await {
            Ok(done) => done,
            Err(exc) => {
                // 대부분 예상한 멈춤(결과를 모르는 주문·정리 연속 실패·준비 중 네트워크 오류)임 — 이유를 그대로,
                // 키 값은 가려서 남김.
                let note = safe_preflight_reason(&exc, &self.credentials);
                self.log(format!("실행을 멈췄어요: {note}"));
                ("error".to_string(), note)
            }
        };
        let dust = self.dust_note();
        if !dust.is_empty() {
            note = format!("{note} · {dust}");
        }
        // 서버 보고(최대 수십 초)를 기다리기 전에 화면부터 바꿈 — 그동안 '실행 중'으로 보이지 않게.
        let _ = self.tx.send(BotEvent::Status(self.ui_snapshot(self.snapshot())));
        let _ = self.tx.send(BotEvent::Ending { status: status.clone(), note: note.clone() });
        self.report_end(&status, &mut note).await;
        let _ = self.tx.send(BotEvent::Status(self.ui_snapshot(self.snapshot())));
        let _ = self.tx.send(BotEvent::Finished { status, note });
    }

    pub(super) async fn run_inner(&mut self) -> Result<(String, String), String> {
        if !self.connect() || !self.prepare().await? {
            return Ok(("error".into(), "시작 준비에 실패했어요. 활동 기록에서 이유를 확인하세요.".into()));
        }
        let mut guard = self.risk_guard();
        let broker = self.broker()?;
        // 서버가 "네가 판단해라" 고 한 세션이면 로컬 엔진을 세운다. 세우지 못하면
        // **아무도 판단하지 않는다** — 서버도 이 세션의 드라이버를 띄우지 않았다.
        let mut local = if self.decides_locally() {
            match self.start_local_engine().await {
                Ok(engine) => Some(engine),
                Err(why) => {
                    self.logger.log_full(
                        &why,
                        Some("error"),
                        "ERROR",
                        "runner.local_engine_failed",
                        "",
                        &obj(json!({"error": why})),
                    );
                    return Ok(("error".into(), why));
                }
            }
        } else {
            None
        };
        loop {
            let tick = std::time::Instant::now();
            self.check_wall_gap();
            // 1) 종료 명령 확인
            if let Some(cmd) = self.get_command() {
                let note = self.finish_position(cmd).await?;
                let status = if cmd == StopMode::CloseAndStop && self.in_position { "error" } else { "stopped" };
                return Ok((status.into(), note));
            }
            // 2) 시세 — 못 받으면 하트비트만 하고(청산 명령은 실행) 다음 틱으로
            let Some(price) = self.fetch_price(broker.as_ref(), &mut guard).await else {
                self.sleep_until_next(tick).await;
                continue;
            };
            guard.roll_day();
            // 3) 로컬 안전망 — 일일 손실·최대 보유시간 + 손절
            self.enforce_safety_net(&mut guard, price).await?;
            // 4) 로컬 판단 — 마감봉을 받아 스스로 사고판다(서버가 켜 준 세션만).
            //    안전망(3) 뒤에 둔다: 손절이 이미 포지션을 닫았으면 그 상태로 판단한다.
            if let Some(engine) = local.as_mut() {
                self.local_round(engine, price, &mut guard).await?;
            }

            // 5) 하트비트 — 상태·ack을 올리고 명령을 받아 순서대로 실행
            let sent_fills = local.as_ref().map(|e| e.unsent()).unwrap_or(0);
            let reply = match local.as_ref() {
                Some(engine) => {
                    let mut snap = self.snapshot();
                    if let (Some(base), Value::Object(extra)) =
                        (snap.as_object_mut(), self.local_snapshot_extra(engine))
                    {
                        base.extend(extra);
                    }
                    self.report_snapshot(snap).await
                }
                None => self.report_status().await,
            };
            if !reply.offline
                && let Some(engine) = local.as_mut()
            {
                // 서버가 받았다 — 보낸 만큼만 비운다(그 사이 새로 난 체결은 남긴다).
                self.clear_sent_fills(engine, sent_fills);
            }
            // 하트비트를 기다리는 동안(최대 10초) 시세가 움직였을 수 있음 — 명령이 있으면 새 시세로 안전망을 다시 보고
            // 그 값으로 주문 크기를 정함.
            let price = if reply.commands.is_empty() {
                price
            } else {
                match broker.price().await {
                    Ok(p) if p.is_finite() && p > 0.0 => {
                        self.last_price = p;
                        self.enforce_safety_net(&mut guard, p).await?;
                        p
                    }
                    _ => price,
                }
            };
            if local.is_some() {
                // 판단은 이 PC 가 한다 — 서버 명령이 섞여 오면 같은 신호로 두 번 산다.
                // 원격 종료 · 청산은 그대로 듣는다.
                if let Some(m) = StopMode::parse(&reply.action) {
                    self.note_remote_stop(&reply, m);
                }
                if !reply.commands.is_empty() {
                    self.skip_commands(
                        &reply.commands,
                        "local_decisions",
                        "이 실행은 앱이 직접 판단해서 서버 명령을 쓰지 않아요",
                    );
                }
            } else {
                self.apply_reply(&reply, &mut guard, price).await?;
            }
            // 6) 대기(종료 명령 시 즉시 깨어남) — 이번 틱에 걸린 시간을 빼서 5초 주기를 지킴.
            self.sleep_until_next(tick).await;
        }
    }

    /// 매크로의 공통 리스크 설정으로 안전망을 만듦.
    pub(super) fn risk_guard(&self) -> RiskGuard {
        let risk = self.macro_.get("risk").filter(|r| r.is_object()).cloned().unwrap_or(json!({}));
        let capital = num(self.macro_.get("params").and_then(|p| p.get("initial_capital")));
        let invest_ratio = match risk.get("invest_ratio") {
            None | Some(Value::Null) => 1.0,
            v => num(v),
        };
        let cap = order_cap(self.quote);
        let guard = RiskGuard::new(&risk, if capital != 0.0 { capital * invest_ratio } else { 0.0 }, cap);
        self.log(format!(
            "공통 리스크: {} · 주문 상한 {} {} · {:.0}초마다 평가",
            guard.describe(),
            grouped(cap, 0),
            self.quote,
            POLL_SECONDS
        ));
        guard
    }

    /// 손절·일일 손실·최대 보유시간에 닿으면 정리함. 결과를 알 수 없으면 Err(실행 끝). 분명히 실패하면
    /// 실행을 끝내지 않고 다시 시도함 — 한도만큼 연달아 실패하면 크게 한 번 알리고 간격을 늘려 가며(최대 1분).
    /// 실행을 끝내면 손절 감시도 함께 사라지므로 끝내지 않음.
    pub(super) async fn enforce_safety_net(&mut self, guard: &mut RiskGuard, price: f64) -> Result<(), String> {
        if self.in_position {
            // 안전 규칙은 수수료 반영 평가손익으로 봄(지금 정리하면 남는 돈 — 수수료를 모르면 기본 수수료율로 어림).
            let unreal = self.unrealized_net(price);
            let (forced, why) = guard.force_close(unreal);
            let stop = self.local_stop_loss(price);
            if (forced || stop) && self.close_failures >= MAX_CLOSE_FAILURES && unix_ms() < self.close_retry_at {
                return Ok(()); // 연달아 실패한 뒤 쉬는 중 — 다음 시도 시각까지 기다림
            }
            if forced || stop {
                let head = if forced { format!("강제청산: {why}") } else { "손절".to_string() };
                // 손절은 가격 기준 — 평단 대비 가격이 손절 %만큼 움직였는지(수수료와 무관).
                let moved = signed_pct(pnl_pct(self.entry_price, price, self.side));
                let raw =
                    format!("[{head}] {} (평단 {} · 평단 대비 가격 {moved})", py_num(price), py_num(self.entry_price));
                self.log(&raw);
                let title = if forced {
                    "안전 규칙으로 포지션을 정리해요"
                } else {
                    "손절 기준에 닿아 정리해요"
                };
                let detail = if forced {
                    why.clone()
                } else {
                    let sl = num(self.macro_.get("risk").and_then(|r| r.get("stop_loss_pct")));
                    format!(
                        "평단 대비 가격 {moved}(손절 기준 -{}%) · 현재가 {} · 평단 {}",
                        crate::macro_info::pct_text(sl),
                        price_text(price, self.quote),
                        price_text(self.entry_price, self.quote)
                    )
                };
                self.logger.activity("stop_loss", title, detail, raw, "warn");
                let (entry_price, net_before) = (self.entry_price, self.realized_net);
                match self.close_position().await {
                    Ok(true) => {
                        self.close_failures = 0;
                        self.close_retry_at = 0;
                        let close = self.last_close.unwrap_or_default();
                        let (pnl, total, _) = self.realized_texts(&close);
                        self.log(format!(
                            "  {pnl} · 평단 대비 가격 {} · {total}",
                            signed_pct(pnl_pct(entry_price, self.last_fill_price, self.side)),
                        ));
                        guard.on_exit(self.realized_net - net_before, stop);
                    }
                    Ok(false) => {}
                    // 결과를 알 수 없으면 더 주문하지 않고 멈춤.
                    Err(e) if self.position_uncertain => return Err(e),
                    // 분명히 실패(거절 등)했고 포지션은 그대로 — 실행을 끝내지 않고 다음 틱에 다시 정리함.
                    Err(e) => {
                        self.close_failures += 1;
                        // 일부라도 정리됐으면 그 손익(수수료 반영)은 하루 손실에 넣음.
                        if self.realized_net != net_before {
                            guard.on_partial_exit(self.realized_net - net_before);
                        }
                        if self.close_failures < MAX_CLOSE_FAILURES {
                            self.logger.activity(
                                "error",
                                "정리 주문이 실패했어요",
                                format!("다음 확인 때 다시 시도해요 ({}/{MAX_CLOSE_FAILURES})", self.close_failures),
                                with_error_code(&format!("  ⚠ 정리 실패: {e}"), &self.order_error_code),
                                "danger",
                            );
                        } else {
                            // 거래소를 같은 실패로 5초마다 두드리지 않게 간격을 늘림: 10·20·40·60초.
                            let step = (self.close_failures - MAX_CLOSE_FAILURES + 1).min(6);
                            let wait = (5_000u64 << step).min(CLOSE_RETRY_MAX_MS);
                            self.close_retry_at = unix_ms() + wait;
                            if self.close_failures == MAX_CLOSE_FAILURES {
                                self.logger.activity(
                                    "error",
                                    format!("정리 주문이 {MAX_CLOSE_FAILURES}번 연달아 실패했어요"),
                                    format!(
                                        "새 진입은 멈추고 정리를 계속 다시 시도해요(최대 1분 간격). {}",
                                        self.where_to_settle()
                                    ),
                                    with_error_code(&format!("  ⚠ 정리 실패: {e}"), &self.order_error_code),
                                    "danger",
                                );
                            } else {
                                self.log(with_error_code(
                                    &format!("  ⚠ 정리 실패({}회): {e}", self.close_failures),
                                    &self.order_error_code,
                                ));
                            }
                        }
                    }
                }
            }
        }
        Ok(())
    }

    /// 서버 답 처리 — 원격 종료면 종료 명령을 걸고, 아니면 명령을 순서대로 실행해 ack을 쌓음.
    pub(super) async fn apply_reply(
        &mut self,
        reply: &crate::server::HeartbeatReply,
        guard: &mut RiskGuard,
        price: f64,
    ) -> Result<(), String> {
        if let Some(m) = StopMode::parse(&reply.action) {
            self.note_remote_stop(reply, m);
            // 함께 온 명령도 실행하지 않았다고 알림(서버가 처리 안 된 채로 기다리지 않게).
            self.skip_commands(&reply.commands, "stop_requested", "종료 요청을 받아 실행하지 않았어요");
        } else {
            for cmd in &reply.commands {
                let kind = value_text(cmd.get("action")).to_lowercase();
                let is_entry = kind == "buy" || kind == "short";
                // 진입 전 확인 — 정리 실패 중·종료 요청 뒤·이상한 명령(비율·만료·가격 차이·총 투자 한도)·
                // 안전 규칙(하루 손실·쿨다운). 청산은 막지 않음.
                let (blocked, why) = if self.close_failures > 0 {
                    (true, "정리하지 못한 포지션이 있어 새 진입을 미뤄요".to_string())
                } else if self.get_command().is_some() {
                    (true, "종료를 요청해서 새로 진입하지 않아요".to_string())
                } else if let Some(why) = self.entry_refusal(cmd, price) {
                    (true, why)
                } else {
                    guard.entry_blocked()
                };
                if blocked && is_entry {
                    let id = cmd.get("id").cloned().unwrap_or(Value::Null);
                    self.logger.activity(
                        "blocked",
                        "진입을 미뤘어요",
                        why.clone(),
                        format!("  ⏸ 진입 보류: {why}"),
                        "warn",
                    );
                    self.logger.log_full(
                        &format!("  ⏸ 진입 보류: {why}"),
                        Some("signal"),
                        "WARN",
                        "runner.entry_blocked",
                        &format!("command:{}", value_text(Some(&id))),
                        &obj(json!({"command_id": id, "action": kind, "reason": why})),
                    );
                    self.pending_acks.push(json!({"command_id": id, "ok": false, "executed_qty": 0.0,
                    "fill_price": 0.0, "error": format!("로컬 리스크 보류: {why}")}));
                    continue;
                }
                let (net_before, was_flat) = (self.realized_net, !self.in_position);
                let ack = match self.execute_command(cmd, price).await {
                    Ok(ack) => ack,
                    Err(e) => {
                        // 실행을 끝내기 전에 이 명령의 결과도 남김(종료 전 마지막 하트비트로 올라감).
                        let id = cmd.get("id").cloned().unwrap_or(Value::Null);
                        self.pending_acks.push(json!({"command_id": id, "ok": false,
                        "executed_qty": self.last_fill_qty, "fill_price": self.last_fill_price,
                        "error": e.chars().take(200).collect::<String>(), "outcome": "position_uncertain"}));
                        return Err(e);
                    }
                };
                // 안전 규칙 집계는 ack의 성공 여부가 아니라 실제 체결로 함 — 일부만 체결되고 끝난 주문도
                // 보유 시간·하루 손실에 들어가야 함. 하루 손실은 수수료 반영 실현손익으로 셈.
                if num(ack.get("executed_qty")) > 0.0 {
                    if is_entry {
                        if was_flat && self.in_position {
                            guard.on_entry();
                        }
                    } else if !self.in_position {
                        guard.on_exit(self.realized_net - net_before, false);
                    } else {
                        guard.on_partial_exit(self.realized_net - net_before);
                    }
                }
                self.pending_acks.push(ack);
            }
        }
        Ok(())
    }
}
