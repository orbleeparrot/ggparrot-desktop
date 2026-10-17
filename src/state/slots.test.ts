import { describe, expect, it } from "vitest";
import { slotCard } from "../test/fixtures";
import { arrangeSlots, isPractice, noteToneOf, slotInUse, slotOrder } from "./slots";

describe("거래소 연결 칸 순서·접기", () => {
  const slots = [
    slotCard("binance"),
    slotCard("binance_testnet"),
    slotCard("binance_demo", { accessFilled: true }),
    slotCard("upbit", { status: "connected", accessFilled: true, secretFilled: true }),
    slotCard("bithumb"),
  ];

  it("쓰는 칸(키·연결 상태·실행 중)만 펼침 대상", () => {
    expect(slots.map(slotInUse)).toEqual([false, false, true, true, false]);
    expect(slotInUse(slotCard("bithumb", { inUseBy: 1 }))).toBe(true);
    expect(slotInUse(slotCard("bithumb", { status: "blocked" }))).toBe(true);
  });

  it("쓰는 칸 먼저, 그 안·밖에서는 받은 순서 그대로", () => {
    expect(slotOrder(slots)).toEqual(["binance_demo", "upbit", "binance", "binance_testnet", "bithumb"]);
  });

  it("정해 둔 순서를 지킴 — 나중에 키를 넣어도 자리가 바뀌지 않고, 모르는 칸은 뒤에", () => {
    const order = slotOrder(slots);
    const later = slots.map((s) => (s.slot === "bithumb" ? { ...s, accessFilled: true } : s));
    expect(arrangeSlots(later, order).map((s) => s.slot)).toEqual(order);
    expect(arrangeSlots(later, ["upbit"]).map((s) => s.slot)).toEqual([
      "upbit",
      "binance",
      "binance_testnet",
      "binance_demo",
      "bithumb",
    ]);
  });

  it("안내 색 — noteTone을 따르고, 없으면 막힘·확인 실패·잔고 문구면 주의", () => {
    expect(noteToneOf(slotCard("upbit", { note: "참고", noteTone: "warn" }))).toBe("warn");
    expect(noteToneOf(slotCard("upbit", { note: "주문할 원화 잔고가 모자라요.", noteTone: "muted" }))).toBe("muted");
    const derived = { noteTone: undefined };
    expect(noteToneOf(slotCard("upbit", { ...derived, status: "blocked", note: "키가 맞지 않아요." }))).toBe("warn");
    expect(
      noteToneOf(slotCard("upbit", { ...derived, status: "connected", note: "주문할 원화 잔고가 모자라요." })),
    ).toBe("warn");
    expect(
      noteToneOf(
        slotCard("upbit", {
          ...derived,
          status: "connected",
          note: "x",
          checks: { auth: "ok", ip: "ok", order: "bad" },
        }),
      ),
    ).toBe("warn");
    expect(noteToneOf(slotCard("upbit", { ...derived, status: "connected", note: "저장 위치 안내" }))).toBe("muted");
  });
});

describe("연습 칸", () => {
  it("바이낸스 테스트넷 · 데모만 연습 칸", () => {
    expect(isPractice(slotCard("binance_testnet"))).toBe(true);
    expect(isPractice(slotCard("binance_demo"))).toBe(true);
    expect(isPractice(slotCard("binance"))).toBe(false);
    expect(isPractice(slotCard("upbit"))).toBe(false);
  });
});
