//! 서버 명령 실행과 ack.

use super::*;

impl Bot {
    /// 서버 명령 하나를 실행하고 ack을 만듦. Err는 '포지션이 불확실해져 루프를 끝내야 함'이라는 뜻.
    pub async fn execute_command(&mut self, cmd: &Value, price: f64) -> Result<Value, String> {
        let cid = cmd.get("id").cloned().unwrap_or(Value::Null);
        let mut ack = obj(json!({"command_id": cid, "ok": true, "executed_qty": 0.0, "fill_price": 0.0, "error": "",
                                 "mode": self.mode.as_str(), "outcome": "execution_not_reported"}));
        if self.done_command_ids.contains(&cid) {
            ack.insert("outcome".into(), json!("duplicate_command_no_reexecution"));
            return Ok(Value::Object(ack));
        }
        let action = value_text(cmd.get("action")).to_lowercase();
        let reason = value_text(cmd.get("reason"));
        let operation_id = format!("command:{}", value_text(Some(&cid)));
        self.logger.set_operation(&operation_id);
        let context = obj(json!({"command_id": cid, "action": action, "command_seq": cmd.get("seq"),
            "signal_price": cmd.get("signal_price"), "notional_frac": cmd.get("notional_frac"),
            "qty_frac": cmd.get("qty_frac"), "reason": reason, "mode": self.mode.as_str()}));
        self.logger.telemetry(
            "info",
            "서버 주문 명령을 앱이 받았어요.",
            "runner.command_received",
            "INFO",
            &operation_id,
            &context,
        );
        let uncertain_before = self.position_uncertain;
        let funds_before = self.funds_short;
        self.order_error_code.clear();
        self.order_no_funds = false;
        // 이 명령에서 생긴 체결만 ack에 실음.
        self.last_fill_qty = 0.0;
        self.last_fill_price = 0.0;
        self.signal_price = num(cmd.get("signal_price"));
        let outcome: Result<(), String> = async {
            if !["buy", "short", "sell", "cover"].contains(&action.as_str()) {
                return Err(format!("알 수 없는 명령 {action}"));
            }
            let allowed: [&str; 2] = if self.side == Side::Long { ["buy", "sell"] } else { ["short", "cover"] };
            if !allowed.contains(&action.as_str()) {
                return Err(format!("매크로 방향과 맞지 않는 명령이에요: {action}"));
            }
            if uncertain_before {
                return Err(
                    "포지션을 확인하지 못해 추가 주문을 멈췄어요. 거래소에서 주문과 포지션을 확인하세요.".into()
                );
            }
            if action == "buy" || action == "short" {
                self.run_entry(cmd, &action, &reason, price, &mut ack).await
            } else {
                self.run_exit(cmd, &reason, price, &mut ack).await
            }
        }
        .await;
        self.signal_price = 0.0;

        let is_exit = action == "sell" || action == "cover";
        if outcome.is_ok() && is_exit && ack["outcome"] == "execution_not_reported" && !uncertain_before {
            // 보유가 없는데 청산 명령 — 주문 없이 ok, executed_qty=0.
            let raw = format!("[신호] {reason} → 보유 없음, 건너뜀");
            self.log(&raw);
            // 이 앱은 시작할 때 보유를 0으로 봄 — 지난 실행에서 남은 포지션이면 여기서 정리되지 않음.
            let (detail, tone) = if self.mode == RunMode::Mock {
                (reason.clone(), "muted")
            } else {
                let hint = self.where_to_check();
                (format!("{reason} · 이 앱이 모르는 보유분(지난 실행에서 남은 포지션 등)이 있다면 {hint}"), "warn")
            };
            self.logger.activity("info", "보유한 게 없어 정리 신호를 건너뛰었어요", detail, raw, tone);
            self.remember_done(cid.clone());
            ack.insert("outcome".into(), json!("no_position_no_order"));
            let mut d = context.clone();
            d.insert("outcome".into(), json!("no_position_no_order"));
            d.insert("executed_qty".into(), json!(0));
            self.logger.telemetry(
                "info",
                "보유 포지션이 없어 청산 주문을 보내지 않았어요.",
                "runner.command_reported",
                "INFO",
                &operation_id,
                &d,
            );
            self.logger.set_operation("");
            return Ok(Value::Object(ack));
        }
        if let Err(exc) = outcome {
            if self.position_uncertain && !uncertain_before {
                let mut d = context.clone();
                d.insert("error".into(), json!(exc));
                d.insert("position_uncertain".into(), json!(true));
                self.logger.telemetry(
                    "error",
                    "주문 결과가 불확실해 다음 명령 실행을 멈췄어요.",
                    "runner.command_failed",
                    "ERROR",
                    &operation_id,
                    &d,
                );
                self.logger.set_operation("");
                self.logger.activity(
                    "error",
                    "주문 결과를 확인하지 못했어요",
                    "추가 주문을 멈췄어요. 거래소에서 주문과 포지션을 확인해 주세요.",
                    exc.clone(),
                    "danger",
                );
                return Err(exc);
            }
            ack.insert("ok".into(), json!(false));
            ack.insert("error".into(), json!(exc.chars().take(200).collect::<String>()));
            ack.insert("outcome".into(), json!("execution_failed"));
            // 일부만 체결되고 끝난 주문(만료·취소)도 체결분은 알림.
            if self.last_fill_qty > 0.0 {
                ack.insert("executed_qty".into(), json!(self.last_fill_qty));
                ack.insert("fill_price".into(), json!(self.last_fill_price));
                ack.insert("outcome".into(), json!("partially_executed"));
            }
            // 오류 코드는 문장(ack·활동 설명)에 넣지 않고 원문 줄·자세히 보기·실행 기록 세부에만 남김.
            let raw = with_error_code(&format!("  ⚠ 명령 실행 실패: {exc}"), &self.order_error_code);
            self.log(&raw);
            if self.last_fill_qty > 0.0 {
                self.logger.activity("error", "주문이 일부만 체결됐어요", exc.clone(), raw, "warn");
            } else if self.order_no_funds {
                // 잔고 부족이 이어지는 동안 OS 알림(위험 줄)은 처음 한 번만 — 다음부터는 주의 줄로만 남김.
                let tone = if funds_before { "warn" } else { "danger" };
                let detail = format!("{exc} 다음 신호에서 다시 주문해요.");
                self.logger.activity("error", "잔고가 모자라 주문하지 못했어요", detail, raw, tone);
            } else {
                self.logger.activity("error", "주문이 처리되지 않았어요", exc.clone(), raw, "danger");
            }
        }
        let ok = ack["ok"].as_bool().unwrap_or(false);
        if ok {
            self.remember_done(cid);
        }
        let mut d = context.clone();
        d.insert("status".into(), json!(if ok { "reported_ok" } else { "reported_failed" }));
        for k in ["executed_qty", "fill_price", "error", "outcome"] {
            d.insert(k.into(), ack[k].clone());
        }
        if !ok && !self.order_error_code.is_empty() {
            d.insert("error_code".into(), json!(self.order_error_code));
        }
        d.insert("verification".into(), json!("runner_reported_not_exchange_verified"));
        self.logger.telemetry(
            if ok { "order" } else { "error" },
            &format!("{} 서버 명령 실행 결과를 앱이 보고해요.", self.mode_label()),
            if ok { "runner.command_reported" } else { "runner.command_failed" },
            if ok { "INFO" } else { "ERROR" },
            &operation_id,
            &d,
        );
        self.logger.set_operation("");
        Ok(Value::Object(ack))
    }

