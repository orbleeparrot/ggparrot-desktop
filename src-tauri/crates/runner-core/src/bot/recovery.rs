//! 중단 실행의 체크포인트·대조·재개. 주문 의도가 남으면 새 주문 없이 보존함.

use super::*;
use crate::recovery::{Checkpoint, RecoveryContext};

#[derive(serde::Serialize, serde::Deserialize)]
struct BotState {
    step: f64,
    in_position: bool,
    entry_price: f64,
    held_qty: f64,
    realized: f64,
    realized_net: f64,
    open_entry_fee: f64,
    open_fee_known: bool,
    observed_fee_rate: Option<f64>,
    fees_quote: f64,
    fees_complete: bool,
    position_uncertain: bool,
    position_dust_qty: f64,
    capital: f64,
    done_command_ids: VecDeque<Value>,
    pending_acks: Vec<Value>,
    close_failures: u32,
    funds_short: bool,
    close_retry_at: u64,
    last_price: f64,
    last_fill_qty: f64,
    last_fill_price: f64,
    settings_digest: String,
}

impl BotState {
    fn capture(bot: &Bot) -> Self {
        Self {
            step: bot.step,
            in_position: bot.in_position,
            entry_price: bot.entry_price,
            held_qty: bot.held_qty,
            realized: bot.realized,
            realized_net: bot.realized_net,
            open_entry_fee: bot.open_entry_fee,
            open_fee_known: bot.open_fee_known,
            observed_fee_rate: bot.observed_fee_rate,
            fees_quote: bot.fees_quote,
            fees_complete: bot.fees_complete,
            position_uncertain: bot.position_uncertain,
            position_dust_qty: bot.position_dust_qty,
            capital: bot.capital,
            done_command_ids: bot.done_command_ids.clone(),
            pending_acks: bot.pending_acks.clone(),
            close_failures: bot.close_failures,
            funds_short: bot.funds_short,
            close_retry_at: bot.close_retry_at,
            last_price: bot.last_price,
            last_fill_qty: bot.last_fill_qty,
            last_fill_price: bot.last_fill_price,
            settings_digest: bot.settings_digest.clone(),
        }
    }
    fn apply(self, bot: &mut Bot) {
        bot.step = self.step;
        bot.in_position = self.in_position;
        bot.entry_price = self.entry_price;
        bot.held_qty = self.held_qty;
        bot.realized = self.realized;
        bot.realized_net = self.realized_net;
        bot.open_entry_fee = self.open_entry_fee;
        bot.open_fee_known = self.open_fee_known;
        bot.observed_fee_rate = self.observed_fee_rate;
        bot.fees_quote = self.fees_quote;
        bot.fees_complete = self.fees_complete;
        bot.position_uncertain = self.position_uncertain;
        bot.position_dust_qty = self.position_dust_qty;
        bot.capital = self.capital;
        bot.done_command_ids = self.done_command_ids;
        bot.pending_acks = self.pending_acks;
        bot.close_failures = self.close_failures;
        bot.funds_short = self.funds_short;
        bot.close_retry_at = self.close_retry_at;
        bot.last_price = self.last_price;
        bot.last_fill_qty = self.last_fill_qty;
        bot.last_fill_price = self.last_fill_price;
        bot.settings_digest = self.settings_digest;
    }
}

pub(super) struct RecoveryRuntime {
    context: RecoveryContext,
    pub(super) checkpoint: Checkpoint,
}

pub struct PreparedResume {
    bot: Bot,
}

impl PreparedResume {
    pub fn checkpoint(&self) -> &Checkpoint {
        &self.bot.recovery_state.as_ref().expect("복구 준비가 끝난 실행").checkpoint
    }

