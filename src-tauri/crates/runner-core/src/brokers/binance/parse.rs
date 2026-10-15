//! 바이낸스 응답 읽기 — 심볼 필터·주문 상태·잔고 줄·체결 수수료와 주문 결과 정리.
use serde_json::{Map, Value};

use super::BinanceBroker;
use crate::brokers::{get_pos, is_close, positive_number, py_str, py_truthy, FeePaid, Order, TERMINAL_STATUSES};
use crate::types::{split_symbol, Market};

/// 바이낸스가 주문 조회에 돌려주는 '더 조회해도 같은' 상태. TERMINAL_STATUSES보다 넓음.
const BINANCE_SETTLED: [&str; 5] = ["FILLED", "CANCELED", "REJECTED", "EXPIRED", "EXPIRED_IN_MATCH"];

/// 파이썬 `float(value)` — 문자열·숫자(참·거짓은 1·0)만. 읽지 못하면 None.
pub(super) fn num_of(value: &Value) -> Option<f64> {
    match value {
        Value::Number(n) => n.as_f64(),
        Value::String(s) => s.trim().parse::<f64>().ok(),
        Value::Bool(b) => Some(if *b { 1.0 } else { 0.0 }),
        _ => None,
    }
}

/// 심볼 필터에서 (수량 단위, 최소 주문 금액)을 읽음.
pub fn parse_filters(info: &Value) -> (f64, f64) {
    let (mut step, mut min_notional) = (0.0, 0.0);
    let Some(filters) = info.get("filters").and_then(Value::as_array) else {
        return (step, min_notional);
    };
    for f in filters {
        let kind = f.get("filterType").and_then(Value::as_str).unwrap_or("");
        if (kind == "LOT_SIZE" || kind == "MARKET_LOT_SIZE") && step == 0.0 {
            step = f.get("stepSize").and_then(num_of).unwrap_or(0.0);
        } else if kind == "MIN_NOTIONAL" || kind == "NOTIONAL" {
            // f.get("minNotional", f.get("notional", 0)) or 0
            let raw = match f.get("minNotional") {
                Some(v) => v.clone(),
                None => f.get("notional").cloned().unwrap_or(Value::Null),
            };
            min_notional = if py_truthy(&raw) { num_of(&raw).unwrap_or(0.0) } else { 0.0 };
        }
    }
    (step, min_notional)
}

/// 거래소 상태를 봇이 아는 네 가지로 접음.
pub fn fold_status(raw: Option<&Value>) -> &'static str {
    let status = match raw {
        Some(v) if py_truthy(v) => py_str(v),
        _ => return "UNKNOWN",
    };
    if status == "EXPIRED" || status == "EXPIRED_IN_MATCH" {
        return "CANCELED";
    }
    TERMINAL_STATUSES.iter().find(|s| **s == status).copied().unwrap_or("OPEN")
}

pub(super) fn settled(order: &Value) -> bool {
    order.get("status").and_then(Value::as_str).map(|s| BINANCE_SETTLED.contains(&s)).unwrap_or(false)
}

/// 자산 줄 하나(현물 balances·선물 잔고 목록 모두 `asset` 칸으로 찾음).
pub(super) fn asset_row<'a>(rows: &'a Value, asset: &str) -> Option<&'a Map<String, Value>> {
    rows.as_array()?.iter().filter_map(Value::as_object).find(|r| r.get("asset").and_then(Value::as_str) == Some(asset))
}

/// 선물 잔고 줄의 사용 가능 증거금. availableBalance가 없는 응답이면 balance로 물러남.
pub(super) fn futures_available(row: &Map<String, Value>) -> Option<f64> {
    row.get("availableBalance").or_else(|| row.get("balance")).and_then(num_of)
}

/// 현물 잔고 줄의 사용 가능 수량(free).
pub(super) fn spot_free(row: &Map<String, Value>) -> Option<f64> {
    row.get("free").and_then(num_of)
}

/// 현물 체결 목록의 수수료. 체결이 주문 수량을 다 덮지 못하거나 수수료 칸이 빠졌으면 None.
/// 견적 통화로 낸 것은 그대로, 기초자산으로 낸 것은 그 체결가로 옮기고, 그 밖(BNB 등)은 따로 둠.
pub fn spot_fills_fee(fills: &[Value], executed: f64, base: &str, quote: &str) -> Option<FeePaid> {
    if fills.is_empty() {
        return None;
    }
    let mut fee = FeePaid::default();
    let mut covered = 0.0;
    for fill in fills {
        let f = fill.as_object()?;
        let asset = f.get("commissionAsset").and_then(Value::as_str)?;
        let amount = num_of(f.get("commission")?)?;
        covered += get_pos(f, "qty");
        if amount <= 0.0 {
            continue;
        }
        if !quote.is_empty() && asset == quote {
            fee.quote += amount;
        } else if !base.is_empty() && asset == base {
            fee.quote += amount * get_pos(f, "price");
        } else if let Some(row) = fee.other.iter_mut().find(|(a, _)| a == asset) {
            row.1 += amount;
        } else {
            fee.other.push((asset.to_string(), amount));
        }
    }
    is_close(covered, executed).then_some(fee)
}

