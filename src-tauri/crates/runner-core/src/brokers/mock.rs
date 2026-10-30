//! 모의 모드 — 시세·규격은 진짜로 읽고 주문만 삼킴.
//!
//! 바이낸스에는 테스트넷이 있지만 업비트·빗썸에는 없음. 세 거래소가 같은 방식으로 연습되게 하는 것이
//! 이 어댑터의 존재 이유임. 안쪽 어댑터의 주문 입구(submit)는 어떤 경우에도 부르지 않고,
//! 계정 설정 쓰기(레버리지 등)가 들어 있는 안쪽 ensure_ready도 부르지 않음.
use async_trait::async_trait;
use std::collections::HashSet;
use std::sync::{Arc, Mutex};

use super::{Broker, BrokerError, FeePaid, LogFn, Order, OrderRules, OrderSide, SubmitRequest, exc_text, positive_f64};
use crate::types::{Market, Side};

const MOCK_FALLBACK_RULES: OrderRules = OrderRules { step: 0.0, min_notional: 0.0 };

pub struct MockBroker {
    inner: Arc<dyn Broker>,
    log: Mutex<LogFn>,
    noted: Mutex<HashSet<&'static str>>,
}

impl MockBroker {
    pub fn new(inner: Arc<dyn Broker>, log: LogFn) -> Self {
        MockBroker { inner, log: Mutex::new(log), noted: Mutex::new(HashSet::new()) }
    }

    fn log(&self, msg: String) {
        let log = self.log.lock().unwrap().clone();
        log(msg);
    }

    /// 봇이 주문마다 부르는 메서드도 있어, 매번 남기면 기록이 묻힘.
    fn note_once(&self, key: &'static str, message: String) {
        if self.noted.lock().unwrap().insert(key) {
            self.log(message);
        }
    }

    fn fee_rate(&self) -> f64 {
        // 바이낸스 어댑터에는 수수료율이 없음. 읽지 못했거나 0이면 거래소·시장 기본값
        // (바이낸스 현물 0.1%·선물 0.05%·업비트 0.05%·빗썸 0.25%)으로 대신함.
        let rate = self.inner.bid_fee().map(positive_f64).unwrap_or(0.0);
        if rate != 0.0 && crate::pnl::plausible_rate(rate) { rate } else { self.inner.default_fee_rate() }
    }
}

#[async_trait]
impl Broker for MockBroker {
    fn market(&self) -> Market {
        self.inner.market()
    }
    fn side(&self) -> Side {
        self.inner.side()
    }
    fn symbol(&self) -> String {
        self.inner.symbol()
    }
    fn fee_from_base_asset(&self) -> bool {
        self.inner.fee_from_base_asset()
    }
    fn bid_fee(&self) -> Option<f64> {
        self.inner.bid_fee()
    }
    fn ask_fee(&self) -> Option<f64> {
        self.inner.ask_fee()
    }
    fn default_fee_rate(&self) -> f64 {
        self.inner.default_fee_rate()
    }
    fn last_error_name(&self) -> String {
        self.inner.last_error_name()
    }
    fn last_error_code(&self) -> String {
        self.inner.last_error_code()
    }
    fn set_log(&self, log: LogFn) {
        self.inner.set_log(log.clone());
        *self.log.lock().unwrap() = log;
    }

    /// 키가 없거나 허용 IP가 막혀 실패해도 연습은 계속함 — 실전에서 막힐 이유는 기록에 남김.
    async fn check_account(&self) -> Result<(), BrokerError> {
        if let Err(e) = self.inner.check_account().await {
            self.note_once(
                "check_account",
                format!("모의: 계정 확인에 실패했지만 계속해요. 실전에서는 막힐 수 있어요({})", exc_text(&e)),
            );
        }
        Ok(())
    }

