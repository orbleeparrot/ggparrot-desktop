// 활동 기록 다루기 — 슬롯별로 id 순서를 지키며 합치기·최근 1,000개 유지·필터·개발자 로그 보기.
import type { Activity, ActivityKind } from "../types";

export const MAX_ACTIVITY = 1000;

/** 이미 있는 id는 버리고, id 순서로 정렬해 최근 MAX_ACTIVITY개만 남김. 바뀐 게 없으면 같은 배열을 돌려줌. */
export function mergeActivity(prev: Activity[], incoming: Activity[]): Activity[] {
  return mergeById(prev, incoming, MAX_ACTIVITY);
}

/** id(Rust 일련번호)로 합치기 — 중복은 버리고 id 오름차순·최근 max개 유지. 바뀐 게 없으면 같은 배열. 앱 기록도 같이 씀. */
export function mergeById<T extends { id: number }>(prev: T[], incoming: T[], max: number): T[] {
  if (incoming.length === 0) return prev;
  const seen = new Set(prev.map((a) => a.id));
  const fresh = incoming.filter((a) => !seen.has(a.id) && (seen.add(a.id), true));
  if (fresh.length === 0) return prev;
  const last = prev.length ? prev[prev.length - 1].id : -Infinity;
  const inOrder = fresh.every((a, i) => a.id > (i === 0 ? last : fresh[i - 1].id));
  const next = inOrder ? prev.concat(fresh) : prev.concat(fresh).sort((a, b) => a.id - b.id);
  return next.length > max ? next.slice(next.length - max) : next;
}

export type ActivityFilter = "all" | "trade" | "signal" | "problem";

const FILTER_KINDS: Record<Exclude<ActivityFilter, "all">, ActivityKind[]> = {
  trade: ["buy", "sell", "stop_loss"],
  signal: ["signal", "blocked"],
  problem: ["error", "connection"],
};

export const ACTIVITY_FILTERS: { id: ActivityFilter; label: string }[] = [
  { id: "all", label: "전체" },
  { id: "trade", label: "거래" },
  { id: "signal", label: "신호" },
  { id: "problem", label: "문제" },
];

function inFilter(kind: ActivityKind, filter: ActivityFilter): boolean {
  return filter === "all" || FILTER_KINDS[filter].includes(kind);
}

/** 화면에 보일 활동. 원문 줄(dev)은 개발자 로그 보기를 켰을 때만, 종류 필터는 원문 줄에도 똑같이 적용함. */
export function visibleActivity(items: Activity[], filter: ActivityFilter, showDev: boolean): Activity[] {
  return items.filter((a) => (showDev || !a.dev) && inFilter(a.kind, filter));
}

/** 필터 칩의 숫자 — 원문 줄은 세지 않음(사람이 읽는 기록의 수). */
export function countByFilter(items: Activity[]): Record<ActivityFilter, number> {
  const out: Record<ActivityFilter, number> = { all: 0, trade: 0, signal: 0, problem: 0 };
  for (const a of items) {
    if (a.dev) continue;
    out.all += 1;
    for (const f of ["trade", "signal", "problem"] as const) if (FILTER_KINDS[f].includes(a.kind)) out[f] += 1;
  }
  return out;
}
