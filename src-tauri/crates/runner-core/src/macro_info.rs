//! 매크로 원문에서 화면·검사에 쓰는 정보를 읽음 — 전략 이름, 이 앱에서 실행할 수 있는지, 투자 금액.
//! 전략 이름과 '지원하지 않음' 판단은 웹(gg_parrot frontend/src/lib/macro.js)과 서버
//! (backend/app/runner.py RUNNER_UNSUPPORTED_RULES·is_portfolio)를 따름 — 서버가 시작을 거절할 매크로를
//! 시작 버튼을 누르기 전에 알려 주려는 것임.

use serde_json::Value;

/// 매크로 유형 글자(A~K). 없거나 모르면 None.
pub fn rule_type(m: &Value) -> Option<char> {
    let raw = m.get("rule_type")?.as_str()?.trim();
    let mut chars = raw.chars();
    let c = chars.next()?.to_ascii_uppercase();
    (chars.next().is_none() && ('A'..='K').contains(&c)).then_some(c)
}

/// 웹과 같은 전략 이름(유형 글자 없이).
pub fn strategy_label(rule: char) -> &'static str {
    match rule {
        'A' => "익절/손절 후 재진입",
        'B' => "지정가 밴드 매매",
        'C' => "정기 분할매수(DCA)",
        'D' => "그리드 매매",
        'E' => "트레일링 스탑",
        'F' => "RSI 조건 매매",
        'G' => "볼린저밴드",
        'H' => "마틴게일 / 세이프티오더",
        'I' => "변동성 돌파",
        'J' => "이동평균 크로스",
        'K' => "하락 방어 전환(SAR)",
        _ => "",
    }
}

/// 국내 거래소(업비트·빗썸)에서 숏·레버리지 매크로를 거절하는 이유.
pub const DOMESTIC_SPOT_ONLY: &str = "국내 거래소(업비트 · 빗썸)는 원화 현물 롱 · 1배만 실행할 수 있어요.";

/// 서버가 세션 시작을 거절하거나 이 앱이 돌릴 수 없는 매크로면 그 이유. 돌릴 수 있으면 None.
pub fn unsupported_reason(m: &Value) -> Option<&'static str> {
    if matches!(rule_type(m), Some('C') | Some('K')) {
        return Some("이 앱은 아직 이 매크로 유형(정기 분할매수 · 하락 방어 전환)을 지원하지 않아요.");
    }
    // 국내 거래소는 원화 현물이라 숏·레버리지를 낼 수 없음 — [시작]을 누르기 전에 알림.
    let domestic = crate::types::Exchange::parse(&crate::types::exchange_name_of(m)).is_some_and(|e| e.is_domestic());
    let leveraged = crate::types::macro_leverage(m).is_ok_and(|l| l > 1);
    if domestic && (crate::types::macro_side(m) != crate::types::Side::Long || leveraged) {
        return Some(DOMESTIC_SPOT_ONLY);
    }
    let many_symbols = m.get("symbols").and_then(Value::as_array).is_some_and(|s| s.len() > 1);
    let legs = m.get("legs").and_then(Value::as_array).is_some_and(|l| !l.is_empty());
    if many_symbols || legs {
        return Some("이 앱은 아직 여러 종목을 묶은 매크로를 지원하지 않아요. 종목 하나씩 나눠 실행하세요.");
    }
    None
}

/// 투자 금액(`params.initial_capital`, 견적 통화 단위). 없거나 0 이하면 None.
pub fn initial_capital(m: &Value) -> Option<f64> {
    let v = m.get("params")?.get("initial_capital")?;
    let n = match v {
        Value::Number(n) => n.as_f64()?,
        Value::String(s) => s.trim().parse().ok()?,
        _ => return None,
    };
    (n.is_finite() && n > 0.0).then_some(n)
}

/// 이 앱이 로컬에서 지키는 안전 설정(`risk`)을 사람이 읽는 조각으로 — 손절·일일 최대 손실·최대 보유 시간.
/// 값이 없거나 0이면 뺌. 1회 투자 비율(invest_ratio)은 넣지 않음 — 진입 금액은 서버 명령(notional_frac)이
/// 정하고, 이 앱은 시작 전 확인 금액과 하루 손실 한도의 기준 금액에만 씀(주문마다 지키는 규칙이 아님).
pub fn risk_summary(m: &Value) -> Vec<String> {
    let Some(risk) = m.get("risk").filter(|r| r.is_object()) else { return vec![] };
    let num = |k: &str| risk.get(k).and_then(Value::as_f64).filter(|v| v.is_finite() && *v > 0.0);
    let mut out = vec![];
    // 손절은 가격 기준(평단 대비 가격), 하루 최대 손실은 수수료 반영 손익 기준 — 두 기준을 문구로 구분함.
    if let Some(v) = num("stop_loss_pct") {
        out.push(format!("손절 평단 대비 가격 -{}%", pct_text(v)));
    }
    if let Some(v) = num("daily_max_loss_pct") {
        out.push(format!("하루 최대 손실 {}%(수수료 포함)", pct_text(v)));
    }
    if let Some(v) = num("max_holding_hours") {
        out.push(format!("최대 보유 {}시간", pct_text(v)));
    }
    out
}

