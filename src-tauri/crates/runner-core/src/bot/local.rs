//! 실행기가 **스스로** 내리는 매수 · 매도 판단.
//!
//! 지금까지는 서버가 봉을 받아 판단하고 명령만 내려보냈다. 그 구조에서는 우리 서버가 잠깐
//! 멈추면 사용자는 자기 PC 가 멀쩡한데도 매수 · 매도 시점을 놓친다 — 원인이 전적으로 우리
//! 쪽에 있고, 전 사용자에게 동시에, 사용자는 손쓸 수도 볼 수도 없다.
//!
//! 주문은 서버 명령과 **같은 길**([`Bot::execute_command`])로 나간다. 길이 하나여야 주문
//! 상한 · 최소 수량 · 수수료 보정 · 포지션 불확실 판정이 양쪽에 똑같이 걸린다.
//!
//! 서버에는 **ack 를 보내지 않는다** — 서버가 모르는 명령이다. 대신 체결 자체를 보고한다
//! (`local_fills`). 실행기가 판단하면 그 보고가 리더보드 · 수익률 · 활동 기록의 **유일한**
//! 입구다. 서버가 명령을 내던 때는 '냈던 명령' 이라도 알았지만 이제는 아무것도 모른다.

use serde_json::{json, Value};

use crate::candles::{CandleError, CandleSource, ClosedCandleStream};
use crate::engine::candle_sim::Bar;
use crate::engine::runner::{build, command_from_fill, session_capital, BuildError, Engine};

use super::*;

/// 로컬 명령 번호 앞에 붙는 말. 서버 명령 번호(정수)와 겹치면 "이미 했다" 기억이 서로의
/// 명령을 지워 주문이 통째로 사라진다.
pub const LOCAL_ID_PREFIX: &str = "local-";

/// 지표를 세우는 데 쓰는 과거 봉 수. 서버와 같아야 같은 판정이 난다.
pub const WARMUP_CANDLES: usize = 500;

/// 한 틱에 꺼낼 체결 상한. 그리드 칸 수보다 넉넉하다 — 여기 걸린다는 건 큐가 끝없이
/// 나온다는 뜻이고, 그러면 주문이 아니라 기록이 필요한 상황이다.
const MAX_FILLS_PER_TICK: usize = 50;

/// 한 번에 되짚는 봉 수. 절전 · 네트워크 끊김으로 몇 봉을 놓쳐도 주워 온다.
const POLL_LOOKBACK: usize = 5;

/// 로컬 판단에 필요한 것들. 서버가 켜 준 세션에서만 만든다.
pub struct LocalEngine {
    engine: Engine,
    stream: ClosedCandleStream,
    /// 진입 금액 비율의 분모.
    capital: f64,
    seq: u64,
    /// 아직 서버에 못 올린 체결.
    pending_fills: Vec<Value>,
    /// 봉을 못 받은 것이 이어진 횟수 — 처음과 가끔만 알린다.
    fetch_failures: u32,
}

impl LocalEngine {
    /// 아직 서버에 못 올린 체결 수.
    pub fn unsent(&self) -> usize {
        self.pending_fills.len()
    }

    /// 지금 보유 중인가(엔진 장부 기준).
    pub fn in_position(&self) -> bool {
        self.engine.in_position()
    }
}

impl Bot {
    /// 이 세션을 실행기가 직접 판단하는가. 서버가 시작 응답으로 정한다 —
    /// 서버가 꺼 두었으면 지금까지처럼 서버 명령만 실행한다.
    pub(super) fn decides_locally(&self) -> bool {
        self.server.as_ref().map(|s| s.decides_locally()).unwrap_or(false)
    }