    pub fn spawn(mut self, tx: mpsc::UnboundedSender<BotEvent>) -> BotHandle {
        if let Some(server) = &self.bot.server {
            let weak = tx.downgrade();
            let credentials = self.bot.credentials.clone();
            server.set_event_warning(Arc::new(move |message| {
                let Some(tx) = weak.upgrade() else { return };
                let message = crate::connection::mask_secrets(&message, &credentials);
                let _ = tx.send(BotEvent::Log(message.clone()));
                let _ = tx.send(BotEvent::Activity(ActivityEvent {
                    kind: "connection",
                    title: "실행 기록 보관에 문제가 생겼어요".into(),
                    detail: message.clone(),
                    raw: message,
                    tone: "warn",
                }));
            }));
        }
        self.bot.tx = tx.clone();
        self.bot.logger = Logger::new(tx, self.bot.server.clone(), self.bot.credentials.clone());
        launch_bot(self.bot)
    }
}

fn launch_bot(bot: Bot) -> BotHandle {
    let command = bot.command.clone();
    let wake = bot.wake.clone();
    let (quote, symbol, side) = (bot.quote, bot.symbol.clone(), bot.side);
    let join = tokio::spawn(bot.run());
    BotHandle { command, wake, quote, symbol, side, join }
}

// 기존 시작 인자를 유지하며 복구 보관함만 덧붙이는 호환 진입점임.
#[allow(clippy::too_many_arguments)]
pub fn spawn_bot_recoverable(
    macro_: Value,
    credentials: Credentials,
    mode: RunMode,
    server: ServerClient,
    preflight: Option<PreflightResult>,
    http: reqwest::Client,
    tx: mpsc::UnboundedSender<BotEvent>,
    context: RecoveryContext,
) -> Result<BotHandle, String> {
    Ok(prepare_bot_recoverable(macro_, credentials, mode, server, preflight, http, context)?.spawn(tx))
}

pub fn prepare_bot_recoverable(
    macro_: Value,
    credentials: Credentials,
    mode: RunMode,
    server: ServerClient,
    preflight: Option<PreflightResult>,
    http: reqwest::Client,
    context: RecoveryContext,
) -> Result<PreparedResume, String> {
    if uuid::Uuid::parse_str(&context.recovery_id).is_err() {
        return Err("실행 복구 번호를 만들지 못했어요.".into());
    }
    let session_id = server.session_id().ok_or("아직 서버 세션이 없어요.")?;
    let checkpoint = Checkpoint {
        recovery_id: context.recovery_id.clone(),
        session_id,
        generation: server.generation(),
        macro_: macro_.clone(),
        macro_digest: server.macro_digest(),
        mode,
        server_base: server.base().into(),
        completed: false,
        runtime: json!({}),
        engine_version: crate::engine::ENGINE_VERSION,
        decides_locally: server.decides_locally(),
    };
    let (tx, _rx) = mpsc::unbounded_channel();
    let mut bot = Bot::new(macro_, credentials, mode, Some(server), tx, http);
    bot.preflight = preflight;
    bot.recovery_state = Some(RecoveryRuntime { context, checkpoint });
    bot.persist_recovery_state()?;
    Ok(PreparedResume { bot })
}

