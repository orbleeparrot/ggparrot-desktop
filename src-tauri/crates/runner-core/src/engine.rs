//! 매매 판단 엔진 — 서버(파이썬 `backend/engine/`)와 **같은 답**을 내야 하는 부분.
//!
//! 판단이 실행기로 내려오면 엔진이 두 언어로 존재한다. "같은 코드를 쓴다" 가 성립하지
//! 않으므로, 같은 입력에 같은 답을 내는지 계약으로 잰다 —
//! `tests/fixtures/engine_golden.json`, `tests/engine_golden.rs`.
//!
//! 구조는 서버와 같다:
//!
//! * [`ledger`] — 돈과 수량을 세는 계산기(슬리피지 · 수수료 · 평단 · 분할 청산 · 손절선).
//!   규칙 여섯이 전부 이 위에 올라탄다.
//! * 규칙(D · E · F · G · H · I) — "언제 살까" 만 정한다. 아직 포팅 전.

pub mod candle_sim;
pub mod ledger;
pub mod rules;
pub mod runner;
