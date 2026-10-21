//! 주문 — 시장가 주문·체결 반영·정리·로컬 손절 판단.

use super::*;
use crate::brokers::Order;

impl Bot {
    /// 시장가 주문 하나를 넣고 체결을 확인한 뒤 보유 상태를 갱신함. 실패는 Err(사람이 읽을 문장).
    /// - 거래소 오류 코드는 문장에 넣지 않고 `order_error_code`에 따로 남김(자세히 보기·원문 로그용).
    /// - 진입 주문이 잔고 부족으로 거절되면 `funds_short`를 켜고, 진입 주문이 체결되면 끔. 매매 동작은 그대로임.
    pub async fn place(&mut self, side: OrderSide, qty: f64, reduce_only: bool, notional: f64) -> Result<bool, String> {
        self.place_order(side, qty, reduce_only, notional, None).await
    }

    /// [`place`]와 같되 `limit` 이 있으면 지정가 IOC(호가 단위로 맞춘 가격) — 수량으로 내고, 바로 체결된 만큼만(0 일 수도)
    /// 보유에 반영한 뒤 정상으로 끝냄(설계 §6: IOC 무체결은 정상 결과 — 재시도 · 시장가 전환 없음).
    pub async fn place_order(
        &mut self,
        side: OrderSide,
        qty: f64,
        reduce_only: bool,
        notional: f64,
        limit: Option<f64>,
    ) -> Result<bool, String> {
        self.order_error_code.clear();
        self.order_no_funds = false;
        let broker = self.broker()?;
        let open = if self.side == Side::Long { OrderSide::Buy } else { OrderSide::Sell };
        let closing = reduce_only || (self.in_position && side != open);
        let sellable = if self.held_qty > 0.0 { round_step(self.held_qty, self.step) } else { 0.0 };
        let step_or = if self.step != 0.0 { self.step } else { 1e-12 };
        let partial_close = closing && sellable > 0.0 && (sellable - qty) >= step_or;
        let client_id = format!("ggp-{}", &uuid::Uuid::new_v4().simple().to_string()[..28]);
        let by_money = self.domestic() && !closing && notional > 0.0 && limit.is_none();
        let current_op = self.logger.operation();
        let in_command = current_op.starts_with("command:");
        let operation_id = if current_op.is_empty() { format!("order:{client_id}") } else { current_op };
        let request_details = obj(json!({
            "action": side.as_str(), "client_order_id": client_id,
            "requested_notional": if by_money { json!(notional) } else { Value::Null },
            "requested_qty": if by_money { Value::Null } else { json!(qty) },
            "quote_currency": self.quote, "reduce_only": reduce_only, "closing": closing,
            "mode": self.mode.as_str(), "exchange": self.exchange_name, "symbol": self.symbol,
            "order_type": if limit.is_some() { "limit_ioc" } else { "market" }, "limit_price": limit,
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
        let order = match broker
            .submit(SubmitRequest {
                side,
                base_qty: if by_money { None } else { Some(qty) },
                notional: if by_money { Some(notional) } else { None },
                reduce_only,
                closing: Some(closing),
                client_id: client_id.clone(),
                limit,
            })
            .await
        {
            Ok(o) => o,
            Err(e) => {
                self.order_error_code = e.name().to_string();
                let mut d = request_details.clone();
                d.insert("error".into(), json!(e.to_string()));
                d.insert("error_code".into(), json!(self.order_error_code));
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
        let (result_uncertain, reported_fill) = classify(&order);
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
        let close = self.apply_fill(&order, closing, result_uncertain);
        self.last_close = close;
        if executed > 0.0 {
            self.record_order_fact(&order, side, &client_id, in_command);
        }
        if order.status == "REJECTED" && executed == 0.0 {
            let (text, code, no_funds) = self.rejection_text(&order.raw_status, closing);
            self.order_error_code = code;
            self.order_no_funds = no_funds && !closing;
            if self.order_no_funds {
                self.funds_short = true;
            }
            return Err(text);
        }
        if let Some(px) = limit {
            if !self.position_uncertain && crate::brokers::is_terminal(&order.status) {
                self.ioc_activity(
                    side,
                    closing,
                    executed,
                    average,
                    px,
                    close.as_ref(),
                    order.fee.as_ref(),
                    &order.raw_status,
                );
                if !closing && executed > 0.0 {
                    self.funds_short = false; // 진입 주문이 체결됨 — 잔고 부족이 풀림
                }
                return Ok(true);
            }
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
            self.fill_activity(side, closing, executed, average, close.as_ref(), order.fee.as_ref(), raw);
            // 이 문장은 서버 ack의 error로도 감 — 수량 자릿수를 줄이지 않음(qty_full).
            return Err(format!(
                "주문이 일부만 체결되고 끝났어요(거래소 상태: {}). 체결된 {} {}만 보유에 반영했어요.",
                order.raw_status,
                qty_full(executed),
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
        self.fill_activity(side, closing, executed, average, close.as_ref(), order.fee.as_ref(), raw);
        if !closing {
            self.funds_short = false; // 진입 주문이 체결됨 — 잔고 부족이 풀림
        }
        Ok(true)
    }

    /// 체결 기준 장부(연동 레벨 21) — 체결이 있는 주문 한 건의 사실을 서버로 보낼 자리에 남김.
    /// 서버 명령 중이면 그 명령 ack 의 `orders`, 아니면(로컬 손절 · 안전 규칙 · 종료 정리) 하트비트의 `local_fills`.
    /// 키는 이 주문의 client_order_id — 서버는 같은 키를 한 번만 장부에 넣는다. 수수료를 모르면 어림값을 싣고 fee_known=false.
    fn record_order_fact(&mut self, order: &Order, side: OrderSide, client_id: &str, in_command: bool) {
        let notional = order.executed_qty * order.avg_price;
        let (fee_quote, fee_known) = match &order.fee {
            Some(fee) if fee.other.is_empty() => (fee.quote, true),
            other => (other.as_ref().map(|f| f.quote).unwrap_or(0.0).max(notional * self.fee_rate_estimate()), false),
        };
        let mut fact = obj(json!({
            "client_order_id": client_id, "exchange_order_id": order.order_id, "status": order.status,
            "side": side.as_str().to_lowercase(), "executed_qty": order.executed_qty,
            "acquired_qty": order.acquired_qty, "fill_price": order.avg_price,
            "fee_quote": fee_quote, "fee_known": fee_known, "ts_ms": crate::clock::unix_ms(),
        }));
        if in_command {
            self.command_orders.push(Value::Object(fact));
        } else {
            let reason = if self.local_reason.is_empty() { "local" } else { self.local_reason };
            fact.insert("reason".into(), json!(reason));
            self.pending_local_fills.push(Value::Object(fact));
        }
    }

    /// 지정가 IOC 결과 한 줄 — 체결이 있으면 보통 체결 줄, 없으면 '조건에 맞는 상대가 없어 이번엔 안 했어요'(위험 · 오류가 아님).
    #[allow(clippy::too_many_arguments)]
    fn ioc_activity(
        &mut self,
        side: OrderSide,
        closing: bool,
        executed: f64,
        average: f64,
        limit: f64,
        close: Option<&ClosePnl>,
        fee: Option<&crate::brokers::FeePaid>,
        raw_status: &str,
    ) {
        let selling = side == OrderSide::Sell;
        let bound = if selling { "이상" } else { "이하" };
        if executed > 0.0 {
            let raw = format!(
                "  ✓ {} 지정가 IOC 체결: 수량={} @ {} (지정가 {} {bound}) · 거래소 상태 {raw_status}",
                side.as_str(),
                py_num(executed),
                py_num(average),
                py_num(limit)
            );
            self.logger.ui(&raw);
            self.fill_activity(side, closing, executed, average, close, fee, raw);
            return;
        }
        let verb = if selling { "팔지" } else { "사지" };
        let raw = format!(
            "  · {} 지정가 IOC {} {bound} — 체결 없이 끝남(거래소 상태 {raw_status})",
            side.as_str(),
            py_num(limit)
        );
        self.log(&raw);
        self.logger.activity(
            "info",
            format!("지정가에 맞는 상대가 없어 이번엔 {verb} 않았어요"),
            format!("지정가 {} {bound} · 다음 봉에서 다시 판단해요", price_text(limit, self.quote)),
            raw,
            "muted",
        );
    }

    /// 거래소가 받지 않은 주문의 (사람이 읽을 문장, 오류 코드, 잔고 부족인지). 코드는 문장에 넣지 않음.
    /// `raw`는 거절 상태 글 — 국내는 오류 이름("insufficient_funds_bid"), 바이낸스는 "-2010 Account has …".
    pub(super) fn rejection_text(&self, raw: &str, closing: bool) -> (String, String, bool) {
        if self.domestic() {
            let ex = self.exchange().unwrap_or(Exchange::Upbit);
            let text = crate::brokers::explain_domestic_error(ex, raw, "");
            return (text, raw.to_string(), crate::brokers::is_no_funds_error(raw));
        }
        let (code, message) = crate::brokers::binance_rejection_parts(raw);
        let no_funds = crate::brokers::is_binance_no_funds(code, message);
        let text = if no_funds && !closing {
            if self.market == Market::Futures {
                format!(
                    "주문할 증거금이 모자라요. 바이낸스 선물 지갑에 {}를 채우거나 매크로 투자 금액을 줄이세요.",
                    self.quote
                )
            } else {
                format!("주문할 {} 잔고가 모자라요. 잔고를 채우거나 매크로 투자 금액을 줄이세요.", self.quote)
            }
        } else {
            let why = if message.is_empty() { String::new() } else { format!("({message})") };
            format!("바이낸스에서 주문을 거절했어요{why}. 잔고 · 최소 주문 금액 · 키 권한을 확인하세요. 주문은 들어가지 않았어요.")
        };
        (text, code.to_string(), no_funds)
    }

    /// 체결 하나를 보유 상태에 반영함 — 수수료 합계, 정리(실현 손익·먼지)·추가 진입(가중평균)·새 진입.
    /// `uncertain`은 [`classify`]의 결과 — 불확실하면 보유를 평평하게 보지 않음. 정리 체결이면 그 손익을 돌려줌.
    pub(super) fn apply_fill(&mut self, order: &Order, closing: bool, uncertain: bool) -> Option<ClosePnl> {
        let (executed, average, acquired) = (order.executed_qty, order.avg_price, order.acquired_qty);
        self.last_fill_qty = executed;
        self.last_fill_price = average;
        self.position_uncertain = uncertain;
        // 이 체결의 수수료(견적 통화). 모르면(선물·BNB 등) 기본 수수료율로 어림함 — 안전 규칙이 수수료를 빼고 보게.
        let (fee_quote, fee_known) = if executed > 0.0 {
            let notional = executed * average;
            match &order.fee {
                Some(fee) if fee.other.is_empty() => {
                    self.fees_quote += fee.quote;
                    if notional > 0.0 && crate::pnl::plausible_rate(fee.quote / notional) {
                        self.observed_fee_rate = Some(fee.quote / notional);
                    }
                    (fee.quote, true)
                }
                other => {
                    self.fees_complete = false;
                    let part = other.as_ref().map(|f| f.quote).unwrap_or(0.0);
                    (part.max(notional * self.fee_rate_estimate()), false)
                }
            }
        } else {
            (0.0, true)
        };
        let mut dust_only = false;
        let mut close = None;
        if executed != 0.0 {
            if closing {
                let held_before = self.held_qty;
                let closed_qty = held_before.min(executed);
                let gross = if average != 0.0 && self.entry_price != 0.0 {
                    pnl_amount(closed_qty, self.entry_price, average, self.side)
                } else {
                    0.0
                };
                self.realized += gross;
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
                    // 먼지 평단 — 앞서 남은 먼지가 있으면 수량 가중으로 합친다. 다음 진입 체결 때 수량 · 평단에 넣는다.
                    let before = self.position_dust_qty;
                    self.dust_entry_price = if before > 0.0 {
                        (before * self.dust_entry_price + self.held_qty * self.entry_price) / (before + self.held_qty)
                    } else {
                        self.entry_price
                    };
                    self.position_dust_qty += self.held_qty;
                    self.log(format!(
                        "최소 주문 단위보다 적게 남은 수량: {} {}",
                        py_gp(self.position_dust_qty, 12),
                        crate::types::base_asset(&self.symbol)
                    ));
                    dust_only = true;
                }
                let ended = (self.held_qty == 0.0 || dust_only) && !self.position_uncertain;
                if ended {
                    self.entry_price = 0.0;
                }
                // 진입 수수료는 정리한 수량만큼 나눠 실현손익에 넣음. 포지션이 끝나면(먼지 포함) 남은 몫을 모두 넣음.
                let entry_fee = if ended {
                    self.open_entry_fee
                } else {
                    crate::pnl::allocate_entry_fee(self.open_entry_fee, held_before, closed_qty)
                };
                let fees_known = fee_known && self.open_fee_known;
                self.open_entry_fee = (self.open_entry_fee - entry_fee).max(0.0);
                if ended {
                    self.open_entry_fee = 0.0;
                    self.open_fee_known = true;
                }
                let fees = entry_fee + fee_quote;
                self.realized_net += gross - fees;
                close = Some(ClosePnl { gross, net: gross - fees, fees, fees_known });
            } else if self.in_position && self.held_qty > 0.0 && self.entry_price > 0.0 && average != 0.0 {
                // 추가 진입(그리드·마틴게일): 보유 수량을 더하고 진입가는 가중평균. 진입 수수료는 쌓음.
                let (prev_qty, prev_entry) = (self.held_qty, self.entry_price);
                self.held_qty = prev_qty + acquired;
                self.entry_price = (prev_qty * prev_entry + acquired * average) / self.held_qty;
                self.open_entry_fee += fee_quote;
                self.open_fee_known = self.open_fee_known && fee_known;
            } else if self.position_dust_qty > 0.0 && self.dust_entry_price > 0.0 && average != 0.0 {
                // 팔 수 없던 잔량(먼지)을 새 포지션에 합친다(원가 가중 평균) — 서버 장부와 같은 규칙이라 익절선 · 손절선이 맞는다.
                let dust = self.position_dust_qty;
                self.held_qty = dust + acquired;
                self.entry_price = (dust * self.dust_entry_price + acquired * average) / self.held_qty;
                self.position_dust_qty = 0.0;
                self.dust_entry_price = 0.0;
                self.open_entry_fee = fee_quote;
                self.open_fee_known = fee_known;
            } else {
                self.entry_price = average;
                self.held_qty = acquired;
                self.open_entry_fee = fee_quote;
                self.open_fee_known = fee_known;
            }
        }
        self.in_position = (self.held_qty > 0.0 && !dust_only) || self.position_uncertain;
        close
    }

    /// 정리 손익 두 마디 — ("실현손익 +3,998 KRW", "이번 실행 누적 +3,998 KRW"). 수수료를 모르면 수수료 전 값에
    /// "(수수료 제외 전)"을 붙임. 셋째 값은 보인 금액(색을 정할 때 씀).
    pub(super) fn realized_texts(&self, close: &ClosePnl) -> (String, String, f64) {
        let before = "(수수료 제외 전)";
        let (amount, pnl) = if close.fees_known {
            (close.net, format!("실현손익 {}", money(close.net, self.quote)))
        } else {
            (close.gross, format!("실현손익 {}{before}", money(close.gross, self.quote)))
        };
        let total = if self.fees_complete {
            format!("이번 실행 누적 {}", money(self.realized_net, self.quote))
        } else {
            format!("이번 실행 누적 {}{before}", money(self.realized, self.quote))
        };
        (pnl, total, amount)
    }

    /// 체결을 사람이 읽는 문장으로 — "BTC 0.00122개를 81,806 USDT에 샀어요"(자산 이름 뒤에 조사를 붙이지 않음).
    /// 자세히에는 신호가와의 차이·수수료를 덧붙임(서버 명령으로 낸 주문·수수료를 아는 체결만).
    /// 정리 체결은 "실현손익 +3,998 KRW · 이번 실행 누적 +3,998 KRW · 수수료 1,003 KRW"(수수료 반영) — 수수료는
    /// 이번 정리 수수료와 정리한 수량 몫의 진입 수수료를 합한 값.
    #[allow(clippy::too_many_arguments)] // 체결 한 건의 값들 — 묶으면 부르는 곳마다 구조체만 늘어남.
    pub(super) fn fill_activity(
        &self,
        side: OrderSide,
        closing: bool,
        qty: f64,
        price: f64,
        close: Option<&ClosePnl>,
        fee: Option<&FeePaid>,
        raw: String,
    ) {
        let asset = base_asset(&self.symbol);
        let what = format!("{asset} {}개를 {}에", qty_text(qty), price_text(price, self.quote));
        let mock = self.mode == RunMode::Mock;
        let slippage = slippage_text(self.signal_price, price);
        if !closing {
            let verb = if side == OrderSide::Buy { "샀어요" } else { "공매도했어요" };
            let mut parts: Vec<String> = [slippage, fee_text(fee, self.quote)].into_iter().flatten().collect();
            if mock {
                parts.push("모의 체결 · 실제 주문은 보내지 않았어요".into());
            }
            self.logger.activity("buy", format!("{what} {verb}"), parts.join(" · "), raw, "ok");
        } else {
            let verb = if side == OrderSide::Sell { "팔았어요" } else { "되사서 정리했어요" };
            let close = close.copied().unwrap_or_default();
            let (pnl, total, amount) = self.realized_texts(&close);
            let fee_part = if close.fees_known {
                (close.fees > 0.0).then(|| format!("수수료 {}", price_text(close.fees, self.quote)))
            } else {
                fee_text(fee, self.quote)
            };
            let mut parts: Vec<String> = vec![pnl, total];
            parts.extend([slippage, fee_part].into_iter().flatten());
            if mock {
                parts.push("모의 체결".into());
            }
            self.logger.activity(
                "sell",
                format!("{what} {verb}"),
                parts.join(" · "),
                raw,
                pnl_tone(amount, self.quote),
            );
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

/// 주문 결과 분류 — (결과가 불확실한지, 체결로 보고할지).
/// 끝나지 않은 상태·FILLED인데 수량이나 평균가가 0·평균가 없는 체결·수수료를 모르는 결과는 불확실로 봄.
fn classify(order: &Order) -> (bool, bool) {
    let (executed, average) = (order.executed_qty, order.avg_price);
    let result_uncertain = !crate::brokers::is_terminal(&order.status)
        || (order.status == "FILLED" && !(executed != 0.0 && average != 0.0))
        || (executed != 0.0 && average == 0.0)
        || !order.fees_known;
    let reported_fill =
        order.status == "FILLED" && !result_uncertain && [executed, average].iter().all(|v| v.is_finite() && *v > 0.0);
    (result_uncertain, reported_fill)
}

/// 신호가 대비 체결가 차이 — "신호가 대비 +0.12%"(체결가가 높으면 +). 신호가가 없으면 None.
pub(super) fn slippage_text(signal: f64, fill: f64) -> Option<String> {
    if !(signal.is_finite() && signal > 0.0 && fill.is_finite() && fill > 0.0) {
        return None;
    }
    Some(format!("신호가 대비 {}", crate::money::signed_pct((fill / signal - 1.0) * 100.0)))
}

/// 손익 색 — 보이는 자릿수로 반올림해 0이면 muted(“0 KRW”가 빨갛게 보이지 않게).
pub(super) fn pnl_tone(amount: f64, quote: &str) -> &'static str {
    if crate::money::rounds_to_zero(amount, crate::money::money_dp(quote)) {
        "muted"
    } else if amount > 0.0 {
        "ok"
    } else {
        "danger"
    }
}

/// 낸 수수료 — "수수료 0.04 USDT"·"수수료 12 KRW + 0.00002 BNB". 모르거나 0이면 None.
pub(super) fn fee_text(fee: Option<&FeePaid>, quote: &str) -> Option<String> {
    let fee = fee?;
    let mut parts = vec![];
    if fee.quote > 0.0 {
        parts.push(price_text(fee.quote, quote));
    }
    for (asset, amount) in &fee.other {
        parts.push(format!("{} {asset}", qty_text(*amount)));
    }
    (!parts.is_empty()).then(|| format!("수수료 {}", parts.join(" + ")))
}
