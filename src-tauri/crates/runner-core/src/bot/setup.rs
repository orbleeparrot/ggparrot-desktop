//! 준비 — 거래소 연결·심볼 규격·주문 전 리허설.

use super::*;

impl Bot {
    /// 매크로의 거래소로 어댑터를 만듦. 모의 모드면 MockBroker로 감쌈.
    pub fn connect(&mut self) -> bool {
        let Some(exchange) = self.exchange() else {
            self.fail(
                "실행을 시작하지 못했어요",
                format!("[오류] {}", crate::types::unsupported_exchange_message(&self.exchange_name)),
            );
            return false;
        };
        let label = exchange.label();
        let pair = match self.credentials.pair(exchange) {
            Some(p) => p,
            None => {
                if self.mode != RunMode::Mock {
                    self.fail(
                        "실행을 시작하지 못했어요",
                        format!("[오류] {}", crate::types::exchange_key_needed_message(label)),
                    );
                    return false;
                }
                self.log(format!("{label} 거래소 키가 없어 모의로만 실행해요(주문은 보내지 않아요)."));
                KeyPair::default()
            }
        };
        if exchange != Exchange::Binance && (self.side != Side::Long || self.leverage > 1) {
            self.fail(
                "실행을 시작하지 못했어요",
                format!("[오류] {label}에서는 원화 현물(롱 · 1배)만 돼요. 선물·숏 매크로는 바이낸스로 만드세요."),
            );
            return false;
        }
        let built = match self.broker_override.clone() {
            Some(b) => {
                b.set_log(self.logger.as_log_fn());
                Ok(b)
            }
            None => build_broker(exchange, self.mode, &self.macro_, &pair, self.logger.as_log_fn(), self.http.clone()),
        };
        let broker = match built {
            Ok(b) => b,
            Err(e) => {
                let reason = safe_preflight_reason(&e.to_string(), &self.credentials);
                self.fail("실행을 시작하지 못했어요", format!("[오류] {label} 연결을 준비하지 못했어요: {reason}"));
                return false;
            }
        };
        self.broker = Some(if self.mode == RunMode::Mock {
            Arc::new(MockBroker::new(broker, self.logger.as_log_fn())) as Arc<dyn Broker>
        } else {
            broker
        });
        self.log(format!(
            "거래소 {label} · 실행 모드 {}{}",
            self.mode_label(),
            if self.testnet() { " (테스트넷 주소)" } else { "" }
        ));
        true
    }

    /// 시장별 규격·레버리지 세팅과 주문 전 리허설. 성공이면 Ok(true).
    pub async fn prepare(&mut self) -> Result<bool, String> {
        if self.market != Market::Futures && self.side == Side::Short {
            self.log("현물은 숏을 지원하지 않아요. 선물 매크로를 쓰세요.");
            return Ok(false);
        }
        let broker = self.broker()?;
        let ready = match broker.ensure_ready().await {
            Ok(r) => r,
            Err(BrokerError::Access { explanation, name }) => return Ok(self.rehearsal_failed(&explanation, &name)),
            Err(e) => return Err(e.to_string()),
        };
        if !ready {
            let where_ = if self.market == Market::Futures { "선물" } else { "현물" };
            let net = if self.testnet() { "(테스트넷) " } else { "" };
            self.fail(
                "실행을 시작하지 못했어요",
                format!(
                    "[오류] {net}{} {where_}에 {} 종목이 없어요. 웹에서 종목을 확인해 매크로를 다시 받아 주세요.",
                    exchange_label(&self.exchange_name),
                    self.symbol
                ),
            );
            return Ok(false);
        }
        self.step = broker.order_rules().await.map_err(|e| e.to_string())?.step;
        Ok(self.rehearse().await)
    }

    pub(super) async fn rehearse(&mut self) -> bool {
        if let Some(prepared) = self.preflight.take() {
            if prepared.matches(&preflight_fingerprint(&self.macro_, self.mode, &self.credentials)) {
                self.log(format!("주문 전 확인: {}", prepared.reason));
                // 이 실행의 어댑터가 준비하며 읽은 잔고가 있으면 그게 더 새로움(국내는 주문 가능 정보에 잔고가 옴).
                let fresh = self.broker().map(|b| b.balance_note()).unwrap_or_default();
                self.balance_activity(if fresh.is_empty() { &prepared.balance } else { &fresh });
                return true;
            }
        }
        let broker = match self.broker() {
            Ok(b) => b,
            Err(e) => return self.rehearsal_failed(&e, ""),
        };
        let (notional, spend) = rehearsal_amounts(&self.macro_, &self.exchange_name, Some(&broker));
        let (ok, reason) = broker.rehearse(notional, spend).await;
        let reason = safe_preflight_reason(&reason, &self.credentials);
        if ok {
            self.log(format!("주문 전 확인: {reason}"));
            self.balance_activity(&broker.balance_note());
            return true;
        }
        self.rehearsal_failed(&reason, &broker.last_error_code())
    }

    /// 시작 전 확인에서 읽은 잔고를 활동 한 줄로 — 모의는 실제 잔고를 쓰지 않아 남기지 않음.
    pub(super) fn balance_activity(&self, note: &str) {
        if note.is_empty() || self.mode == RunMode::Mock {
            return;
        }
        self.logger.activity("info", "잔고를 확인했어요", note.to_string(), format!("잔고: {note}"), "muted");
    }

    /// 주문 전 확인 실패를 남김. `code`는 거래소 오류 코드(지원용) — 문장에는 넣지 않고 원문·자세히 보기에만 붙임.
    pub(super) fn rehearsal_failed(&self, reason: &str, code: &str) -> bool {
        let raw = with_error_code(&format!("[오류] 주문 전 확인 실패 — {reason}"), code);
        self.log(&raw);
        if self.mode == RunMode::Mock {
            self.logger.activity("blocked", "실전이라면 주문 전 확인에서 막혀요", reason.to_string(), raw, "warn");
        } else {
            self.logger.activity("error", "주문 전 확인에서 막혀 시작하지 않았어요", reason.to_string(), raw, "danger");
        }
        if self.exchange_name == "binance" && (["-2015", "-2014"].contains(&code) || reason.contains('키')) {
            let market = if self.market == Market::Futures { "선물" } else { "현물" };
            // 연결 도우미가 안내하는 사이트와 같은 이름(`connection::binance_env`).
            let where_ = if self.testnet() {
                if self.market == Market::Futures {
                    "바이낸스 선물 데모(demo.binance.com)"
                } else {
                    "바이낸스 현물 테스트넷(testnet.binance.vision)"
                }
            } else {
                "바이낸스 실거래(binance.com)"
            };
            // 연습 사이트(테스트넷·데모)에는 허용 IP가 없음.
            let what = if self.testnet() { "권한" } else { "허용 IP와 권한" };
            self.log(format!("  이 매크로는 {market} 매크로예요. {where_} 키인지, {what}을 확인하세요."));
        }
        if self.mode == RunMode::Mock {
            self.log("모의라서 주문 없이 계속해요. 실전으로 바꾸기 전에 위 이유를 고쳐 주세요.");
            return true;
        }
        self.log("주문을 보내지 않고 멈췄어요. 위 내용을 고친 뒤 다시 시작하세요.");
        false
    }

    // --- 주문 ---------------------------------------------------------------
}
