//! 로그 한 줄의 종류·아이콘·표시 모양.

pub fn event_kind(msg: &str) -> &'static str {
    let m = msg.trim();
    // 실패 문장에도 '체결'이 자주 들어감 — 낱말 하나로 실패가 체결처럼 보이면 안 됨.
    if m.contains("오류") || m.contains("실패") {
        return "error";
    }
    if ["미체결", "불확실", "확인하지 못", "체결되지"].iter().any(|w| m.contains(w)) {
        return "warn";
    }
    if m.starts_with("[진입]") || m.starts_with("[청산") || m.starts_with("[강제청산") || m.starts_with("[손절]")
    {
        return "order";
    }
    if m.contains("체결") || m.trim_start().starts_with("손익") {
        return "fill";
    }
    if m.contains("종료") {
        return "stop";
    }
    if m.contains("진입 보류") || m.contains("신호") {
        return "signal";
    }
    "info"
}

pub fn icon_of(kind: &str) -> &'static str {
    match kind {
        "signal" => "🔔",
        "order" => "▶",
        "fill" => "✓",
        "error" | "warn" => "⚠",
        "stop" => "■",
        _ => "·",
    }
}

/// (종류, 아이콘, 화면 문장). 메시지가 이미 같은 기호로 시작하면 두 번 붙이지 않음.
pub fn log_style(msg: &str) -> (&'static str, &'static str, String) {
    let kind = event_kind(msg);
    let icon = icon_of(kind);
    let text = msg.trim();
    let text = text.strip_prefix(icon).map(str::trim).unwrap_or(text).to_string();
    (kind, icon, text)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn failure_is_not_fill() {
        assert_eq!(event_kind("체결 확인 실패"), "error");
        assert_eq!(event_kind("주문 상태 OPEN — 체결 완료를 확인하지 못했습니다"), "warn");
        assert_eq!(event_kind("  ✓ BUY 체결: id=1 수량=0.1"), "fill");
        assert_eq!(event_kind("[신호] 골든크로스 → BUY"), "signal");
        assert_eq!(event_kind("[손절] 100"), "order");
        assert_eq!(event_kind("종료됨 (stopped)"), "stop");
        assert_eq!(event_kind("손익 +1.00%"), "fill");
        assert_eq!(event_kind("세션 시작"), "info");
    }

    #[test]
    fn icon_not_doubled() {
        let (_, icon, text) = log_style("⚠ 이 매크로 파일은 웹에서 받은 원본과 달라요.");
        assert_eq!(icon, "·");
        assert!(text.starts_with('⚠'));
        let (kind, icon, text) = log_style("  ✓ SELL 체결: id=2");
        assert_eq!((kind, icon), ("fill", "✓"));
        assert_eq!(text, "SELL 체결: id=2");
    }
}