impl BinanceBroker {
    /// 거래소 응답에서 체결 수량·평균가·실제 보유 수량을 뽑음. 보정 순서를 바꾸지 않음.
    pub async fn normalize(&self, order: &Value, closing: bool) -> Order {
        let empty = Map::new();
        let obj = order.as_object().unwrap_or(&empty);
        let executed = get_pos(obj, "executedQty");
        let mut average = get_pos(obj, "avgPrice");
        if average == 0.0 && executed != 0.0 {
            let quote = match obj.get("cummulativeQuoteQty") {
                Some(v) if py_truthy(v) => v.clone(),
                _ => obj.get("cumQuote").cloned().unwrap_or(Value::Null),
            };
            average = positive_number(&quote) / executed;
        }
        let response_fills: Vec<Value> = match obj.get("fills") {
            Some(Value::Array(rows)) => rows.clone(),
            _ => Vec::new(),
        };
        if average == 0.0 && executed != 0.0 {
            let quote: f64 = response_fills
                .iter()
                .filter_map(Value::as_object)
                .map(|f| get_pos(f, "price") * get_pos(f, "qty"))
                .sum();
            average = quote / executed;
        }

        let mut acquired = executed;
        let mut fees_known = true;
        // ensure_ready() 전이라도 심볼에서 기초자산을 끌어냄 — 이게 없으면 수수료가 조용히 안 빠짐.
        // 견적 통화를 모르는 심볼이면 빈 채로 둠(수수료를 모르는 것으로).
        let mut base_asset = self.base_asset();
        if base_asset.is_empty() {
            let (base, quote) = split_symbol(&self.symbol);
            if !quote.is_empty() {
                base_asset = base;
            }
        }
        let mut fills = response_fills;
        if self.market == Market::Spot && !closing && executed != 0.0 {
            let order_id = obj.get("orderId").filter(|v| !v.is_null());
            if fills.is_empty() {
                if let Some(order_id) = order_id {
                    let id = py_str(order_id);
                    let params = [("symbol", self.symbol.clone()), ("orderId", id.clone()), ("limit", "1000".into())];
                    fills = match self.get(false, "/api/v3/myTrades", &params, true).await {
                        Ok(Value::Array(trades)) => trades
                            .into_iter()
                            .filter(|t| py_str(t.get("orderId").unwrap_or(&Value::Null)) == id)
                            .collect(),
                        _ => Vec::new(),
                    };
                }
            }
            fees_known = !base_asset.is_empty()
                && !fills.is_empty()
                && fills.iter().all(|f| {
                    f.as_object()
                        .map(|o| o.contains_key("commissionAsset") && o.contains_key("commission"))
                        .unwrap_or(false)
                });
            let filled: f64 = fills.iter().filter_map(Value::as_object).map(|f| get_pos(f, "qty")).sum();
            fees_known = fees_known && is_close(filled, executed);
            if fees_known {
                let base_fee: f64 = fills
                    .iter()
                    .filter_map(Value::as_object)
                    .filter(|f| f.get("commissionAsset").and_then(Value::as_str) == Some(base_asset.as_str()))
                    .map(|f| get_pos(f, "commission"))
                    .sum();
                acquired = (executed - base_fee).max(0.0);
            }
        }
        // 화면에 보일 수수료 — 이미 받은 체결 목록만 씀(정리 주문을 위해 더 조회하지 않음). 선물은 모름.
        let fee = if self.market == Market::Spot && executed != 0.0 {
            spot_fills_fee(&fills, executed, &base_asset, self.quote_asset())
        } else if executed == 0.0 {
            Some(FeePaid::default())
        } else {
            None
        };
        Order {
            status: fold_status(obj.get("status")).to_string(),
            executed_qty: executed,
            avg_price: average,
            acquired_qty: acquired,
            fees_known,
            order_id: obj.get("orderId").filter(|v| !v.is_null()).map(py_str),
            raw_status: obj.get("status").map(py_str).unwrap_or_else(|| "unknown".into()),
            fee,
        }
    }
}
