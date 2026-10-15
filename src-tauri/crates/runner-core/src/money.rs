//! 주문 금액·수량·손익 계산. 수량 단위 내림·주문 금액·최소 진입 금액·주문 수량·금액 표기·손익(금액·%).
//! 로그·실행 기록의 숫자 표기는 서버와 같은 형식(파이썬 출력 모양)으로 맞춤(`py_*`).

use rust_decimal::prelude::{FromPrimitive, ToPrimitive};
use rust_decimal::{Decimal, RoundingStrategy};
use serde_json::Value;
use std::str::FromStr;

use crate::types::{order_cap, order_cap_basis, Market, Side};

/// 파이썬 `Decimal(str(x))` — 가장 짧은 십진 표기로 옮김.
pub fn decimal_of(x: f64) -> Option<Decimal> {
    if !x.is_finite() {
        return None;
    }
    Decimal::from_str(&format!("{x}")).ok().or_else(|| Decimal::from_f64(x))
}

/// 수량을 step의 배수로 내림. step이 0 이하면 그대로.
pub fn round_step(qty: f64, step: f64) -> f64 {
    if step <= 0.0 {
        return qty;
    }
    match (decimal_of(qty), decimal_of(step)) {
        (Some(q), Some(s)) if !s.is_zero() => {
            let n = (q / s).round_dp_with_strategy(0, RoundingStrategy::ToZero);
            (n * s).to_f64().unwrap_or(0.0)
        }
        _ => 0.0,
    }
}

/// 숫자로 읽히지 않거나 0 이하면 0 — 매크로 params는 사용자 JSON임.
pub fn positive(value: Option<&Value>) -> f64 {
    let n = match value {
        Some(Value::Number(n)) => n.as_f64().unwrap_or(0.0),
        Some(Value::String(s)) => s.trim().parse::<f64>().unwrap_or(0.0),
        Some(Value::Bool(true)) => 1.0,
        _ => 0.0,
    };
    if n.is_finite() && n > 0.0 {
        n
    } else {
        0.0
    }
}

/// 진입 예산에서 실제로 거래소에 나가는 주문 금액. 상한을 먼저 씌우고, 선물·증거금 기준이면 레버리지를 곱함.
pub fn notional_for(budget: f64, leverage: u32, market: Market, quote: &str) -> f64 {
    let cap = budget.min(order_cap(quote));
    if market == Market::Futures && order_cap_basis() == "margin" {
        cap * leverage as f64
    } else {
        cap
    }
}

/// 이 매크로의 진입 한 건이 쓸 수 있는 가장 작은 주문 금액(규칙 D: 그리드 한 칸, H: 마틴게일 기본/추가 주문 중 작은 쪽).
pub fn smallest_entry_notional(m: &Value, leverage: u32, market: Market, quote: &str) -> f64 {
    let empty = Value::Object(Default::default());
    let params = m.get("params").filter(|v| v.is_object()).unwrap_or(&empty);
    let risk = m.get("risk").filter(|v| v.is_object()).unwrap_or(&empty);
    let capital = match positive(params.get("initial_capital")) {
        x if x > 0.0 => x,
        _ => order_cap(quote),
    };
    let ratio = match positive(risk.get("invest_ratio")) {
        x if x > 0.0 => x,
        _ => 1.0,
    };
    let mut budget = capital * ratio;
    let rule = m.get("rule_type").and_then(Value::as_str).unwrap_or("A").to_uppercase();
    if rule == "D" {
        let per_grid = positive(params.get("per_grid_invest"));
        let grids = positive(params.get("grid_count")).trunc();
        budget = if per_grid > 0.0 {
            per_grid
        } else if grids >= 1.0 {
            budget / grids
        } else {
            budget
        };
    } else if rule == "H" {
        let mut base = match positive(params.get("base_order_size")) {
            x if x > 0.0 => x,
            _ => budget,
        };
        let mut safety = positive(params.get("safety_order_size"));
        let scale = match positive(params.get("safety_order_volume_scale")) {
            x if x > 0.0 => x,
            _ => 1.0,
        };
        let steps = positive(params.get("max_safety_orders")).trunc() as i64;
        if safety > 0.0 && steps > 0 {
            safety *= scale.min(1.0).powi((steps - 1).max(0) as i32);
            base = base.min(safety);
        }
        budget = base;
    }
    notional_for(budget, leverage, market, quote)
}