    /// 서버가 "네가 판단해라" 고 한 세션에서 로컬 엔진을 세운다.
    ///
    /// 여기서 실패하면 **아무도 판단하지 않는다** — 서버는 이 세션의 드라이버를 띄우지
    /// 않았다. 조용히 서버 모드로 되돌아가는 길이 없으므로 세션을 오류로 끝낸다.
    pub(super) async fn start_local_engine(&mut self) -> Result<LocalEngine, String> {
        let interval = crate::macro_info::candle_interval(&self.macro_);
        let engine = build(&self.macro_).map_err(|e| match e {
            BuildError::NotPorted(_) | BuildError::BadMacro(_) => e.to_string(),
        })?;
        let source = CandleSource::new(
            &self.exchange_name,
            &self.symbol,
            &interval,
            self.market.as_str(),
            self.http.clone(),
            None,
        )
        .map_err(|e| e.message().to_string())?;
        let mut stream = ClosedCandleStream::new(source, None);
        let history = stream
            .warmup(WARMUP_CANDLES, now_ms())
            .await
            .map_err(|e| format!("봉을 받지 못해 매크로를 시작할 수 없어요: {}", e.message()))?;

        let mut local = LocalEngine {
            engine,
            stream,
            capital: session_capital(&self.macro_),
            seq: 0,
            pending_fills: Vec::new(),
            fetch_failures: 0,
        };
        for candle in &history {
            local.engine.on_bar(&Bar::from(*candle));
        }
        // 지표는 두고 장부만 되돌린다 — 웜업 중의 가짜 매수가 실거래 포지션으로 둔갑하지 않게.
        local.engine.warmup_done();

        let last = history.last().map(|c| c.t).unwrap_or(0);
        self.logger.log_full(
            &format!(
                "로컬 판단으로 시작해요 — {interval} 봉 {}개로 지표를 세웠어요. \
                 서버가 멈춰도 매매는 이 PC 에서 이어집니다.",
                history.len()
            ),
            Some("info"),
            "INFO",
            "runner.local_engine_started",
            "",
            &obj(json!({
                "interval": interval,
                "warmup": history.len(),
                "last_candle_ms": last,
                "engine_version": crate::engine::ENGINE_VERSION,
            })),
        );
        Ok(local)
    }

    /// 로컬 판단 한 바퀴 — 마감봉을 받아 판단하고, 나온 주문을 서버 명령과 같은 길로 낸다.
    ///
    /// `Err` 는 '포지션이 불확실해져 루프를 끝내야 함' 이라는 뜻이다(서버 명령과 같은 규칙).
    pub(super) async fn local_round(
        &mut self,
        local: &mut LocalEngine,
        price: f64,
        guard: &mut RiskGuard,
    ) -> Result<(), String> {
        // 틱 기반 규칙(A · B · C)은 봉이 아니라 시계를 본다.
        local.engine.set_clock(now_ms() as i64);

        match local.stream.poll(POLL_LOOKBACK, now_ms()).await {
            Ok(fresh) => {
                if local.fetch_failures > 0 {
                    local.fetch_failures = 0;
                    self.logger.log("봉 수집이 복구됐어요.");
                }
                for candle in fresh {
                    local.engine.on_bar(&Bar::from(candle));
                }
            }
            Err(e) => {
                // 봉을 못 받은 것은 치명적이지 않다 — 다음 바퀴에 다시 받는다. 그동안에도
                // 손절 · 보유시간 같은 안전망은 이미 루프가 따로 보고 있다.
                self.note_candle_failure(local, &e);
            }
        }

        for _ in 0..MAX_FILLS_PER_TICK {
            let Some(fill) = local.engine.tick(price) else { break };
            local.seq += 1;
            let cmd = command_from_fill(&fill, local.capital);
            let id = format!("{LOCAL_ID_PREFIX}{}", local.seq);
            let command = json!({
                "id": id,
                "seq": local.seq,
                "source": "local",
                "action": cmd.action,
                "notional_frac": cmd.notional_frac,
                "qty_frac": cmd.qty_frac,
                "signal_price": cmd.signal_price,
                "reason": cmd.reason,
            });
            self.run_local_command(local, &command, price, guard).await?;
        }
        Ok(())
    }

    fn note_candle_failure(&mut self, local: &mut LocalEngine, error: &CandleError) {
        local.fetch_failures += 1;
        // 장애가 길어져도 활동 기록을 채우지 않게 — 처음과 그 뒤 가끔만.
        if local.fetch_failures == 1 || local.fetch_failures % 60 == 0 {
            self.logger.log_full(
                &format!("  일시 오류(봉): {} — 다음 틱에 다시 받아요", error.message()),
                Some("warn"),
                "WARN",
                "runner.candle_fetch_failed",
                "",
                &obj(json!({"error": error.message(), "streak": local.fetch_failures})),
            );
        }
    }

