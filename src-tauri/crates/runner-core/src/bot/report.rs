//! 서버 보고·연결 상태 — 하트비트, 연결·인증·거절 알림, 화면으로 보내는 RunHealth, 원격 종료, 틱 공백(잠자기).

use super::*;

impl Bot {
    pub(super) async fn heartbeat(&mut self, snapshot: &Value) -> crate::server::HeartbeatReply {
        let acks = std::mem::take(&mut self.pending_acks);
        match &self.server {
            Some(s) => s.heartbeat(snapshot, acks).await,
            None => crate::server::HeartbeatReply::idle(),
        }
    }

    /// 상태를 화면·서버에 올리고 서버의 답을 받음(연결·인증 상태 알림 포함).
    pub(super) async fn report_status(&mut self) -> crate::server::HeartbeatReply {
        let snap = self.snapshot();
        self.report_snapshot(snap).await
    }

    /// [`report_status`]와 같되 스냅샷을 받음 — 시세를 못 받은 틱도 같은 연결 처리를 거침.
    pub(super) async fn report_snapshot(&mut self, snap: Value) -> crate::server::HeartbeatReply {
        let _ = self.tx.send(BotEvent::Status(self.ui_snapshot(snap.clone())));
        let reply = self.heartbeat(&snap).await;
        self.apply_connection(&reply);
        reply
    }

    /// 하트비트 결과로 연결·인증·거절 상태를 알리고 화면에 [`RunHealth`]를 보냄.
    pub(super) fn apply_connection(&mut self, reply: &crate::server::HeartbeatReply) {
        if reply.auth_failed && !self.auth_noticed {
            self.auth_noticed = true;
            self.logger.activity(
                "connection",
                "서버가 회원 키를 받지 않아요",
                "실행 중인 매크로는 시작할 때의 회원 키를 계속 써요. 이 매크로를 종료하고 설정 › 껄무새 계정에서 회원 키를 확인한 뒤 다시 시작하세요. 그동안 새 진입은 멈추고 손절만 지켜요.",
                "heartbeat 401/403 — 회원 키 확인 필요",
                "danger",
            );
        } else if !reply.auth_failed && !reply.offline {
            self.auth_noticed = false;
        }
        if reply.rejected != 0 && self.rejected_noticed != reply.rejected {
            self.rejected_noticed = reply.rejected;
            self.logger.activity(
                "connection",
                format!("서버가 상태 보고를 받지 않아요(HTTP {})", reply.rejected),
                "기다려도 풀리지 않을 수 있어요. 그동안 새 진입은 멈추고 손절만 지켜요. 계속되면 이 매크로를 종료하고 다시 시작하세요.",
                format!("heartbeat 거절 HTTP {}", reply.rejected),
                "danger",
            );
        } else if !reply.offline {
            self.rejected_noticed = 0;
        }
        if reply.offline {
            if self.offline_since == 0 {
                self.offline_since = wall_ms();
            }
            if !self.offline_logged {
                if !reply.auth_failed && reply.rejected == 0 {
                    self.logger.activity(
                        "connection",
                        "서버 연결을 다시 시도하고 있어요",
                        "그동안 새 진입은 멈추고 손절만 지켜요",
                        "서버 연결 재시도 중 — 신호 대기(진입 없음, 손절만 로컬에서 봅니다)",
                        "warn",
                    );
                }
                self.logger.log_full(
                    "서버 연결 재시도 중 — 신호 대기(진입 없음, 손절만 로컬에서 봅니다)",
                    Some("warn"),
                    "WARN",
                    "runner.connection_lost",
                    "",
                    &Map::new(),
                );
                self.offline_logged = true;
            }
            if !self.offline_escalated && wall_ms().saturating_sub(self.offline_since) >= OFFLINE_ALERT_MS {
                self.offline_escalated = true;
                self.logger.activity(
                    "connection",
                    "서버와 5분 넘게 연결되지 않았어요",
                    "웹의 종료 · 정리 신호도 이 앱에 오지 않아요. 멈추려면 이 앱에서 종료하세요. 손절은 계속 지켜요.",
                    "",
                    "danger",
                );
            }
        } else {
            if self.offline_logged {
                self.logger.activity("connection", "서버 연결이 돌아왔어요", "", "", "ok");
                self.logger.telemetry(
                    "info",
                    "서버 상태 보고(하트비트) 응답이 다시 와요.",
                    "runner.connection_restored",
                    "INFO",
                    "",
                    &Map::new(),
                );
            }
            self.offline_logged = false;
            self.offline_since = 0;
            self.offline_escalated = false;
        }
        let _ = self.tx.send(BotEvent::Health(self.health(reply)));
    }

    pub(super) fn health(&self, reply: &crate::server::HeartbeatReply) -> RunHealth {
        RunHealth {
            online: !reply.offline,
            auth_failed: reply.auth_failed,
            rejected: reply.rejected,
            offline_since: self.offline_since,
            price_stale_since: self.price_fail_since,
            close_failures: self.close_failures,
        }
    }

    /// 원격 종료 걸기. 서버가 이유를 주면(세션이 없어짐·이미 끝남) 웹에서 누른 종료와 구분해 알림.
    pub(super) fn note_remote_stop(&mut self, reply: &crate::server::HeartbeatReply, m: StopMode) {
        let raw = format!("원격 종료 명령 수신: {}", reply.action);
        self.log(&raw);
        if reply.reason.is_empty() {
            self.logger.activity("stop", "웹에서 종료를 요청했어요", m.label(), raw, "muted");
        } else {
            let tail = if self.in_position && self.mode != RunMode::Mock {
                format!(" · 포지션은 그대로예요. {}", self.where_to_check())
            } else {
                String::new()
            };
            self.logger.activity("stop", "서버가 이 실행을 끝냈어요", format!("{}{tail}", reply.reason), raw, "warn");
        }
        self.set_command(m);
    }

    /// 틱 사이가 크게 벌어졌으면(잠자기·앱 멈춤) 알리고 서버 재시도를 바로 하게 함.
    pub(super) fn check_wall_gap(&mut self) {
        let now = wall_ms();
        let gap = now.saturating_sub(self.last_tick_ms);
        if self.last_tick_ms > 0 && gap >= WALL_GAP_ALERT_MS {
            let minutes = (gap as f64 / 60_000.0).round().max(1.0);
            self.logger.activity(
                "connection",
                format!("기기가 {minutes:.0}분 동안 멈춰 있었어요"),
                "잠자기 등으로 그동안 손절과 서버 신호를 확인하지 못했어요. 지금 다시 확인해요.",
                format!("틱 간격 {:.0}초", gap as f64 / 1000.0),
                "warn",
            );
            if let Some(s) = &self.server {
                s.reset_backoff();
            }
        }
        self.last_tick_ms = now;
    }
}