pub async fn prepare_resume(
    mut checkpoint: Checkpoint,
    credentials: Credentials,
    http: reqwest::Client,
    server: ServerClient,
    context: RecoveryContext,
) -> Result<PreparedResume, String> {
    checkpoint.validate()?;
    if context.recovery_id != checkpoint.recovery_id || server.base() != checkpoint.server_base {
        return Err("저장된 실행의 연결 정보가 맞지 않아요. 원래 실행한 PC에서 다시 연결하세요.".into());
    }
    let authoritative = server.recovery_preview(checkpoint.session_id, &checkpoint.recovery_id).await?;
    let pending_resume = checkpoint.runtime.get("resume_attempt").cloned();
    if authoritative.get("recoverable").and_then(Value::as_bool) != Some(true) && pending_resume.is_none() {
        return Err(authoritative
            .get("reason")
            .and_then(Value::as_str)
            .filter(|s| !s.is_empty())
            .unwrap_or("이 실행을 이어받을 수 없어요. 웹에서 실행 상태를 확인하세요.")
            .into());
    }
    let remote_generation = authoritative.get("generation").and_then(Value::as_u64);
    let generation_matches = remote_generation == Some(checkpoint.generation)
        || (pending_resume.is_some() && remote_generation == checkpoint.generation.checked_add(1));
    if !generation_matches
        || authoritative.get("macro_digest").and_then(Value::as_str) != Some(checkpoint.macro_digest.as_str())
        || authoritative.get("macro") != Some(&checkpoint.macro_)
        || authoritative.get("mode").and_then(Value::as_str) != Some(checkpoint.mode.as_str())
        || authoritative.get("decides").and_then(Value::as_str)
            != Some(if checkpoint.decides_locally { "local" } else { "server" })
    {
        return Err(
            "웹의 실행 정보와 PC에 저장된 상태가 달라요. 새 주문은 보내지 않았어요. 실행 기록을 확인하세요.".into()
        );
    }
    let (tx, _rx) = mpsc::unbounded_channel();
    let mut bot = Bot::new(checkpoint.macro_.clone(), credentials, checkpoint.mode, Some(server.clone()), tx, http);
    let state: BotState =
        serde_json::from_value(checkpoint.runtime.get("bot").cloned().ok_or("저장된 포지션 상태가 없어요.")?)
            .map_err(|_| "저장된 포지션 상태를 읽지 못했어요.".to_string())?;
    state.apply(&mut bot);
    if !bot.connect() {
        return Err("거래소 연결을 준비하지 못했어요. 거래소 키를 확인하세요.".into());
    }
    let broker = bot.broker()?;
    if let Some(pending) = checkpoint.runtime.get("pending_orders").and_then(Value::as_array).filter(|p| !p.is_empty())
    {
        for intent in pending {
            let client_id = intent.get("client_id").and_then(Value::as_str).ok_or("저장된 주문 번호가 없어요.")?;
            let closing = intent.get("closing").and_then(Value::as_bool).unwrap_or(false);
            if bot.mode != RunMode::Mock {
                let _result = broker.recovery_order(client_id, closing).await;
            }
        }
        return Err("주문 처리 도중 실행이 끊겨 매크로 상태를 확정할 수 없어요. 주문을 다시 보내지 않았어요. 거래소에서 주문·보유를 확인하세요.".into());
    }
    if bot.position_uncertain {
        return Err("주문 결과가 불확실해요. 거래소에서 주문과 포지션을 확인하세요.".into());
    }
    let risk = checkpoint.runtime.get("risk").ok_or("저장된 안전 설정 상태가 없어요.")?;
    bot.risk_guard().restore_checkpoint(risk)?;
    if checkpoint.decides_locally {
        let mut local =
            bot.restore_local_engine(checkpoint.runtime.get("local").ok_or("저장된 매크로 판단 상태가 없어요.")?)?;
        local.refresh_after_resume(&checkpoint.macro_).await?;
        checkpoint.runtime["local"] = local.checkpoint()?;
    }
    if bot.mode != RunMode::Mock {
        reconcile::reconcile_position(&mut bot, &checkpoint, broker.as_ref()).await?;
    }

    let mut payload = json!({"session_id":checkpoint.session_id,"recovery_id":checkpoint.recovery_id,
        "generation":checkpoint.generation,"runner_version":crate::SERVER_LINK_LEVEL,
        "engine_version":crate::engine::ENGINE_VERSION.to_string(),"macro_digest":checkpoint.macro_digest,
        "snapshot":bot.snapshot()});
    if checkpoint.decides_locally {
        payload["strategy_state"] = json!({"engine_version":crate::engine::ENGINE_VERSION,
            "checkpoint_ms":now_ms(),"in_position":bot.in_position,"last_candle_ms":checkpoint.runtime.pointer("/local/last_t").cloned().unwrap_or(Value::Null)});
    } else if let Some(strategy_state) = authoritative.get("strategy_state") {
        payload["strategy_state"] = strategy_state.clone();
    }
    if let Some(saved) = pending_resume {
        payload = saved;
    } else {
        payload["resume_id"] = json!(uuid::Uuid::new_v4().to_string());
        checkpoint.runtime["resume_attempt"] = payload.clone();
        context.store.save(&checkpoint)?;
    }
    let resumed = server.resume_session(&payload).await?;
    let generation = resumed
        .get("generation")
        .and_then(Value::as_u64)
        .filter(|g| *g == checkpoint.generation + 1)
        .ok_or("복구된 실행의 소유권을 확인하지 못했어요. 새 주문은 보내지 않았어요.")?;
    let mut checkpoint = checkpoint;
    checkpoint.generation = generation;
    if let Some(runtime) = checkpoint.runtime.as_object_mut() {
        runtime.remove("resume_attempt");
    }
    context.store.save(&checkpoint)?;
    bot.pending_acks.clear();
    bot.restoring = true;
    bot.recovery_state = Some(RecoveryRuntime { context, checkpoint });
    Ok(PreparedResume { bot })
}

