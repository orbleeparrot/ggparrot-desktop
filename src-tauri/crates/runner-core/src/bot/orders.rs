//! 주문 — 시장가 주문·체결 반영·정리·로컬 손절 판단.

use super::*;

impl Bot {
    /// 시장가 주문 하나를 넣고 체결을 확인한 뒤 보유 상태를 갱신함. 실패는 Err(사람이 읽을 문장).
    pub async fn place(&mut self, side: OrderSide, qty: f64, reduce_only: bool, notional: f64) -> Result<bool, String> {
        let broker = self.broker()?;
        let open = if self.side == Side::Long { OrderSide::Buy } else { OrderSide::Sell };
        let closing = reduce_only || (self.in_position && side != open);
        let sellable = if self.held_qty > 0.0 { round_step(self.held_qty, self.step) } else { 0.0 };
        let step_or = if self.step != 0.0 { self.step } else { 1e-12 };
        let partial_close = closing && sellable > 0.0 && (sellable - qty) >= step_or;
        let client_id = format!("ggp-{}", &uuid::Uuid::new_v4().simple().to_string()[..28]);
        let by_money = self.domestic() && !closing && notional > 0.0;
        let current_op = self.logger.operation();
        let operation_id = if current_op.is_empty() { format!("order:{client_id}") } else { current_op };
        let request_details = obj(json!({
            "action": side.as_str(), "client_order_id": client_id,
            "requested_notional": if by_money { json!(notional) } else { Value::Null },
            "requested_qty": if by_money { Value::Null } else { json!(qty) },
            "quote_currency": self.quote, "reduce_only": reduce_only, "closing": closing,
            "mode": self.mode.as_str(), "exchange": self.exchange_name, "symbol": self.symbol,
        }));
        let mode_label = self.mode_label();
        let target = if self.mode == RunMode::Mock { "시뮬레이터" } else { "거래소 어댑터" };
        self.logger.telemetry(
            "order",
            &format!("{mode_label} 주문 요청을 {target}에 보내요."),
            "runner.order_requested",
            "INFO",
            &operation_id,
            &request_details,
        );
        let realized_before = self.realized;
        let order = match broker
            .submit(SubmitRequest {
                side,
                base_qty: if by_money { None } else { Some(qty) },
                notional: if by_money { Some(notional) } else { None },
                reduce_only,
                closing: Some(closing),
                client_id: client_id.clone(),
            })
            .await
        {
            Ok(o) => o,
            Err(e) => {
                let mut d = request_details.clone();
                d.insert("error".into(), json!(e.to_string()));
                self.logger.telemetry(
                    "error",
                    &format!("{mode_label} {target}에서 주문 처리 오류를 알려 왔어요."),
                    "runner.order_failed",
                    "ERROR",
                    &operation_id,
                    &d,
                );
                return Err(e.to_string());
            }
        };
        let (executed, average, acquired, fees_known) =
            (order.executed_qty, order.avg_price, order.acquired_qty, order.fees_known);
        let result_uncertain = !crate::brokers::is_terminal(&order.status)
            || (order.status == "FILLED" && !(executed != 0.0 && average != 0.0))
            || (executed != 0.0 && average == 0.0)
            || !fees_known;
        let reported_fill = order.status == "FILLED"
            && !result_uncertain
            && [executed, average].iter().all(|v| v.is_finite() && *v > 0.0);
        let raw = if order.raw_status.is_empty() { "미기록".to_string() } else { order.raw_status.clone() };
        let id_text = order.order_id.clone().unwrap_or_else(|| "미기록".into());
        self.logger.telemetry(
            if reported_fill { "fill" } else { "warn" },
            &format!(
                "{mode_label} {target} 주문 결과 · status={raw} · id={id_text} · 수량={} · 평균가={}",
                py_g(executed),
                py_g(average)
            ),
            "runner.order_result",
            if reported_fill { "INFO" } else { "WARN" },
            &operation_id,
            &obj(json!({
                "action": side.as_str(), "status": order.status, "raw_status": order.raw_status,
                "client_order_id": client_id, "exchange_order_id": order.order_id,
                "executed_qty": executed, "fill_price": average, "acquired_qty": acquired,
                "fees_known": fees_known, "reconciliation_scope": self.reconciliation_scope(),
                "position_uncertain": result_uncertain, "mode": self.mode.as_str(),
                "exchange": self.exchange_name, "symbol": self.symbol,
                "verification": "runner_reported_not_exchange_verified", "ack_source": "runner",
            })),
        );
        self.last_fill_qty = executed;
        self.last_fill_price = average;
        self.position_uncertain = result_uncertain;
        let mut dust_only = false;
        if executed != 0.0 {
            if closing {
                let closed_qty = self.held_qty.min(executed);
                if average != 0.0 && self.entry_price != 0.0 {
                    self.realized += pnl_amount(closed_qty, self.entry_price, average, self.side);
                }
                self.held_qty = (self.held_qty - executed).max(0.0);
                if self.held_qty < 1e-12 {
                    self.held_qty = 0.0;
                }
                let floor = if self.step != 0.0 { self.step } else { DUST_FLOOR };
                if self.market == Market::Spot
                    && order.status == "FILLED"
                    && self.held_qty > 0.0
                    && self.held_qty < floor
                    && !self.position_uncertain
                {
                    self.position_dust_qty += self.held_qty;
                    self.log(format!(
                        "최소 주문 단위보다 적게 남은 수량: {} {}",
                        py_gp(self.position_dust_qty, 12),
                        crate::types::base_asset(&self.symbol)
                    ));
                    dust_only = true;
                }
                if (self.held_qty == 0.0 || dust_only) && !self.position_uncertain {
                    self.entry_price = 0.0;
                }
            } else if self.in_position && self.held_qty > 0.0 && self.entry_price > 0.0 && average != 0.0 {
                // 추가 진입(그리드·마틴게일): 보유 수량을 더하고 진입가는 가중평균.
                let (prev_qty, prev_entry) = (self.held_qty, self.entry_price);
                self.held_qty = prev_qty + acquired;
                self.entry_price = (prev_qty * prev_entry + acquired * average) / self.held_qty;
            } else {
                self.entry_price = average;
                self.held_qty = acquired;
            }
        }
        self.in_position = (self.held_qty > 0.0 && !dust_only) || self.position_uncertain;
        if order.status == "REJECTED" && executed == 0.0 {
            if self.domestic() {
                let ex = self.exchange().unwrap_or(Exchange::Upbit);
                return Err(crate::brokers::explain_domestic_error(ex, 400, &order.raw_status, ""));
            }
            return Err(format!(
                "바이낸스에서 주문을 거절했어요({}). 잔고 · 최소 주문 금액 · 키 권한을 확인하세요. 주문은 들어가지 않았어요.",
                order.raw_status
            ));
        }
        let incomplete = order.status != "FILLED" || (closing && self.in_position && !partial_close);
        if incomplete && !self.position_uncertain && executed > 0.0 {
            // 일부만 체결되고 끝난 주문(만료·취소) — 체결분은 보유에 반영함. 체결은 알리고 실패로 돌려줌.
            let raw = format!(
                "  ◐ {} 일부 체결: 수량={} · 거래소 상태 {}",
                side.as_str(),
                py_num(executed),
                order.raw_status
            );
            self.logger.ui(&raw);
            self.fill_activity(side, closing, executed, average, realized_before, raw);
            return Err(format!(
                "주문이 일부만 체결되고 끝났어요(거래소 상태: {}). 체결된 {} {}만 보유에 반영했어요.",
                order.raw_status,
                qty_text(executed),
                base_asset(&self.symbol)
            ));
        }
        if incomplete || self.position_uncertain {
            return Err(format!(
                "주문 결과를 확인하지 못했어요(거래소 상태: {}). 거래소에서 주문과 포지션을 확인하세요.",
                order.raw_status
            ));
        }
        let raw = format!(
            "  ✓ {}{} 체결: id={} 수량={}",
            side.as_str(),
            if reduce_only { " (청산)" } else { "" },
            order.order_id.clone().unwrap_or_else(|| "None".into()),
            py_num(executed)
        );
        self.logger.ui(&raw);
        self.fill_activity(side, closing, executed, average, realized_before, raw);
        Ok(true)
    }