    /// 규격만 읽음(안쪽 load_market). 심볼이 없다는 답(거짓)은 그대로, 인증·네트워크 실패는 알리고 넘어감.
    async fn ensure_ready(&self) -> Result<bool, BrokerError> {
        match self.inner.load_market().await {
            Ok(found) => Ok(found),
            Err(e) => {
                self.note_once("ensure_ready", format!("모의: 거래소 규격을 읽지 못했지만 계속해요({})", exc_text(&e)));
                Ok(true)
            }
        }
    }

    /// 읽기뿐이라 ensure_ready와 같게 둠.
    async fn load_market(&self) -> Result<bool, BrokerError> {
        self.ensure_ready().await
    }

    /// 키 없이 돌아야 연습이므로 읽지 못하면 '제한 없음'(수량 단위 0·최소 금액 0)으로 물러남.
    async fn order_rules(&self) -> Result<OrderRules, BrokerError> {
        match self.inner.order_rules().await {
            Ok(rules) => Ok(rules),
            Err(e) => {
                self.note_once(
                    "order_rules",
                    format!("모의: 주문 규격을 읽지 못해 제한 없이 진행해요({})", exc_text(&e)),
                );
                Ok(MOCK_FALLBACK_RULES)
            }
        }
    }

    /// 안쪽 리허설을 해 보되, 통과 못 해도 모의는 멈추지 않음. 이유는 기록해서 실전 전환 때 보이게 함.
    async fn rehearse(&self, notional: f64, spend: Option<f64>) -> (bool, String) {
        let spend = spend.filter(|s| *s != 0.0);
        let (ok, reason) = self.inner.rehearse(notional, spend).await;
        if ok {
            return (true, reason);
        }
        let line = format!("모의 모드: 실전이라면 여기서 막힙니다 — {reason}");
        self.log(super::with_error_code(&line, &self.inner.last_error_code()));
        (true, format!("모의라서 그대로 진행해요. 실전이라면 막힐 이유: {reason}"))
    }

    async fn price(&self) -> Result<f64, BrokerError> {
        self.inner.price().await
    }

    /// 현재가로 즉시 전부 체결된 것으로 돌려줌. 안쪽 submit은 어떤 경우에도 부르지 않음.
    async fn submit(&self, req: SubmitRequest) -> Result<Order, BrokerError> {
        let buying = req.side == OrderSide::Buy;
        let price = positive_f64(self.inner.price().await?);
        if price == 0.0 {
            return Err(BrokerError::Other("모의 체결에 쓸 현재가를 받지 못했어요".into()));
        }
        let notional = req.notional.map(positive_f64).unwrap_or(0.0);
        let executed =
            if buying && notional != 0.0 { notional / price } else { req.base_qty.map(positive_f64).unwrap_or(0.0) };
        if executed == 0.0 {
            return Err(BrokerError::Other("모의 주문에 쓸 금액이나 수량이 없어요".into()));
        }
        // 어댑터마다 수수료를 떼는 곳이 다름 — 연습이 실전과 같은 보유 수량을 보여 주려면 그 규칙을 따름.
        let closing = req.closing.unwrap_or(req.reduce_only || !buying);
        let mut acquired = executed;
        if buying && !closing && self.inner.fee_from_base_asset() {
            acquired = executed * (1.0 - self.fee_rate());
        }
        // 모의 수수료(견적 통화) — 현물·선물 모두 알려 줌(화면의 수수료 반영 손익을 연습에서도 보이게).
        // 실전 선물은 수수료를 읽지 않아 화면이 '수수료 제외 전'으로 보임.
        let fee = Some(FeePaid { quote: executed * price * self.fee_rate(), other: vec![] });
        Ok(Order {
            status: "FILLED".into(),
            executed_qty: executed,
            avg_price: price,
            acquired_qty: acquired,
            fees_known: true,
            order_id: Some(format!("mock-{}", req.client_id)),
            raw_status: "MOCK_FILLED".into(),
            fee,
        })
    }

    fn balance_note(&self) -> String {
        self.inner.balance_note()
    }

    fn day_change(&self) -> Option<f64> {
        self.inner.day_change()
    }
}

#[cfg(test)]
mod tests;