    /// 진입 명령(buy·short) — 투자 금액의 비율만큼 주문하고 체결을 ack에 실음.
    async fn run_entry(
        &mut self,
        cmd: &Value,
        action: &str,
        reason: &str,
        price: f64,
        ack: &mut Map<String, Value>,
    ) -> Result<(), String> {
        // 1을 넘는 비율은 투자 금액 전부로 자름(서버 값이 이상해도 상한·투자 금액 안에서).
        let frac = num(cmd.get("notional_frac")).clamp(0.0, 1.0);
        let mut notional = (self.capital * frac).min(order_cap(self.quote));
        if self.domestic() {
            notional = fee_inclusive(notional, bid_fee_rate(self.broker.as_ref()));
        }
        let (qty, _) = order_qty(price, self.step, notional, self.leverage, self.market, self.quote);
        if qty <= 0.0 {
            return Err("주문 수량이 최소 단위보다 작아요.".into());
        }
        let word = if action == "buy" { OrderSide::Buy } else { OrderSide::Sell };
        let raw = format!("[신호] {reason} → {} {} {} @ {}", word.as_str(), py_num(qty), self.symbol, py_num(price));
        self.log(&raw);
        let title = if action == "buy" { "매수 신호" } else { "공매도 신호" };
        self.logger.activity("signal", title, reason.to_string(), raw, "muted");
        let (prev_qty, prev_entry) = if self.in_position { (self.held_qty, self.entry_price) } else { (0.0, 0.0) };
        let extra = if self.domestic() { notional } else { 0.0 };
        if !self.place(word, qty, false, extra).await? {
            return Err("주문이 체결되지 않았어요.".into());
        }
        let (filled_qty, filled_px) = (self.last_fill_qty, self.last_fill_price);
        if prev_qty > 0.0 && filled_qty > 0.0 {
            let mut added = self.held_qty - prev_qty;
            if added <= 0.0 {
                added = filled_qty;
                self.held_qty = prev_qty + added;
            }
            self.entry_price = (prev_qty * prev_entry + added * filled_px) / self.held_qty;
            self.in_position = true;
        }
        ack.insert("executed_qty".into(), json!(filled_qty));
        ack.insert("fill_price".into(), json!(filled_px));
        ack.insert("outcome".into(), json!("execution_reported"));
        Ok(())
    }

