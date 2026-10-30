//! 종료 — 종료 방법(매크로만 종료·청산 후 종료)에 따른 포지션 처리, 남길 종료 이유, 서버 종료 보고.

use super::*;

impl Bot {
    /// 종료 시 포지션 처리. 반환값은 서버/화면에 남길 note.
    pub(super) async fn finish_position(&mut self, mode: StopMode) -> Result<String, String> {
        if mode == StopMode::CloseAndStop {
            if self.position_uncertain {
                // 결과를 모르는 포지션에는 주문하지 않음 — '정리 실패'가 아니라 주문을 내지 않은 것임.
                self.logger.activity(
                    "error",
                    "결과를 모르는 포지션이라 정리 주문을 내지 않았어요",
                    "거래소에서 주문과 포지션을 직접 확인하고 정리하세요.",
                    "⚠ 포지션 불확실 — 청산 주문 생략",
                    "danger",
                );
                return Ok("결과 불확실 — 정리하지 않고 종료".into());
            }
            if self.in_position {
                self.log("청산 후 종료 요청 — 보유 포지션을 정리해요.");
                if let Ok(broker) = self.broker()
                    && let Ok(p) = broker.price().await
                {
                    self.last_price = p;
                }
                // 분명히 실패하면(요청 한도로 쉬는 중·일시 거절) 잠깐 기다렸다 다시 — 한 번 실패로 포지션을 남기고 끝내지 않게.
                // 결과를 모르는 주문이 생기면 더 주문하지 않음.
                for (attempt, wait) in [2u64, 5, 0].into_iter().enumerate() {
                    match self.close_position().await {
                        // 모의는 실제 청산이 없음 — 모의 보유분을 지금 가격으로 정리한 것으로 계산했을 뿐임.
                        Ok(true) if self.mode == RunMode::Mock => return Ok("모의 보유분 정리 후 종료".into()),
                        Ok(true) => return Ok("청산 완료 후 종료".into()),
                        Ok(false) => {}
                        Err(e) => {
                            let line = format!("  ⚠ 청산 시도 {}/3: {e}", attempt + 1);
                            self.log(with_error_code(&line, &self.order_error_code));
                        }
                    }
                    if self.position_uncertain || !self.in_position || wait == 0 {
                        break;
                    }
                    tokio::time::sleep(Duration::from_secs(wait)).await;
                }
                if !self.in_position {
                    return Ok("청산 완료 후 종료".into());
                }
                self.log("⚠ 청산 주문이 실패했어요. 거래소에서 직접 확인하세요.");
                self.logger.activity(
                    "error",
                    "정리 주문이 실패했어요",
                    self.where_to_check(),
                    "⚠ 청산 주문이 실패했어요. 거래소에서 직접 확인하세요.",
                    "danger",
                );
                return Ok("청산 실패 — 포지션 남음".into());
            }
            return Ok("포지션 없이 종료".into());
        }
        if self.in_position {
            if self.mode == RunMode::Mock {
                return Ok("모의 보유분은 정리하지 않고 종료".into());
            }
            let raw = "매크로만 종료 — 열린 포지션은 그대로 둬요. 거래소에서 직접 관리하세요.";
            self.log(raw);
            // 보유가 없을 때 누른 [종료]여도 그사이 포지션이 생겼을 수 있음 — 남겨 둔 것을 분명히 알림.
            self.logger.activity(
                "stop",
                "포지션을 그대로 두고 끝냈어요",
                "이 앱은 더 이상 손절을 보지 않아요. 거래소에서 직접 관리하세요.",
                raw,
                "warn",
            );
            return Ok("매크로만 종료 — 포지션 유지".into());
        }
        Ok("포지션 없이 종료".into())
    }

    /// 서버에 종료를 보고함 — 마지막 명령 결과(ack)를 먼저 올리고, 보내지 못한 것은 note와 활동 기록에 남김.
    pub(super) async fn report_end(&mut self, status: &str, note: &mut String) {
        if let Some(server) = self.server.clone() {
            let snap = self.snapshot();
            // 마지막 명령 결과(체결 포함)를 종료 보고 전에 올림. 종료 보고는 명령 결과를 받지 않으므로
            // 백오프 중이어도 실제로 보냄(한 번 더 시도).
            for _ in 0..2 {
                if self.pending_acks.is_empty() && !server.has_unsent_acks() {
                    break;
                }
                server.reset_backoff();
                let _ = self.heartbeat(&snap).await;
            }
            if server.has_unsent_acks() {
                note.push_str(" · 마지막 주문 결과 일부를 서버에 보내지 못했어요");
                self.logger.activity(
                "connection",
                "마지막 주문 결과를 서버에 보내지 못했어요",
                "웹의 [내 에이전트] 기록에 이 실행의 마지막 체결이 빠질 수 있어요. 거래소 체결 내역을 기준으로 보세요.",
                "종료 전 ack 전송 실패",
                "warn",
            );
            }
            if !server.stopped(status, note, Some(&snap)).await {
                note.push_str(" · 서버 종료 상태 전송 실패");
                self.logger.activity(
                    "connection",
                    "종료 결과를 서버에 알리지 못했어요",
                    "웹의 [내 에이전트]에는 한동안 실행 중으로 보일 수 있어요.",
                    "서버에 종료 결과를 전송하지 못했어요.",
                    "warn",
                );
            }
        }
    }
}