impl Bot {
    pub(super) fn persist_recovery_state(&mut self) -> Result<(), String> {
        if self.recovery_failed {
            return Err(
                "실행 복구 상태를 저장하지 못해 주문을 멈췄어요. PC의 저장 공간과 파일 접근 권한을 확인하세요.".into(),
            );
        }
        let bot =
            serde_json::to_value(BotState::capture(self)).map_err(|_| "실행 상태를 저장하지 못했어요.".to_string())?;
        let snapshot = self.snapshot();
        let Some(recovery) = self.recovery_state.as_mut() else { return Ok(()) };
        recovery.checkpoint.macro_ = self.macro_.clone();
        if !self.settings_digest.is_empty() {
            recovery.checkpoint.macro_digest = self.settings_digest.clone();
        }
        recovery.checkpoint.runtime["bot"] = bot;
        recovery.checkpoint.runtime["snapshot"] = snapshot;
        let result = recovery.context.store.save(&recovery.checkpoint);
        if result.is_err() {
            self.recovery_failed = true;
        }
        result
    }

    pub(super) async fn initialize_recovery(
        &mut self,
        guard: &RiskGuard,
        local: Option<&local::LocalEngine>,
    ) -> Result<(), String> {
        if self.recovery_state.is_none() {
            return Ok(());
        }
        if !self.restoring {
            let baseline = if self.mode == RunMode::Mock {
                0.0
            } else {
                self.broker()?.recovery_position().await.map_err(|e| e.to_string())?
            };
            if let Some(recovery) = self.recovery_state.as_mut() {
                recovery.checkpoint.runtime["account_baseline"] = json!(baseline);
            }
        }
        self.sync_recovery(guard, local, false)
    }

    pub(super) fn sync_recovery(
        &mut self,
        guard: &RiskGuard,
        local: Option<&local::LocalEngine>,
        clean: bool,
    ) -> Result<(), String> {
        let local = local.map(|e| e.checkpoint()).transpose()?;
        let Some(recovery) = self.recovery_state.as_mut() else { return Ok(()) };
        recovery.checkpoint.runtime["risk"] = guard.checkpoint();
        recovery.checkpoint.runtime["local"] = local.unwrap_or(Value::Null);
        if clean {
            recovery.checkpoint.runtime["pending_orders"] = json!([]);
        }
        self.persist_recovery_state()
    }

    pub(super) fn record_order_intent(&mut self, client_id: &str, closing: bool) -> Result<(), String> {
        let Some(recovery) = self.recovery_state.as_mut() else { return Ok(()) };
        if recovery.checkpoint.runtime.get("pending_orders").is_none() {
            recovery.checkpoint.runtime["pending_orders"] = json!([]);
        }
        recovery.checkpoint.runtime["pending_orders"]
            .as_array_mut()
            .ok_or("실행 복구 상태가 손상됐어요.")?
            .push(json!({"client_id":client_id,"closing":closing}));
        self.persist_recovery_state()
    }

    pub(super) fn complete_recovery(&mut self) -> Result<(), String> {
        let Some(recovery) = self.recovery_state.as_mut() else { return Ok(()) };
        recovery.checkpoint.completed = true;
        self.persist_recovery_state()
    }
}

#[cfg(test)]
mod tests;

mod reconcile;