/// (수량, 수량×가격). 수량은 step으로 내림.
pub fn order_qty(price: f64, step: f64, budget: f64, leverage: u32, market: Market, quote: &str) -> (f64, f64) {
    let notional = notional_for(budget, leverage, market, quote);
    let qty = round_step(notional / price, step);
    (qty, qty * price)
}

fn group_thousands(int_part: &str) -> String {
    let bytes: Vec<char> = int_part.chars().collect();
    let mut out = String::new();
    for (i, c) in bytes.iter().enumerate() {
        if i > 0 && (bytes.len() - i).is_multiple_of(3) {
            out.push(',');
        }
        out.push(*c);
    }
    out
}

/// 화면·활동 문구의 반올림 — 부동소수 찌꺼기(1002.4999999999999)를 유효숫자 12자리로 먼저 걷어 내고
/// 소수 dp자리에서 0에서 먼 쪽으로 반올림함(3,120.5 → 3,121·-0.005 → -0.01).
/// 화면(src/state/format.ts `roundAway`)과 같은 규칙임. 숫자가 아니면 0.
pub fn round_away(x: f64, dp: u32) -> Decimal {
    let Some(d) = decimal_of(x) else { return Decimal::ZERO };
    let d = d.round_sf_with_strategy(12, RoundingStrategy::MidpointAwayFromZero).unwrap_or(d);
    let mut r = d.round_dp_with_strategy(dp, RoundingStrategy::MidpointAwayFromZero);
    if r.is_zero() {
        r = Decimal::ZERO;
    }
    r.rescale(dp);
    r
}

/// 표시 자릿수로 반올림하면 0인지 여부 — 0이면 부호·색을 붙이지 않음.
pub fn rounds_to_zero(x: f64, dp: u32) -> bool {
    round_away(x, dp).is_zero()
}

/// 돈의 표시 자릿수 — 원화는 정수, 그 밖은 소수 둘째 자리.
pub fn money_dp(quote: &str) -> u32 {
    if quote.eq_ignore_ascii_case("KRW") {
        0
    } else {
        2
    }
}

/// 부호 붙은 천 단위 숫자 `+1,234.50`·`-20,000`. 표시 자릿수로 반올림해 0이면 부호 없이 `0.00`.
pub fn signed_grouped(amount: f64, decimals: usize) -> String {
    let r = round_away(amount, decimals as u32);
    let sign = if r.is_zero() {
        ""
    } else if r.is_sign_negative() {
        "-"
    } else {
        "+"
    };
    let text = r.abs().to_string();
    let (int_part, frac) = match text.split_once('.') {
        Some((i, f)) => (i.to_string(), Some(f.to_string())),
        None => (text.clone(), None),
    };
    let mut out = format!("{sign}{}", group_thousands(&int_part));
    if let Some(f) = frac {
        out.push('.');
        out.push_str(&f);
    }
    out
}

/// 천 단위 숫자 `1,234.50` (양수에 부호 없음, 음수는 `-`).
pub fn grouped(amount: f64, decimals: usize) -> String {
    signed_grouped(amount, decimals).trim_start_matches('+').to_string()
}

/// 돈은 통화와 함께 씀. 원화에는 소수부가 없음. 0으로 반올림되면 부호 없이 `0 KRW`·`0.00 USDT`.
pub fn money(amount: f64, quote: &str) -> String {
    let unit = quote.to_uppercase();
    format!("{} {unit}", signed_grouped(amount, money_dp(&unit) as usize))
}