    /// 정리 명령(sell·cover) — 비율만큼(0.999 이상은 전량) 정리하고 체결을 ack에 실음.
    /// 보유가 없으면 주문 없이 Ok — ack의 outcome을 그대로 두어 부르는 쪽이 '보유 없음'으로 처리함.
    async fn run_exit(
        &mut self,
        cmd: &Value,
        reason: &str,
        price: f64,
        ack: &mut Map<String, Value>,
    ) -> Result<(), String> {
        if !self.in_position || self.held_qty <= 0.0 {
            return Ok(()); // 보유 없음 — 부르는 쪽에서 처리
        }
        let frac = match num(cmd.get("qty_frac")) {
            f if f != 0.0 => f,
            _ => 1.0,
        };
        let what = if frac >= 0.999 { "전량".to_string() } else { format!("{:.0}%", frac * 100.0) };
        let raw = format!("[신호] {reason} → {what} 청산 @ {}", py_num(price));
        self.log(&raw);
        let title = if frac >= 0.999 { "정리 신호".to_string() } else { format!("일부 정리 신호 · {what}") };
        self.logger.activity("signal", title, reason.to_string(), raw, "muted");
        if frac >= 0.999 {
            if !self.close_position().await? {
                return Err("청산 주문이 체결되지 않았어요.".into());
            }
        } else {
            let qty = round_step(self.held_qty * frac, self.step);
            if qty <= 0.0 {
                return Err("부분 청산 수량이 최소 단위보다 작아요.".into());
            }
            let word = if self.side == Side::Long { OrderSide::Sell } else { OrderSide::Buy };
            let reduce = self.market == Market::Futures;
            if !self.place(word, qty, reduce, 0.0).await? {
                return Err("부분 청산 주문이 체결되지 않았어요.".into());
            }
        }
        ack.insert("executed_qty".into(), json!(self.last_fill_qty));
        ack.insert("fill_price".into(), json!(self.last_fill_price));
        ack.insert("outcome".into(), json!("execution_reported"));
        Ok(())
    }

    pub(super) fn remember_done(&mut self, cid: Value) {
        if self.done_command_ids.len() >= 200 {
            self.done_command_ids.pop_front();
        }
        self.done_command_ids.push_back(cid);
    }
}
