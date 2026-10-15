import { describe, expect, it } from "vitest";
import type { ExitRules } from "../types";
import { exitBar, exitSummary, moveFromEntry, priceAt } from "./exitBar";

// Rust exits.rs·웹 positionExits.js와 같은 모양의 규칙.
const both: ExitRules = {
  sl: { pct: 3, label: "손절 -3%" },
  tp: { pct: 5, label: "익절 +5%" },
  trailing: null,
  kind: "two",
  summary: "익절 +5% · 손절 -3%",
};
const slOnly: ExitRules = { sl: both.sl, tp: null, trailing: null, kind: "sl-only", summary: "손절 -3% · 익절은 신호" };
const trailing: ExitRules = {
  sl: both.sl,
  tp: null,
  trailing: { activation: 5, trail: 2, label: "발동 +5% · 고점 -2%" },
  kind: "two",
  summary: "트레일링 발동 +5% · 고점 -2% · 손절 -3%",
};
const none: ExitRules = { sl: null, tp: null, trailing: null, kind: "none", summary: "전략 신호" };
const tpOnly: ExitRules = { sl: null, tp: both.tp, trailing: null, kind: "tp-only", summary: "익절 +5%" };

describe("청산 기준 막대", () => {
  it("롱 — 손절가 97·평단 100·익절가 105, 현재가 102는 축의 62.5%", () => {
    const bar = exitBar(both, "long", 100, 102);
    expect(bar.show).toBe(true);
    expect(bar.left).toEqual({ label: "손절 -3%", price: 97, tone: "down" });
    expect(bar.right).toEqual({ label: "익절 +5%", price: 105, tone: "up" });
    expect(bar.entryX).toBeCloseTo(37.5, 9); // (0 − (−3)) ÷ 8
    expect(bar.priceX).toBeCloseTo(62.5, 9); // (2 + 3) ÷ 8
    expect(bar.beyond).toBeNull();
  });

  it("숏 — 손절가는 위(103), 익절가는 아래(95), 가격이 내리면 오른쪽(이익 쪽)으로", () => {
    const bar = exitBar(both, "short", 100, 98);
    expect(bar.left.price).toBeCloseTo(103, 9);
    expect(bar.right.price).toBeCloseTo(95, 9);
    expect(bar.priceX).toBeCloseTo(62.5, 9);
    expect(moveFromEntry(98, 100, "short")).toBeCloseTo(2, 9);
    expect(priceAt(100, -3, "short")).toBeCloseTo(103, 9);
  });

  it("현재가가 범위 밖이면 끝에 붙이고 어느 쪽인지 알림", () => {
    const below = exitBar(both, "long", 100, 95);
    expect([below.priceX, below.beyond]).toEqual([0, "left"]);
    const above = exitBar(both, "long", 100, 107);
    expect([above.priceX, above.beyond]).toEqual([100, "right"]);
    expect(exitBar(both, "short", 100, 106).beyond).toBe("left");
  });

  it("익절이 없으면 오른쪽 끝은 '익절은 신호'(흐린 톤), 폭은 손절 폭을 씀", () => {
    const bar = exitBar(slOnly, "long", 100, 101.5);
    expect(bar.right).toEqual({ label: "익절은 신호", price: null, tone: "muted" });
    expect(bar.entryX).toBe(50);
    expect(bar.priceX).toBeCloseTo(75, 9);
  });

  it("손절이 없으면 왼쪽 끝은 '손절 규칙 없음'", () => {
    const bar = exitBar(tpOnly, "long", 100, 100);
    expect(bar.left).toEqual({ label: "손절 규칙 없음", price: null, tone: "muted" });
    expect(bar.right.price).toBe(105);
  });

  it("트레일링 — 발동 전엔 발동가, 발동 뒤엔 '고점 -2% 추적 중'", () => {
    const before = exitBar(trailing, "long", 100, 101);
    expect(before.right).toEqual({ label: "발동 +5% · 고점 -2%", price: 105, tone: "up" });
    const armed = exitBar(trailing, "long", 100, 106.2);
    expect(armed.right.label).toBe("고점 -2% 추적 중");
    expect(armed.beyond).toBe("right");
  });

  it("막대 아래 거리 — 현재가에서 손절은 -, 익절은 +(롱 97·101·104 → -3.96% · +2.97%)", () => {
    const four: ExitRules = { ...both, tp: { pct: 4, label: "익절 +4%" } };
    const bar = exitBar(four, "long", 100, 101);
    expect([bar.toLeft, bar.toRight]).toEqual(["손절까지 -3.96%", "익절까지 +2.97%"]);
    expect(bar.move).toBeCloseTo(1, 9);
  });

  it("숏도 같은 축 — 손절(가격 위)까지 -, 익절(가격 아래)까지 +", () => {
    const bar = exitBar(both, "short", 100, 99);
    // 손절가 103: 99에서 4.04% 올라야 함(손실 쪽) · 익절가 95: 4.04% 내려야 함(이익 쪽).
    expect([bar.toLeft, bar.toRight]).toEqual(["손절까지 -4.04%", "익절까지 +4.04%"]);
  });

  it("지나친 기준은 '…지남', 익절 없음은 '익절은 신호', 손절 없음은 '손절 규칙 없음'", () => {
    expect(exitBar(both, "long", 100, 95).toLeft).toBe("손절선 지남");
    expect(exitBar(both, "long", 100, 107).toRight).toBe("익절선 지남");
    expect(exitBar(slOnly, "long", 100, 101).toRight).toBe("익절은 신호");
    expect(exitBar(tpOnly, "long", 100, 101).toLeft).toBe("손절 규칙 없음");
  });

  it("트레일링 — 발동 전엔 '발동까지', 추적 중이면 끝 이름이 말하므로 비움", () => {
    expect(exitBar(trailing, "long", 100, 101).toRight).toBe("발동까지 +3.96%");
    expect(exitBar(trailing, "long", 100, 106).toRight).toBe("");
    const fromStart: ExitRules = { ...trailing, trailing: { activation: 0, trail: 2, label: "고점 -2%" } };
    expect(exitBar(fromStart, "long", 100, 101).toRight).toBe("고점 -2% 추적 중");
  });

  it("규칙이 없거나 평단이 없으면 막대를 그리지 않음", () => {
    expect(exitBar(none, "long", 100, 101).show).toBe(false);
    expect(exitBar(both, "long", 0, 101).show).toBe(false);
  });

  it("보유가 없을 때 요약 한 줄 — 평단 대비 가격 기준임을 밝힘", () => {
    expect(exitSummary(both)).toBe("익절 +5% · 손절 -3% (평단 대비 가격)");
    expect(exitSummary(none)).toBe("전략 신호");
  });
});
