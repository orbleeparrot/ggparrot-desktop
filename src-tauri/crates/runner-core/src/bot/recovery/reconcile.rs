//! 복구 전 보유·주문·선물 평단 대조. 읽기 전용 조회만 허용함.

use super::*;

pub(super) async fn reconcile_position(
    bot: &mut Bot,
    checkpoint: &Checkpoint,
    broker: &dyn Broker,
) -> Result<(), String> {
    if !broker.load_market().await.map_err(|e| safe_preflight_reason(&e.to_string(), &bot.credentials))? {
        return Err("이 종목의 현재 주문 규격을 찾지 못했어요. 거래소에서 종목을 확인하세요.".into());
    }
    let rules = broker.order_rules().await.map_err(|e| safe_preflight_reason(&e.to_string(), &bot.credentials))?;
    if !rules.step.is_finite() || rules.step < 0.0 {
        return Err("현재 주문 수량 단위를 확인하지 못했어요.".into());
    }
    bot.step = rules.step;
    let position = broker
        .recovery_position_details()
        .await
        .map_err(|e| safe_preflight_reason(&e.to_string(), &bot.credentials))?;
    let baseline = checkpoint
        .runtime
        .get("account_baseline")
        .and_then(Value::as_f64)
        .ok_or("보유 대조 기준이 없어 이어서 실행할 수 없어요. 거래소에서 포지션을 확인하세요.")?;
    let held = bot.held_qty + bot.position_dust_qty;
    let signed = if bot.market == Market::Futures && bot.side == Side::Short { -held } else { held };
    let expected = baseline + signed;
    let tolerance = (bot.step * 0.0001).max(expected.abs() * 1e-8).max(1e-12);
    if !position.qty.is_finite() || !expected.is_finite() || (position.qty - expected).abs() > tolerance {
        return Err(
            "거래소 보유가 저장된 실행 상태와 달라요. 주문을 보내지 않았어요. 거래소에서 주문과 보유를 확인하세요."
                .into(),
        );
    }
    if bot.market == Market::Futures {
        if baseline.abs() > 1e-12 {
            return Err("실행 전부터 있던 선물 보유와 매크로 보유를 구분할 수 없어 이어서 실행하지 않았어요. 거래소에서 포지션을 확인하세요.".into());
        }
        if position.leverage != Some(bot.leverage) {
            return Err(
                "거래소 레버리지가 저장된 매크로와 달라요. 주문을 보내지 않았어요. 거래소에서 포지션을 확인하세요."
                    .into(),
            );
        }
        if bot.in_position {
            let actual_entry = position.entry_price.filter(|p| *p > 0.0).ok_or("선물 평단을 확인하지 못했어요.")?;
            let tolerance = actual_entry.abs().max(bot.entry_price.abs()).max(1.0) * 1e-7;
            if (actual_entry - bot.entry_price).abs() > tolerance {
                return Err("선물 보유 수량은 같지만 평단이 저장된 실행과 달라요. 주문을 보내지 않았어요. 거래소에서 포지션을 확인하세요.".into());
            }
        }
    }
    Ok(())
}