/// 안전 설정을 이름 · 값 두 칸으로 — 화면의 칸 표(투자 금액 옆)에 씀. 문구는 [`risk_summary`]와 같은 낱말.
/// ("손절", "평단 대비 가격 -3%") · ("하루 최대 손실", "5%(수수료 포함)") · ("최대 보유", "24시간").
pub fn risk_items(m: &Value) -> Vec<(String, String)> {
    let Some(risk) = m.get("risk").filter(|r| r.is_object()) else { return vec![] };
    let num = |k: &str| risk.get(k).and_then(Value::as_f64).filter(|v| v.is_finite() && *v > 0.0);
    let mut out = vec![];
    if let Some(v) = num("stop_loss_pct") {
        out.push(("손절".into(), format!("평단 대비 가격 -{}%", pct_text(v))));
    }
    if let Some(v) = num("daily_max_loss_pct") {
        out.push(("하루 최대 손실".into(), format!("{}%(수수료 포함)", pct_text(v))));
    }
    if let Some(v) = num("max_holding_hours") {
        out.push(("최대 보유".into(), format!("{}시간", pct_text(v))));
    }
    out
}

/// 봉 간격 그대로("1h" · "1d" 등). 없으면 서버 기본값과 같은 "1d".
pub fn candle_interval(m: &Value) -> String {
    match m.get("candle_interval").and_then(Value::as_str).map(str::trim) {
        Some(s) if !s.is_empty() => s.to_string(),
        _ => "1d".to_string(),
    }
}

/// 봉 간격(`candle_interval`)의 이름 — "4h" → "4시간"(서버 entry_wait.py INTERVAL_LABEL과 같음).
/// 매크로에 없거나 모르는 값이면 "" — 봉을 쓰지 않는 매크로에 간격을 지어 보이지 않음.
pub fn candle_interval_label(m: &Value) -> &'static str {
    match m.get("candle_interval").and_then(Value::as_str).map(str::trim).unwrap_or("") {
        "1m" => "1분",
        "3m" => "3분",
        "5m" => "5분",
        "15m" => "15분",
        "30m" => "30분",
        "1h" => "1시간",
        "2h" => "2시간",
        "4h" => "4시간",
        "6h" => "6시간",
        "8h" => "8시간",
        "12h" => "12시간",
        "1d" => "1일",
        "3d" => "3일",
        "1w" => "1주",
        _ => "",
    }
}

/// %·시간 값을 소수 둘째 자리까지, 끝의 0 없이 — 5·2.5·0.33 (웹 `fmtPct`와 같음).
pub fn pct_text(v: f64) -> String {
    let s = crate::money::round_away(v, 2).normalize().to_string();
    if s == "-0" {
        "0".into()
    } else {
        s
    }
}

