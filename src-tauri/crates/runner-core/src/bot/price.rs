//! 시세 — 현재가 받기, 못 받은 틱의 처리(알림·상태 보고·청산 명령만 실행), 실행하지 않은 명령의 ack.

use super::*;

impl Bot {
    /// 현재가. 못 받으면 알리고, 하트비트로 상태만 올림(받은 명령은 실행하지 않았다고 ack) — None.
    /// 그동안 손절·안전 규칙도 볼 수 없으므로 화면에 바로 보이고, 포지션이 있으면 1분 뒤 한 번 더 크게 알림.
    pub(super) async fn fetch_price(&mut self, broker: &dyn Broker, guard: &mut RiskGuard) -> Option<f64> {
        match broker.price().await {
            // 0·음수·NaN은 시세가 아님 — 손절·강제 청산을 잘못 당기지 않게 실패로 봄.
            Ok(p) if !(p.is_finite() && p > 0.0) => {
                self.price_failed(BrokerError::Other(format!("거래소가 이상한 현재가({p})를 줬어요")), guard).await;
                None
            }
            Ok(p) => {
                self.last_price = p;
                if self.price_failing {
                    self.price_failing = false;
                    self.price_fail_since = 0;
                    self.price_fail_ticks = 0;
                    self.price_escalated = false;
                    self.logger.activity(
                        "connection",
                        "현재가를 다시 받고 있어요",
                        "손절 · 안전 규칙도 다시 확인해요",
                        "",
                        "ok",
                    );
                }
                Some(p)
            }
            Err(exc) => {
                self.price_failed(exc, guard).await;
                None
            }
        }
    }

    /// 현재가를 못 받은 틱 — 알리고, 상태만 올리고, 받은 명령 중 청산은 현재가 없이 실행함(시장가 청산은 보유
    /// 수량만 있으면 됨·서버가 다시 보내지 않으면 청산 신호가 사라짐). 진입은 실행하지 않았다고 ack.
    async fn price_failed(&mut self, exc: BrokerError, guard: &mut RiskGuard) {
        let raw = format!("  일시 오류(시세): {exc} — {POLL_SECONDS:.0}초 후 재시도");
        if self.price_fail_ticks.is_multiple_of(PRICE_LOG_EVERY_TICKS) {
            self.log(&raw);
        }
        self.price_fail_ticks = self.price_fail_ticks.wrapping_add(1);
        if !self.price_failing {
            self.price_failing = true;
            self.price_fail_since = unix_ms();
            let detail = if self.in_position {
                format!("그동안 손절 · 안전 규칙을 확인하지 못해요. {POLL_SECONDS:.0}초마다 다시 시도해요")
            } else {
                format!("그동안 신호대로 주문하지 않아요. {POLL_SECONDS:.0}초마다 다시 시도해요")
            };
            self.logger.activity("connection", "현재가를 받지 못하고 있어요", detail, raw.clone(), "warn");
        }
        if self.in_position
            && !self.price_escalated
            && unix_ms().saturating_sub(self.price_fail_since) >= PRICE_STALE_ALERT_MS
        {
            self.price_escalated = true;
            self.logger.activity(
                "error",
                "현재가를 1분 넘게 받지 못하고 있어요",
                format!("손절 · 안전 규칙을 확인하지 못하고 있어요. {}", self.where_to_check()),
                raw,
                "danger",
            );
        }
        let mut snap = self.snapshot();
        snap["note"] = json!("시세 연결 재시도 중 · 마지막 확인 가격");
        let reply = self.report_snapshot(snap).await;
        if let Some(m) = StopMode::parse(&reply.action) {
            self.note_remote_stop(&reply, m);
        }
        if StopMode::parse(&reply.action).is_some() {
            self.skip_commands(&reply.commands, "stop_requested", "종료 중이라 실행하지 않았어요");
            return;
        }
        let (exits, entries): (Vec<Value>, Vec<Value>) = reply
            .commands
            .iter()
            .cloned()
            .partition(|c| matches!(value_text(c.get("action")).to_lowercase().as_str(), "sell" | "cover"));
        self.skip_commands(&entries, "price_unavailable", "현재가를 받지 못해 실행하지 않았어요");
        if !entries.is_empty() {
            self.logger.activity(
                "blocked",
                format!("서버 진입 신호 {}개를 실행하지 못했어요", entries.len()),
                "현재가를 받지 못해 주문하지 않았어요",
                "",
                "warn",
            );
        }
        if !exits.is_empty() {
            let only_exits =
                crate::server::HeartbeatReply { action: "continue".into(), commands: exits, ..Default::default() };
            let last = self.last_price;
            // 결과를 모르는 주문이 생기면 다음 틱의 안전망·명령이 멈춘 상태를 따름.
            if let Err(e) = self.apply_reply(&only_exits, guard, last).await {
                self.log(format!("  ⚠ 청산 실행 중단: {e}"));
            }
        }
    }

    /// 실행하지 않은 명령을 그 이유와 함께 ack함(서버가 기다리지 않게·조용히 버리지 않게).
    pub(super) fn skip_commands(&mut self, cmds: &[Value], outcome: &str, why: &str) {
        for cmd in cmds {
            let id = cmd.get("id").cloned().unwrap_or(Value::Null);
            self.pending_acks.push(json!({"command_id": id, "ok": false, "executed_qty": 0.0,
                "fill_price": 0.0, "error": why, "outcome": outcome}));
        }
    }
}
