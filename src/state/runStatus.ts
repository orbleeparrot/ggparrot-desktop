// 실행 상태 카드의 문구 — 라벨·서버 연결 칸·저하 상태 띠. 화면과 떼어 순수 함수로 둠(테스트·재사용).
import type { RunMode, RunSlot, RunStatus } from "../types";
import { formatAgo } from "./format";

/** 상태가 이만큼 갱신되지 않으면(평소 5초마다) 멈춘 것일 수 있다고 보임. */
export const STALE_MS = 20_000;

export function labelOf(run: RunStatus, phase: RunSlot["phase"], status: RunSlot["status"]): string {
  if (run.finished)
    return run.finished.status === "error" ? `오류로 멈춤 · ${run.finished.note}` : `종료됨 · ${run.finished.note}`;
  if (phase === "stopping") return "종료 처리 중";
  if (phase === "starting") return "시작하는 중";
  return status.text;
}

/** 서버 연결 칸 — 회원 키 거절·서버 거절은 기다려도 풀리지 않을 수 있어 끊김과 구분함. */
export function serverCell(run: RunStatus, now: number): { text: string; tone: string } {
  if (run.finished) return { text: "끝남", tone: "tone-muted" };
  if (run.updatedAt === 0) return { text: "확인 중", tone: "tone-muted" };
  if (run.authFailed) return { text: "회원 키 거절됨", tone: "tone-down" };
  if (run.rejected) return { text: `서버가 거절함(HTTP ${run.rejected})`, tone: "tone-down" };
  if (!run.online) {
    // 1분이 넘으면 얼마나 끊겼는지 함께 — "다시 연결하는 중 · 3분째".
    const gone = run.offlineSince > 0 ? now - run.offlineSince : 0;
    const since = gone >= 60_000 ? ` · ${formatAgo(gone).replace(" 전", "")}째` : "";
    return { text: `다시 연결하는 중${since}`, tone: "tone-warn" };
  }
  return { text: "연결됨", tone: "tone-up" };
}

/** 확인할 곳 — 모의는 실제 계정이 없고, 테스트넷은 실전 거래소가 아님. */
const WHERE: Record<RunMode, string> = {
  mock: "모의 보유분이라 실제 계정에는 영향이 없어요",
  testnet: "바이낸스 테스트넷에서 포지션을 확인하세요",
  live: "거래소에서 포지션을 확인하세요",
};

/** 저하 상태 띠 — 손절을 못 보는 중처럼 사용자가 확인해야 하는 것만. */
export function alertOf(run: RunStatus, mode: RunMode): string | null {
  if (run.finished) return null;
  if (run.uncertain) return `주문 결과가 불확실해요 · ${WHERE[mode]}`;
  if (run.closeFailures > 0)
    return `정리 주문이 ${run.closeFailures}번 실패했어요 · 다음 확인 때 다시 시도해요. ${WHERE[mode]}`;
  if (run.priceStaleSince > 0 && run.inPosition)
    return `현재가를 받지 못해 손절 · 안전 규칙을 확인하지 못하고 있어요 · ${WHERE[mode]}`;
  if (run.authFailed) return "서버가 회원 키를 받지 않아요 · 이 매크로를 종료하고 회원 키를 확인한 뒤 다시 시작하세요";
  return null;
}