    /// 로컬 명령 하나를 실행하고 그 결과를 **체결 보고**로 쌓는다.
    async fn run_local_command(
        &mut self,
        local: &mut LocalEngine,
        command: &Value,
        price: f64,
        guard: &mut RiskGuard,
    ) -> Result<(), String> {
        let action = value_text(command.get("action")).to_lowercase();
        let is_entry = action == "buy" || action == "short";

        // 진입 전 확인 — 서버 명령과 같은 관문을 거친다. 청산은 막지 않는다.
        let (blocked, why) = if self.close_failures > 0 {
            (true, "정리하지 못한 포지션이 있어 새 진입을 미뤄요".to_string())
        } else if self.get_command().is_some() {
            (true, "종료를 요청해서 새로 진입하지 않아요".to_string())
        } else {
            guard.entry_blocked()
        };
        if blocked && is_entry {
            self.logger.activity(
                "blocked",
                "진입을 미뤘어요",
                why.clone(),
                format!("  ⏸ 진입 보류: {why}"),
                "warn",
            );
            return Ok(());
        }

        let (net_before, was_flat) = (self.realized_net, !self.in_position);
        let ack = match self.execute_command(command, price).await {
            Ok(ack) => ack,
            Err(e) => {
                // 포지션이 불확실해졌다 — 실행을 끝내기 전에 이 체결도 보고에 남긴다.
                local.pending_fills.push(local_fill(command, &json!({}), false, &e));
                return Err(e);
            }
        };
        let executed = num(ack.get("executed_qty"));
        if executed > 0.0 {
            if is_entry {
                if was_flat && self.in_position {
                    guard.on_entry();
                }
            } else if !self.in_position {
                guard.on_exit(self.realized_net - net_before, false);
            } else {
                guard.on_partial_exit(self.realized_net - net_before);
            }
        }
        let ok = ack.get("ok").and_then(Value::as_bool).unwrap_or(false) && executed > 0.0;
        local.pending_fills.push(local_fill(command, &ack, ok, value_text(ack.get("error")).as_str()));
        Ok(())
    }

    /// 로컬 판단 세션이 하트비트에 덧붙이는 것 — 서버는 이걸로 장부를 적고 화면을 비춘다.
    ///
    /// 체결은 **보낸 뒤에** 비운다([`Bot::clear_sent_fills`]). 전송이 실패해 다시 보내면
    /// 서버가 같은 체결을 두 번 적을 수 있지만, 빠뜨리는 쪽이 훨씬 나쁘다 — 빠진 체결은
    /// 리더보드 · 수익률에서 영영 사라진다.
    pub(super) fn local_snapshot_extra(&self, local: &LocalEngine) -> Value {
        let mut extra = serde_json::Map::new();
        if !local.pending_fills.is_empty() {
            extra.insert("local_fills".into(), Value::Array(local.pending_fills.clone()));
        }
        extra.insert("strategy_state".into(), self.local_strategy_state(local));
        Value::Object(extra)
    }

    /// 서버가 화면에 비출 전략 상태 사본.
    fn local_strategy_state(&self, local: &LocalEngine) -> Value {
        json!({
            "engine_version": crate::engine::ENGINE_VERSION,
            "checkpoint_ms": now_ms(),
            "in_position": local.engine.in_position(),
            "equity": local.engine.equity(self.last_price),
            "last_candle_ms": local.stream.last_t,
        })
    }

    /// 서버가 받았다 — 보낸 만큼만 비운다(그 사이 새로 난 체결은 남긴다).
    pub(super) fn clear_sent_fills(&self, local: &mut LocalEngine, sent: usize) {
        let sent = sent.min(local.pending_fills.len());
        local.pending_fills.drain(..sent);
    }
}

/// 체결 보고 한 줄. 서버 `record_local_fills` 가 읽는 모양이다.
fn local_fill(command: &Value, ack: &Value, ok: bool, error: &str) -> Value {
    json!({
        "action": value_text(command.get("action")).to_lowercase(),
        "notional_frac": num(command.get("notional_frac")),
        "qty_frac": num(command.get("qty_frac")),
        "signal_price": num(command.get("signal_price")),
        "reason": value_text(command.get("reason")),
        "ok": ok,
        "executed_qty": num(ack.get("executed_qty")),
        "fill_price": num(ack.get("fill_price")),
        "error": error.chars().take(200).collect::<String>(),
    })
}