/// 부호 붙은 % — `+1.23%`·`-0.50%`, 0으로 반올림되면 `0.00%`.
pub fn signed_pct(pct: f64) -> String {
    format!("{}%", signed_grouped(pct, 2))
}

pub fn pnl_amount(qty: f64, entry: f64, price: f64, side: Side) -> f64 {
    match side {
        Side::Long => qty * (price - entry),
        Side::Short => qty * (entry - price),
    }
}

pub fn pnl_pct(entry: f64, price: f64, side: Side) -> f64 {
    if entry <= 0.0 {
        return 0.0;
    }
    let mv = (price - entry) / entry * 100.0;
    match side {
        Side::Long => mv,
        Side::Short => -mv,
    }
}

/// 사람이 읽는 가격 — 81,806 USDT·81,831.58 USDT·0.004512 USDT·115,200,000 KRW·0.0234 KRW.
/// 원화는 100원 이상이면 정수, 그 아래는 유효숫자 4자리(동전주가 "0"으로 보이지 않게). 반올림은 [`round_away`].
pub fn price_text(price: f64, quote: &str) -> String {
    let unit = quote.to_uppercase();
    let body = if !price.is_finite() || price <= 0.0 {
        "0".to_string()
    } else if unit == "KRW" && price >= 100.0 {
        grouped(price, 0)
    } else if unit == "KRW" {
        significant(price, 4)
    } else if price >= 1.0 {
        trim_zeros(grouped(price, 2))
    } else {
        py_g(price)
    };
    format!("{body} {unit}")
}

/// 유효숫자 n자리로 반올림(0에서 먼 쪽), 끝의 0 없이 — 0.0234·12.35·99.99.
pub fn significant(x: f64, n: u32) -> String {
    let Some(d) = decimal_of(x) else { return "0".into() };
    let d = d.round_sf_with_strategy(12, RoundingStrategy::MidpointAwayFromZero).unwrap_or(d);
    let r = d.round_sf_with_strategy(n, RoundingStrategy::MidpointAwayFromZero).unwrap_or(d).normalize();
    let text = r.to_string();
    let (int, frac) = text.split_once('.').map(|(i, f)| (i.to_string(), Some(f.to_string()))).unwrap_or((text, None));
    match frac {
        Some(f) => format!("{}.{f}", group_thousands(&int)),
        None => group_thousands(&int),
    }
}

fn trim_zeros(s: String) -> String {
    match s.split_once('.') {
        Some((int, frac)) => {
            let frac = frac.trim_end_matches('0');
            if frac.is_empty() {
                int.to_string()
            } else {
                format!("{int}.{frac}")
            }
        }
        None => s,
    }
}

/// 지수 없이, 끝의 0 없이 쓴 숫자(`0.00001`·`2`) — 파이썬 `_plain(Decimal(repr(x)))`와 같은 글자.
pub fn plain_number(x: f64) -> String {
    format!("{x}")
}

/// 사람이 읽는 수량 — 지수 표기 없이, 뒤 0 없이(최대 8자리).
pub fn qty_text(qty: f64) -> String {
    if !qty.is_finite() || qty <= 0.0 {
        return "0".into();
    }
    let s = format!("{qty:.8}");
    s.trim_end_matches('0').trim_end_matches('.').to_string()
}

/// 파이썬 `{x:g}` — 유효숫자 6자리. NaN·무한은 Rust 표기(`NaN`·`inf`) 그대로.
pub fn py_g(x: f64) -> String {
    py_gp(x, 6)
}

