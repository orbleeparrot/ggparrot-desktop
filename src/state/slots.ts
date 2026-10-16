// 설정 › 거래소 연결 칸 표시 규칙 — 쓰는 칸을 앞에, 안 쓰는 칸은 접음, 안내 색.
import type { SlotCard } from "../types";

/** noteTone이 없을 때 고쳐야 할 안내로 볼 문구(잔고·권한·허용 IP). */
const WARN_WORDS = /잔고|권한|허용 IP|모자라/;

/** 칸 아래 안내의 색 — Rust noteTone을 따르고, 없으면 막힘(blocked)·확인 실패(bad)·문구로 짐작함. */
export function noteToneOf(s: SlotCard): "muted" | "warn" {
  if (s.noteTone) return s.noteTone;
  if (s.status === "blocked") return "warn";
  if (Object.values(s.checks).includes("bad")) return "warn";
  return WARN_WORDS.test(s.note) ? "warn" : "muted";
}

/** 쓰는 칸 — 키가 하나라도 있거나, 연결 상태가 비어 있지 않거나, 실행 중인 매크로가 씀. 이런 칸은 접지 않고 앞에 둠. */
export function slotInUse(s: SlotCard): boolean {
  return s.status !== "empty" || s.accessFilled || s.secretFilled || s.inUseBy > 0;
}

/** 칸 순서 — 쓰는 칸 먼저, 그 안에서는 받은 순서 그대로(안정 정렬). 칸 이름(slot) 목록을 돌려줌. */
export function slotOrder(slots: SlotCard[]): SlotCard["slot"][] {
  return [...slots.filter(slotInUse), ...slots.filter((s) => !slotInUse(s))].map((s) => s.slot);
}

/**
 * 정해 둔 순서(order)대로 칸을 늘어놓음 — 설정을 여는 동안 키를 넣어 '쓰는 칸'이 돼도 자리가 튀지 않게.
 * order에 없는 새 칸은 뒤에 붙임.
 */
export function arrangeSlots(slots: SlotCard[], order: SlotCard["slot"][]): SlotCard[] {
  const rank = (s: SlotCard) => {
    const i = order.indexOf(s.slot);
    return i < 0 ? order.length : i;
  };
  return slots
    .map((s, i) => ({ s, i }))
    .sort((a, b) => rank(a.s) - rank(b.s) || a.i - b.i)
    .map((x) => x.s);
}

/**
 * 두 칸씩 놓이는 거래소 칸에서 한 줄을 다 쓸 칸 — 펼친 칸이 이어진 묶음에서 짝이 없는 마지막 칸(홀수 번째).
 * 펼친 칸 하나만 반쪽 폭으로 놓여 옆이 비는 것을 막음. 접힌 칸은 CSS가 늘 한 줄로 둠.
 */
export function wideSlots(expanded: readonly boolean[]): boolean[] {
  let run = 0;
  return expanded.map((open, i) => {
    run = open ? run + 1 : 0;
    const lastOfRun = !expanded[i + 1];
    return open && lastOfRun && run % 2 === 1;
  });
}
