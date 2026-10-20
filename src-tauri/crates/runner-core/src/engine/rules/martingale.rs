//! **H — 마틴게일 / 안전주문.** 기준 매수로 들어가고, 내려갈 때마다 더 산다.
//!
//! 평단이 내려가므로 조금만 반등해도 익절선(`take_profit`%)에 닿는다. 대신 **더 내려가면
//! 손실이 커진다** — 안전주문 횟수(`max_safety_orders`)가 그 상한이다.
//!
//! 안전주문 가격은 **기준 매수가**에서 누적으로 내려가며, 두 가지가 단계마다 커질 수 있다:
//! * `safety_order_step_scale` — 다음 단계까지의 **간격**이 넓어진다.
//! * `safety_order_volume_scale` — 다음 단계의 **금액**이 커진다.
//!
//! 한 봉이 여러 단계를 지나면(급락) 그 봉에서 여러 단계를 한꺼번에 산다.
//!
//! 지금까지 채운 단계 수와 기준 매수가는 **봉에서 다시 구할 수 없다.** 잊으면 차수가
//! 0 으로 돌아가 이미 채운 단계를 다시 산다.

use crate::engine::candle_sim::{Bar, SimContext, Strategy};

#[derive(Debug, Clone)]
pub struct Martingale {
    /// 기준 매수 금액.
    pub base_order_size: f64,
    /// 안전주문 한 번의 기본 금액.
    pub safety_order_size: f64,
    /// 단계 사이의 하락 폭(%).
    pub price_deviation: f64,
    /// 단계마다 간격에 곱하는 값.
    pub step_scale: f64,
    /// 단계마다 금액에 곱하는 값.
    pub volume_scale: f64,
    /// 안전주문 최대 횟수.
    pub max_safety_orders: u32,
    /// 평단 대비 익절(%).
    pub take_profit: f64,

    /// 지금까지 채운 안전주문 수.
    so_done: u32,
    /// 기준 매수의 평단 — 안전주문 가격의 기준이다.
    base_price: f64,
}

impl Martingale {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        base_order_size: f64,
        safety_order_size: f64,
        price_deviation: f64,
        step_scale: f64,
        volume_scale: f64,
        max_safety_orders: u32,
        take_profit: f64,
    ) -> Self {
        Self {
            base_order_size,
            safety_order_size,
            price_deviation,
            step_scale,
            volume_scale,
            max_safety_orders,
            take_profit,
            so_done: 0,
            base_price: 0.0,
        }
    }

    /// 다음 안전주문이 걸리는 가격. 화면에 그리는 선이다.
    ///
    /// 단계마다 간격이 `step_scale` 배로 넓어지므로 **누적**으로 더한다 — 단순히
    /// `하락폭 × 차수` 가 아니다.
    pub fn next_safety_price(&self) -> f64 {
        let mut cumulative = 0.0;
        let mut step = self.price_deviation;
        for _ in 0..=self.so_done {
            cumulative += step;
            step *= self.step_scale;
        }
        self.base_price * (1.0 - cumulative / 100.0)
    }

    /// 지금 익절선(평단 기준).
    pub fn take_profit_price(&self, avg_entry: f64) -> f64 {
        avg_entry * (1.0 + self.take_profit / 100.0)
    }

    pub fn safety_orders_done(&self) -> u32 {
        self.so_done
    }

    pub fn base_price(&self) -> f64 {
        self.base_price
    }

    /// 재기동 복구용 — 체크포인트가 실어 보낸 값으로 되돌린다.
    pub fn restore(&mut self, so_done: u32, base_price: f64) {
        self.so_done = so_done;
        self.base_price = base_price;
    }
}

impl Strategy for Martingale {
    fn on_bar(&mut self, ctx: &mut SimContext<'_>, bar: &Bar) {
        if ctx.common_risk(bar) {
            // 손절로 끌려 나갔다 — 다음 진입은 처음부터 다시 센다.
            self.so_done = 0;
            return;
        }

        if ctx.in_position() {
            // 익절이 먼저다. 손절은 위 `common_risk` 에서 이미 봤다 — 한 봉이 둘 다
            // 스쳤으면 손절이 이긴다(보수적으로).
            let target = self.take_profit_price(ctx.avg_entry());
            if bar.h >= target {
                ctx.close_all(target, target, false, "");
                self.so_done = 0;
                return;
            }
            // 내려가는 길의 안전주문. 한 봉이 여러 단계를 지날 수 있다.
            while self.so_done < self.max_safety_orders {
                let price = self.next_safety_price();
                if bar.l > price || ctx.cash() <= 1e-9 {
                    break;
                }
                let size = self.safety_order_size * self.volume_scale.powi(self.so_done as i32);
                ctx.open_long(size, price, bar.c, "진입");
                // 주문이 안 나갔어도 차수는 올린다 — 서버와 같다. 돈이 없어 못 산 단계를
                // 다음 봉마다 다시 시도하면 같은 자리에서 영영 맴돈다.
                self.so_done += 1;
            }
            return;
        }

        // 평평하다 — 기준 매수.
        if ctx.entry_blocked() {
            return;
        }
        if ctx.open_long(self.base_order_size, bar.c, bar.c, "진입") {
            self.base_price = ctx.avg_entry();
            self.so_done = 0;
        }
    }

    fn on_warmup_reset(&mut self) {
        self.so_done = 0;
        self.base_price = 0.0;
    }
}