/// 파이썬 `{x:.{p}g}` — 유효숫자 p자리, 불필요한 0 제거, 지수는 e+XX.
pub fn py_gp(x: f64, p: usize) -> String {
    let p = p.max(1);
    if x == 0.0 {
        return "0".into();
    }
    if !x.is_finite() {
        return format!("{x}");
    }
    let sci = format!("{:.*e}", p - 1, x);
    let (_, e) = sci.split_once('e').unwrap_or((&sci, "0"));
    let exp: i32 = e.parse().unwrap_or(0);
    if exp < -4 || exp >= p as i32 {
        let (mant, _) = sci.split_once('e').unwrap_or((&sci, "0"));
        let mant = if mant.contains('.') { mant.trim_end_matches('0').trim_end_matches('.') } else { mant };
        return format!("{mant}e{}{:02}", if exp < 0 { "-" } else { "+" }, exp.abs());
    }
    let decimals = (p as i32 - 1 - exp).max(0) as usize;
    let s = format!("{:.*}", decimals, x);
    if s.contains('.') {
        s.trim_end_matches('0').trim_end_matches('.').to_string()
    } else {
        s
    }
}

/// 파이썬 str(float) 근사 — 정수면 "30.0", 아니면 가장 짧은 표기. 로그에 그대로 찍는 가격·수량.
pub fn py_num(x: f64) -> String {
    if x.is_finite() && x.fract() == 0.0 && x.abs() < 1e16 {
        format!("{x:.1}")
    } else {
        format!("{x}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn round_step_floors() {
        assert_eq!(round_step(1.23456, 0.001), 1.234);
        assert_eq!(round_step(0.3, 0.1), 0.3);
        assert_eq!(round_step(5.0, 0.0), 5.0);
    }

    #[test]
    fn krw_entry_quantity_uses_krw_cap() {
        let (qty, notional) = order_qty(15_000_000.0, 1e-8, 1_000_000.0, 1, Market::Spot, "KRW");
        assert!((qty - 0.01).abs() < 1e-9);
        assert!((notional - 150_000.0).abs() < 1e-6);
    }

    #[test]
    fn usdt_entry_quantity() {
        let (qty, notional) = order_qty(100.0, 0.001, 1000.0, 1, Market::Spot, "USDT");
        assert_eq!(qty, 1.0);
        assert_eq!(notional, 100.0);
    }

    #[test]
    fn smallest_entry_rules() {
        let a = json!({"params": {"initial_capital": 50}, "risk": {"invest_ratio": 0.5}});
        assert_eq!(smallest_entry_notional(&a, 1, Market::Spot, "USDT"), 25.0);
        let d = json!({"rule_type": "D", "params": {"initial_capital": 90, "grid_count": 3}});
        assert_eq!(smallest_entry_notional(&d, 1, Market::Spot, "USDT"), 30.0);
        let d2 = json!({"rule_type": "d", "params": {"initial_capital": 90, "per_grid_invest": 12}});
        assert_eq!(smallest_entry_notional(&d2, 1, Market::Spot, "USDT"), 12.0);
        let h = json!({"rule_type": "H", "params": {"base_order_size": 40, "safety_order_size": 20,
                       "safety_order_volume_scale": 0.5, "max_safety_orders": 3}});
        assert_eq!(smallest_entry_notional(&h, 1, Market::Spot, "USDT"), 5.0);
        // 기본 자본은 통화 상한
        assert_eq!(smallest_entry_notional(&json!({}), 1, Market::Spot, "KRW"), 150_000.0);
    }

    #[test]
    fn money_format() {
        assert_eq!(money(1234.5, "USDT"), "+1,234.50 USDT");
        assert_eq!(money(-20000.4, "KRW"), "-20,000 KRW");
        assert_eq!(money(0.0, "krw"), "0 KRW");
    }

    #[test]
    fn display_rounding_is_half_away_from_zero_and_matches_the_screen() {
        // 화면(src/state/format.test.ts)과 같은 숫자·같은 결과.
        assert_eq!(price_text(3120.6, "KRW"), "3,121 KRW", "내림이 아니라 반올림");
        assert_eq!(price_text(3120.4, "KRW"), "3,120 KRW");
        assert_eq!(price_text(3120.5, "KRW"), "3,121 KRW");
        assert_eq!(price_text(0.0234, "KRW"), "0.0234 KRW", "100원 아래는 유효숫자 4자리");
        assert_eq!(price_text(12.345, "KRW"), "12.35 KRW");
        assert_eq!(price_text(99.99, "KRW"), "99.99 KRW");
        assert_eq!(price_text(5.0, "KRW"), "5 KRW");
        assert_eq!(price_text(0.000_123_456, "KRW"), "0.0001235 KRW");
        assert_eq!(money(1002.5, "KRW"), "+1,003 KRW");
        assert_eq!(money(500.0 + 1_005_000.0 * 0.0005, "KRW"), "+1,003 KRW", "부동소수 찌꺼기를 걷고 반올림");
        assert_eq!(money(3997.5, "KRW"), "+3,998 KRW");
        assert_eq!(money(-3997.5, "KRW"), "-3,998 KRW");
        assert_eq!(money(1.005, "USDT"), "+1.01 USDT");
        assert_eq!(money(-0.125, "USDT"), "-0.13 USDT");
    }

    #[test]
    fn values_that_round_to_zero_have_no_sign() {
        assert_eq!(money(-0.004, "USDT"), "0.00 USDT");
        assert_eq!(money(0.004, "USDT"), "0.00 USDT");
        assert_eq!(money(-0.4, "KRW"), "0 KRW");
        assert_eq!(money(-0.005, "USDT"), "-0.01 USDT");
        assert_eq!(signed_pct(-0.004), "0.00%");
        assert_eq!(signed_pct(1.234), "+1.23%");
        assert_eq!(signed_pct(-0.5), "-0.50%");
        assert!(rounds_to_zero(-0.004, money_dp("USDT")));
        assert!(!rounds_to_zero(-0.005, money_dp("USDT")));
        assert!(rounds_to_zero(-0.49, money_dp("krw")));
        assert_eq!(grouped(-20000.4, 0), "-20,000");
        assert_eq!(grouped(150000.0, 0), "150,000");
    }

    #[test]
    fn pnl() {
        assert_eq!(pnl_amount(2.0, 100.0, 110.0, Side::Long), 20.0);
        assert_eq!(pnl_amount(2.0, 100.0, 110.0, Side::Short), -20.0);
        assert!((pnl_pct(100.0, 110.0, Side::Short) + 10.0).abs() < 1e-9);
        assert_eq!(pnl_pct(0.0, 110.0, Side::Long), 0.0);
    }

    #[test]
    fn python_number_strings() {
        assert_eq!(py_num(30.0), "30.0");
        assert_eq!(py_num(2.5), "2.5");
        assert_eq!(py_g(0.01), "0.01");
        assert_eq!(py_g(65000.0), "65000");
        assert_eq!(py_g(1234567.0), "1.23457e+06");
        assert_eq!(py_num(65000.0), "65000.0");
        assert_eq!(price_text(81806.0, "USDT"), "81,806 USDT");
        assert_eq!(price_text(81831.58, "USDT"), "81,831.58 USDT");
        assert_eq!(price_text(81831.5, "USDT"), "81,831.5 USDT");
        assert_eq!(price_text(81800.0, "USDT"), "81,800 USDT");
        assert_eq!(price_text(115_200_000.4, "KRW"), "115,200,000 KRW");
        assert_eq!(price_text(0.004512, "USDT"), "0.004512 USDT");
        assert_eq!(qty_text(0.00122), "0.00122");
        assert_eq!(qty_text(9.39e-6), "0.00000939");
        assert_eq!(py_num(0.0123), "0.0123");
        assert_eq!(py_gp(0.000_000_005, 12), "5e-09");
        assert_eq!(py_gp(1.5, 12), "1.5");
        assert_eq!(py_g(99999.95), "99999.9");
    }
}
