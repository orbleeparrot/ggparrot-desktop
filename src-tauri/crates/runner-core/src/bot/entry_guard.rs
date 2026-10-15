//! 진입 전 확인 — 서버 진입 명령을 실주문 전에 한 번 더 봄(투자 비율·신호 유효 시간·신호 가격·총 투자 한도).

use super::*;

impl Bot {
    /// 진입 명령을 받지 않을 이유 — 서버 신호를 그대로 믿지 않고 실주문 전에 한 번 더 봄. 없으면 None.
    pub(super) fn entry_refusal(&self, cmd: &Value, price: f64) -> Option<String> {
        let frac = num(cmd.get("notional_frac"));
        if !(frac.is_finite() && frac > 0.0) {
            return Some("명령의 투자 비율이 올바르지 않아요".into());
        }
        // 서버는 진입 명령에 만료 시각을 달아 보냄 — 끊겼다 돌아와 늦게 받은 진입은 하지 않음.
        let expires = num(cmd.get("expires_ms"));
        if expires > 0.0 && wall_ms() as f64 > expires + ENTRY_EXPIRY_GRACE_MS as f64 {
            return Some("진입 신호의 유효 시간이 지났어요".into());
        }
        let signal = num(cmd.get("signal_price"));
        if signal > 0.0 && price > 0.0 {
            let gap = (price - signal).abs() / signal;
            if gap > MAX_SIGNAL_DEVIATION {
                return Some(format!(
                    "신호 가격({})과 지금 가격({})이 {:.1}% 달라 진입하지 않았어요",
                    price_text(signal, self.quote),
                    price_text(price, self.quote),
                    gap * 100.0
                ));
            }
        }
        // 총 투자 한도 — 보유 중 추가 진입(그리드·마틴게일)이 매크로 투자 금액을 넘지 않게. 선물은 증거금 기준.
        let notional = (self.capital * frac.min(1.0)).min(order_cap(self.quote));
        let lev = if self.market == Market::Futures { self.leverage.max(1) as f64 } else { 1.0 };
        let in_use = if self.in_position { self.held_qty * self.entry_price / lev } else { 0.0 };
        if self.in_position && in_use + notional > self.capital * EXPOSURE_TOLERANCE {
            return Some(format!(
                "이미 {} 투자 중이라 투자 금액({})을 넘는 추가 진입은 하지 않아요",
                money(in_use, self.quote).trim_start_matches('+'),
                money(self.capital, self.quote).trim_start_matches('+')
            ));
        }
        None
    }
}
