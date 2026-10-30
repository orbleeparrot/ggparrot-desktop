// 목록 묶음 — 확인 필요·실행 중·대기. 묶음 안 순서는 저장된 순서(Rust move_run) 그대로.
// - 확인 필요: Rust needs_attention(머리줄 '확인 필요 N개'와 같은 기준) — 실행 중이어도 여기로 올라옴
// - 실행 중: 확인·시작·실행·종료 중
// - 대기: 대기 중·끝남(확인 필요가 아닌 것)
import type { RunSlot } from "../types";
import { isIdle } from "./runs";

export type GroupKey = "attention" | "running" | "idle";

export const GROUP_LABELS: Record<GroupKey, string> = { attention: "확인 필요", running: "실행 중", idle: "대기" };

export const GROUP_ORDER: GroupKey[] = ["attention", "running", "idle"];

export function groupOf(slot: Pick<RunSlot, "attention" | "phase">): GroupKey {
  if (slot.attention) return "attention";
  return isIdle(slot.phase) ? "idle" : "running";
}

/** 찾기 칸 — 종목·매크로 이름·전략·거래소 이름에 낱말이 다 들어 있으면 보임(대소문자 무시). */
export function matches(slot: Pick<RunSlot, "macro">, query: string): boolean {
  const words = query.trim().toLowerCase().split(/\s+/).filter(Boolean);
  if (!words.length) return true;
  const m = slot.macro;
  const hay = [m.symbol, m.title, m.strategy, m.exchangeLabel, m.exchangeName].join(" ").toLowerCase();
  return words.every((w) => hay.includes(w));
}

/** 묶음별로 나눈 목록(빈 묶음은 뺌). */
export function groupRuns<T extends Pick<RunSlot, "attention" | "phase">>(runs: T[]): { key: GroupKey; items: T[] }[] {
  return GROUP_ORDER.map((key) => ({ key, items: runs.filter((r) => groupOf(r) === key) })).filter(
    (g) => g.items.length > 0,
  );
}

/**
 * 묶음 안에서 옮긴 뒤의 전체 번호 — move_run(runId, index)의 index(자기를 뺀 목록에서 놓일 자리).
 * - groupIds: 그 묶음의 지금 순서, to: 묶음 안에서 놓일 자리(자기를 뺀 목록 기준).
 * - 다른 묶음 사이의 순서는 건드리지 않음 — 같은 묶음 이웃의 앞(또는 마지막 이웃의 뒤)에 놓음.
 */
export function globalIndex(allIds: string[], groupIds: string[], id: string, to: number): number {
  const others = groupIds.filter((x) => x !== id);
  const rest = allIds.filter((x) => x !== id);
  if (others.length === 0) return allIds.indexOf(id);
  if (to < others.length) return rest.indexOf(others[to]);
  return rest.indexOf(others[others.length - 1]) + 1;
}