/// 투자 금액을 바꿈. 다른 값은 건드리지 않음.
pub fn set_initial_capital(m: &mut Value, amount: f64) -> Result<(), String> {
    if !amount.is_finite() || amount <= 0.0 {
        return Err("투자 금액은 0보다 커야 해요.".into());
    }
    if amount > 1_000_000_000.0 {
        return Err("투자 금액은 10억 이하로 넣어 주세요.".into());
    }
    let params = m
        .as_object_mut()
        .ok_or("매크로 형식이 올바르지 않아요.")?
        .entry("params")
        .or_insert_with(|| Value::Object(Default::default()));
    let obj = params.as_object_mut().ok_or("매크로 형식이 올바르지 않아요.")?;
    obj.insert("initial_capital".into(), serde_json::json!(amount));
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn risk_items_split_name_and_value_with_the_same_words() {
        let m = json!({"risk": {"stop_loss_pct": 3, "daily_max_loss_pct": 5, "max_holding_hours": 24, "cooldown_minutes": 30}});
        let pairs: Vec<(String, String)> = risk_items(&m);
        assert_eq!(
            pairs,
            vec![
                ("손절".to_string(), "평단 대비 가격 -3%".to_string()),
                ("하루 최대 손실".to_string(), "5%(수수료 포함)".to_string()),
                ("최대 보유".to_string(), "24시간".to_string()),
            ]
        );
        let joined: Vec<String> = pairs.iter().map(|(k, v)| format!("{k} {v}")).collect();
        assert_eq!(joined, risk_summary(&m), "한 줄 요약과 같은 낱말");
        assert!(risk_items(&json!({"risk": {"stop_loss_pct": 0}})).is_empty());
        assert!(risk_items(&json!({})).is_empty());
    }

    #[test]
    fn candle_interval_label_matches_server_names_and_hides_unknown() {
        assert_eq!(candle_interval_label(&json!({"candle_interval": "4h"})), "4시간");
        assert_eq!(candle_interval_label(&json!({"candle_interval": "1d"})), "1일");
        assert_eq!(candle_interval_label(&json!({"candle_interval": "7h"})), "");
        assert_eq!(candle_interval_label(&json!({})), "", "없으면 지어 보이지 않음");
    }

    #[test]
    fn rule_and_label() {
        assert_eq!(rule_type(&json!({"rule_type": "f"})), Some('F'));
        assert_eq!(rule_type(&json!({"rule_type": "Z"})), None);
        assert_eq!(rule_type(&json!({"rule_type": "AB"})), None);
        assert_eq!(rule_type(&json!({})), None);
        assert_eq!(strategy_label('F'), "RSI 조건 매매");
    }

    #[test]
    fn unsupported_matches_the_server() {
        assert!(unsupported_reason(&json!({"rule_type": "C"})).unwrap().contains("정기 분할매수"));
        assert!(unsupported_reason(&json!({"rule_type": "K"})).is_some());
        assert!(unsupported_reason(&json!({"rule_type": "A", "symbols": ["BTCUSDT", "ETHUSDT"]})).is_some());
        assert!(unsupported_reason(&json!({"rule_type": "A", "legs": [{"symbol": "X"}]})).is_some());
        assert!(unsupported_reason(&json!({"rule_type": "A", "symbols": ["BTCUSDT"]})).is_none());
        assert!(unsupported_reason(&json!({"rule_type": "F", "legs": null})).is_none());
        let upbit = |extra: Value| {
            let mut m = json!({"rule_type": "F", "exchange": "upbit", "symbol": "KRW-BTC"});
            m.as_object_mut().unwrap().extend(extra.as_object().unwrap().clone());
            unsupported_reason(&m)
        };
        assert_eq!(upbit(json!({"position_side": "short"})), Some(DOMESTIC_SPOT_ONLY));
        assert_eq!(upbit(json!({"leverage": 3})), Some(DOMESTIC_SPOT_ONLY));
        assert_eq!(upbit(json!({})), None);
    }

    #[test]
    fn risk_summary_reads_local_safety_settings() {
        let m = json!({"risk": {"stop_loss_pct": 3.0, "daily_max_loss_pct": 0, "max_holding_hours": 24, "invest_ratio": 0.5}});
        assert_eq!(
            risk_summary(&m),
            vec!["손절 평단 대비 가격 -3%", "최대 보유 24시간"],
            "투자 비율은 로컬 안전 규칙이 아니다"
        );
        assert!(risk_summary(&json!({})).is_empty());
        assert_eq!(risk_summary(&json!({"risk": {"stop_loss_pct": 2.5}})), vec!["손절 평단 대비 가격 -2.5%"]);
        assert_eq!(
            risk_summary(&json!({"risk": {"daily_max_loss_pct": 5}})),
            vec!["하루 최대 손실 5%(수수료 포함)"],
            "하루 최대 손실은 수수료 반영 손익으로 봄"
        );
    }

    #[test]
    fn capital_read_and_write() {
        let mut m = json!({"rule_type": "F", "params": {"initial_capital": 1000, "rsi_period": 14}});
        assert_eq!(initial_capital(&m), Some(1000.0));
        set_initial_capital(&mut m, 250.5).unwrap();
        assert_eq!(initial_capital(&m), Some(250.5));
        assert_eq!(m["params"]["rsi_period"], 14, "다른 값은 그대로");
        assert!(set_initial_capital(&mut m, 0.0).is_err());
        assert!(set_initial_capital(&mut m, f64::NAN).is_err());
        assert_eq!(initial_capital(&json!({"params": {"initial_capital": "0"}})), None);
    }
}
