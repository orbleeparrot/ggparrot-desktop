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

/** 연습 칸(바이낸스 테스트넷 · 데모) — 쓰지 않으면 설정 아래 '연습 칸'에 낮게 둠. */
export function isPractice(s: Pick<SlotCard, "slot">): boolean {
  return s.slot === "binance_testnet" || s.slot === "binance_demo";
}
