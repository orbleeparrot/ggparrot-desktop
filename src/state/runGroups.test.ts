import { describe, expect, it } from "vitest";
import { binanceMacro, makeSlot, upbitMacro } from "../test/fixtures";
import { globalIndex, groupOf, groupRuns, matches } from "./runGroups";

describe("목록 묶음", () => {
  it("확인 필요는 실행 중이어도 위로, 나머지는 실행 중 · 대기", () => {
    expect(groupOf(makeSlot({ attention: true, phase: "running" }))).toBe("attention");
    expect(groupOf(makeSlot({ phase: "running" }))).toBe("running");
    expect(groupOf(makeSlot({ phase: "checking" }))).toBe("running");
    expect(groupOf(makeSlot({ phase: "finished" }))).toBe("idle");
    expect(groupOf(makeSlot({ attention: true, phase: "finished" }))).toBe("attention");
  });

  it("묶음 순서는 확인 필요 → 실행 중 → 대기, 묶음 안은 저장된 순서, 빈 묶음은 뺌", () => {
    const runs = [
      makeSlot({ id: "a" }),
      makeSlot({ id: "b", phase: "running" }),
      makeSlot({ id: "c", phase: "running", attention: true }),
      makeSlot({ id: "d" }),
    ];
    expect(groupRuns(runs).map((g) => [g.key, g.items.map((r) => r.id)])).toEqual([
      ["attention", ["c"]],
      ["running", ["b"]],
      ["idle", ["a", "d"]],
    ]);
    expect(groupRuns([makeSlot({ id: "a" })]).map((g) => g.key)).toEqual(["idle"]);
  });

  it("찾기 — 종목 · 이름 · 전략 · 거래소에 낱말이 다 있으면", () => {
    const btc = makeSlot({ macro: binanceMacro });
    const krw = makeSlot({ macro: upbitMacro });
    expect(matches(btc, "btc")).toBe(true);
    expect(matches(krw, "업비트 rsi")).toBe(true);
    expect(matches(btc, "업비트")).toBe(false);
    expect(matches(btc, "  ")).toBe(true);
  });

  it("묶음 안에서 옮긴 뒤 전체 번호 — 다른 묶음 사이 순서는 그대로", () => {
    // 전체 [a(대기), b(실행), c(대기), d(대기)] — 대기 묶음은 [a, c, d]
    const all = ["a", "b", "c", "d"];
    const idle = ["a", "c", "d"];
    // d를 대기 묶음 맨 앞(a 앞)으로 → 자기를 뺀 [a, b, c]에서 a 자리 0
    expect(globalIndex(all, idle, "d", 0)).toBe(0);
    // a를 대기 묶음 맨 뒤(d 뒤)로 → 자기를 뺀 [b, c, d]에서 d 뒤 3
    expect(globalIndex(all, idle, "a", 2)).toBe(3);
    // a를 c 뒤·d 앞으로 → [b, c, d]에서 d 자리 2
    expect(globalIndex(all, idle, "a", 1)).toBe(2);
  });
});
