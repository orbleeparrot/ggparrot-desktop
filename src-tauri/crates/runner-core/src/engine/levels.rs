//! 가격선 — "지금 어디서 사고 어디서 파는가".
//!
//! 실행기 차트와 웹 화면이 **같은 값**을 써야 한다. 화면이 직접 계산하면 규칙 산수가 두
//! 벌이 되고, 실제로 그 일이 있었다 — E 눌림목의 '진입 목표 −3%' 선을 웹이 **현재가**
//! 기준으로 그려 가격을 따라 영원히 내려왔고(엔진은 평평해진 순간 잡힌 고정 기준가를
//! 본다), 사용자는 "안 사진다" 로 읽었다.
//!
//! 그래서 선은 **엔진 상태에서만** 나온다. 모르면 `None` — 추측해서 그리면 거짓이 된다.

use serde::Serialize;

/// 지금 그려야 할 선들. 모르는 선은 `None` 이고, 화면은 그 선을 그리지 않는다.
#[derive(Debug, Clone, Default, PartialEq, Serialize)]
pub struct Levels {
    /// **사는 선** — 여기 닿으면 산다(돌파선 · 눌림목 · 지정가).
    pub entry_target: Option<f64>,
    /// **파는 선** — 여기 닿으면 판다(익절 · 지정가 · 격자 한 칸 위).
    pub exit_target: Option<f64>,
    /// 손절선(평단 기준).
    pub stop_loss: Option<f64>,
    /// 트레일이 **발동한 뒤** 의 손절선(고점 기준).
    pub trail_stop: Option<f64>,
    /// 트레일이 **켜지는** 가격(평단 기준). 켜진 뒤에는 `None`.
    pub trail_activation: Option<f64>,
    /// 트레일이 켜졌는가.
    pub armed: bool,
    /// 기준선 — 이동평균 · 밴드 중심처럼 '조건' 으로 쓰이는 값.
    pub reference: Option<f64>,
    /// 보조선 두 개(볼린저 위 · 아래, 이동평균 둘).
    pub band_upper: Option<f64>,
    pub band_lower: Option<f64>,
    /// 그리드 칸 — 사려고 기다리는 가격들.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub buy_levels: Vec<f64>,
    /// 그리드 칸 — 팔려고 기다리는 가격들.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub sell_levels: Vec<f64>,
    /// 다음 적립 매수 예정 시각(ms). 적립식만.
    pub next_buy_ms: Option<i64>,
    /// 가격이 **아닌** 지표(RSI 등). 화면은 가격축이 아니라 아래 칸에 따로 그린다 —
    /// 0~100 짜리 값을 가격축에 섞으면 축이 망가진다.
    pub indicator: Option<Indicator>,
    /// 아직 첫 봉이 마감되지 않아 값이 없다 — "서버가 이 규칙에 관심 없음" 과 다르다.
    ///
    /// 둘을 구분하지 않으면 화면이 빈칸을 미리보기(현재가 기준 추정)로 메우고, 그 추정선이
    /// 실제 진입 목표인 척한다. 같은 거짓의 두 번째 입구였다.
    pub pending: bool,
}

/// 가격이 아닌 지표 하나. 값과 함께 **어느 선에서 사고파는지**를 싣는다 —
/// 값만 보여 주면 사용자가 "지금 조건에 가까운가" 를 알 수 없다.
#[derive(Debug, Clone, Default, PartialEq, Serialize)]
pub struct Indicator {
    /// 화면에 쓰는 이름("RSI").
    pub name: &'static str,
    pub value: f64,
    /// 사는 문턱 · 파는 문턱.
    pub entry_threshold: Option<f64>,
    pub exit_threshold: Option<f64>,
}

impl Levels {
    /// 아직 아무 값도 못 내는 상태.
    pub fn pending() -> Self {
        Self { pending: true, ..Default::default() }
    }

    /// 그릴 선이 하나라도 있는가.
    pub fn has_any(&self) -> bool {
        self.entry_target.is_some()
            || self.exit_target.is_some()
            || self.stop_loss.is_some()
            || self.trail_stop.is_some()
            || self.trail_activation.is_some()
            || self.reference.is_some()
            || !self.buy_levels.is_empty()
            || !self.sell_levels.is_empty()
            || self.indicator.is_some()
    }
}

/// 0 이하 · 유한하지 않은 값은 선이 아니다 — 그리면 축이 망가진다.
pub fn price(value: f64) -> Option<f64> {
    if value.is_finite() && value > 0.0 { Some(value) } else { None }
}
