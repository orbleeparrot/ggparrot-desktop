//! 매매 판단 엔진 — 서버(파이썬 `backend/engine/`)와 **같은 답**을 내야 하는 부분.
//!
//! 판단이 실행기로 내려오면 엔진이 두 언어로 존재함. "같은 코드를 쓴다"가 성립하지
//! 않으므로, 같은 입력에 같은 답을 내는지 계약으로 잼 —
//! `tests/fixtures/engine_golden.json`, `tests/engine_golden.rs`.
//!
//! 구조는 서버와 같음:
//!
//! * [`ledger`] — 돈과 수량을 세는 계산기(슬리피지·수수료·평단·분할 청산·손절선).
//!   규칙 여섯이 전부 이 위에 올라탐.
//! * 규칙(D·E·F·G·H·I) — "언제 살까"만 정함. 아직 포팅 전.

pub mod candle_sim;
pub mod indicators;
pub mod ledger;
pub mod levels;
pub mod rules;
pub mod runner;
pub mod series;
pub mod tick_sim;

/// 이 실행기에 묶여 있는 판단 코드의 세대. 서버(`engine/version.py`의 `ENGINE_VERSION`)와
/// 같은 숫자를 셈 — 세션을 시작할 때 서버에 알리고, 서버가 더 높은 세대를 요구하면
/// 426으로 막음. 판단 계약(`tests/fixtures/engine_golden.json`)의 값과도 같아야 하며,
/// 다르면 `tests/engine_golden.rs`가 먼저 깨짐.
pub const ENGINE_VERSION: u32 = 1;
