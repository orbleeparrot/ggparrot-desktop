//! 바이낸스 주문 전 확인 — 키 권한 조회·첫 주문 잔고·리허설 읽기.
use serde_json::Value;

use super::parse::{asset_row, futures_available, parse_filters, spot_free};
use super::{BinanceBroker, API_RESTRICTIONS_PATH, REHEARSE_OK};
use crate::brokers::{py_truthy, Broker, BrokerError};
use crate::money::price_text;

/// 선물 첫 주문에 더 남겨 두는 몫(주문 금액 대비) — 시장가 수수료(0.05%)와 체결가 차이.
const FUTURES_MARGIN_BUFFER: f64 = 0.001;

impl BinanceBroker {
    /// 실거래 키의 IP 제한·주문 권한을 키 권한 조회(GET /sapi/v1/account/apiRestrictions)로 읽음.
    ///
    /// - IP 제한이 없는 키는 바이낸스가 'Enable Reading'만 허락함 — 조회는 통과해도 첫 주문에서 막힘.
    /// - 주문 권한이 켜져 있으면 IP 제한이 없어도 통과(Default Security Controls를 끈 계정은 켤 수 있음).
    /// - 조회 자체가 안 되면(일시 오류·응답 모양 다름) 막지 않고 '주문 권한은 첫 주문 때'로 둠.
    pub(super) async fn check_live_restrictions(&self) -> (bool, String) {
        let perms = match self.get(false, API_RESTRICTIONS_PATH, &[], true).await {
            Ok(Value::Object(perms)) => perms,
            _ => return (true, REHEARSE_OK.into()),
        };
        let flag = |key: &str| perms.get(key).is_some_and(py_truthy);
        let (wanted, label) = if self.futures() {
            ("enableFutures", "Enable Futures")
        } else {
            ("enableSpotAndMarginTrading", "Enable Spot & Margin & Stock Trading")
        };
        let restricted = flag("ipRestrict");
        if !flag(wanted) {
            if !restricted {
                self.set_error_name("binance_ip_unrestricted");
                return (
                    false,
                    format!("이 키에는 허용 IP와 주문 권한({label})이 없어요. 허용 IP를 등록하고 주문 권한을 켜세요."),
                );
            }
            if self.futures() {
                // 선물 계정을 열기 전에 만든 키에는 Enable Futures 항목 자체가 없음.
                self.set_error_name("binance_no_futures");
                return (
                    false,
                    "이 키에 Enable Futures가 꺼져 있어요. 키 카드에 그 항목이 없으면 선물 계정을 열기 전에 만든 키예요. 선물 계정을 연 뒤 키를 새로 만드세요.".into(),
                );
            }
            self.set_error_name("binance_no_trade");
            return (
                false,
                format!("이 키에 주문 권한({label})이 꺼져 있어요. API Management의 Edit restrictions에서 켜세요."),
            );
        }
        if restricted {
            self.set_error_name("binance_verified");
            (true, "바이낸스 키 · 허용 IP · 주문 권한을 확인했어요.".into())
        } else {
            self.set_error_name("binance_verified_open");
            (true, "바이낸스 키 · 주문 권한을 확인했어요(IP 제한 없는 키).".into())
        }
    }

    /// 마지막으로 읽은 잔고 목록과 그 안의 주문에 쓸 돈(현물: 견적 통화 잔고, 선물: 사용 가능 증거금).
    /// 잔고를 읽지 않았으면 None, 견적 통화 줄이 없으면 0.
    pub(super) fn quote_funds(&self) -> Option<(Value, f64)> {
        let rows = self.balances.lock().unwrap().clone()?;
        let row = asset_row(&rows, self.quote_or_usdt());
        let available = if self.futures() { row.and_then(futures_available) } else { row.and_then(spot_free) };
        Some((rows, available.unwrap_or(0.0)))
    }

    /// 첫 주문에 쓸 돈과 필요한 몫. 잔고를 읽지 않았으면 None.
    fn funds_for(&self, notional: f64) -> Option<(f64, f64)> {
        let (_, available) = self.quote_funds()?;
        if self.futures() {
            let need = notional / f64::from(self.leverage.max(1)) + notional * FUTURES_MARGIN_BUFFER;
            Some((available, need))
        } else {
            // 수량을 단위로 내려 주문하므로 실제 쓰는 금액은 이보다 크지 않음. 수수료는 기초자산·BNB에서 빠짐.
            Some((available, notional))
        }
    }

    /// 잔고가 첫 주문에 모자라면 사람이 읽을 이유. 넉넉하거나 잔고를 모르면 None(모르면 막지 않음).
    pub(super) fn funds_shortfall(&self, notional: f64) -> Option<String> {
        let (available, need) = self.funds_for(notional)?;
        if notional <= 0.0 || available + 1e-9 >= need {
            return None;
        }
        let quote = self.quote_or_usdt();
        let need_text = price_text((need * 100.0).ceil() / 100.0, quote);
        let have_text = price_text((available.max(0.0) * 100.0).floor() / 100.0, quote);
        Some(if self.futures() {
            format!(
                "선물 증거금이 모자라요. 첫 주문({}배)에 수수료 여유까지 증거금 {need_text}가 필요한데 \
                 사용 가능한 증거금은 {have_text}예요. 바이낸스 선물 지갑으로 {quote}를 옮기거나 매크로 투자 금액을 줄이세요.",
                self.leverage.max(1)
            )
        } else {
            format!(
                "주문할 {quote} 잔고가 모자라요. 첫 주문에 {need_text}가 필요한데 사용 가능한 {quote}는 {have_text}예요. \
                 바이낸스 현물 지갑에 {quote}를 채우거나 매크로 투자 금액을 줄이세요."
            )
        })
    }

    /// 선물 리허설: 서명이 필요한 잔고 조회를 직접 불러 실패하면 그대로 나가게 함. 최소 주문 금액을 돌려줌.
    async fn rehearse_futures_reads(&self) -> Result<f64, BrokerError> {
        self.futures_balance_rows().await?;
        match self.futures_exchange_symbol().await? {
            Some(info) => Ok(parse_filters(&info).1),
            None => Err(BrokerError::Other(format!(
                "{} 심볼을 바이낸스에서 찾지 못했어요. 심볼 이름을 확인하세요.",
                self.symbol
            ))),
        }
    }

    pub(super) async fn rehearse_reads(&self) -> Result<Result<f64, String>, BrokerError> {
        if self.futures() {
            return Ok(Ok(self.rehearse_futures_reads().await?));
        }
        self.check_account().await?;
        if !self.ensure_ready().await? {
            return Ok(Err(format!("{} 심볼을 바이낸스에서 찾지 못했어요. 심볼 이름을 확인하세요.", self.symbol)));
        }
        Ok(Ok(self.rules.lock().unwrap().min_notional))
    }
}