    /// 체결을 사람이 읽는 문장으로 — "BTC 0.00122개를 81,806 USDT에 샀어요"(자산 이름 뒤에 조사를 붙이지 않음).
    pub(super) fn fill_activity(
        &self,
        side: OrderSide,
        closing: bool,
        qty: f64,
        price: f64,
        realized_before: f64,
        raw: String,
    ) {
        let asset = base_asset(&self.symbol);
        let what = format!("{asset} {}개를 {}에", qty_text(qty), price_text(price, self.quote));
        let mock = self.mode == RunMode::Mock;
        let mock_note = if mock { "모의 체결 · 실제 주문은 보내지 않았어요" } else { "" };
        if !closing {
            let verb = if side == OrderSide::Buy { "샀어요" } else { "공매도했어요" };
            self.logger.activity("buy", format!("{what} {verb}"), mock_note, raw, "ok");
        } else {
            let verb = if side == OrderSide::Sell { "팔았어요" } else { "되사서 정리했어요" };
            let pnl = self.realized - realized_before;
            let mut detail = format!("손익 {} · 누적 {}", money(pnl, self.quote), money(self.realized, self.quote));
            if mock {
                detail.push_str(" · 모의 체결");
            }
            let tone = if pnl > 0.0 {
                "ok"
            } else if pnl < 0.0 {
                "danger"
            } else {
                "muted"
            };
            self.logger.activity("sell", format!("{what} {verb}"), detail, raw, tone);
        }
    }

    /// 보유 포지션을 시장가로 정리.
    pub async fn close_position(&mut self) -> Result<bool, String> {
        if self.position_uncertain {
            return Err("포지션을 확인하지 못해 추가 주문을 멈췄어요. 거래소에서 주문과 포지션을 확인하세요.".into());
        }
        if !self.in_position || self.held_qty <= 0.0 {
            return Ok(true);
        }
        let word = if self.side == Side::Long { OrderSide::Sell } else { OrderSide::Buy };
        let reduce = self.market == Market::Futures;
        let qty = round_step(self.held_qty, self.step);
        self.place(word, qty, reduce, 0.0).await
    }

    /// 서버와 끊겨도 손절은 됨 — 로컬에 남긴 유일한 청산 판단.
    pub fn local_stop_loss(&self, price: f64) -> bool {
        let sl = num(self.macro_.get("risk").and_then(|r| r.get("stop_loss_pct")));
        if sl == 0.0 || !self.in_position || self.entry_price <= 0.0 {
            return false;
        }
        let sl = sl / 100.0;
        match self.side {
            Side::Long => price <= self.entry_price * (1.0 - sl),
            Side::Short => price >= self.entry_price * (1.0 + sl),
        }
    }
}
