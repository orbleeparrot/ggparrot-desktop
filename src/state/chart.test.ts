import { describe, expect, it } from "vitest";
import { chartDirection, chartRange, chartY, spreadLabels } from "./chart";

describe("chartRange · chartY", () => {
  it("평단을 범위에 넣고, 값이 모두 같으면 위아래로 벌림", () => {
    expect(chartRange([10, 12, 11], 15)).toEqual({ min: 10, max: 15 });
    expect(chartRange([10, 12], 0)).toEqual({ min: 10, max: 12 });
    expect(chartRange([5, 5])).toEqual({ min: 4, max: 6 });
  });
  it("최고값은 위(pad), 최저값은 아래(height - pad)", () => {
    const r = { min: 0, max: 100 };
    expect(chartY(100, r, 64)).toBe(4);
    expect(chartY(0, r, 64)).toBe(60);
    expect(chartY(50, r, 64)).toBe(32);
  });
  it("방향은 처음과 마지막을 견줌", () => {
    expect(chartDirection([1, 2])).toBe("up");
    expect(chartDirection([2, 2])).toBe("up");
    expect(chartDirection([2, 1])).toBe("down");
    expect(chartDirection([])).toBe("up");
  });
});

describe("spreadLabels", () => {
  it("멀리 떨어져 있으면 그대로", () => {
    expect(spreadLabels([10, 50], 64, 14, 8)).toEqual([10, 50]);
  });
  it("가까우면 아래 것을 밀어 내림 — 순서는 받은 대로", () => {
    expect(spreadLabels([30, 25], 64, 14, 8)).toEqual([39, 25]);
  });
  it("바닥에 닿으면 위로 되밀어 올림", () => {
    expect(spreadLabels([58, 60], 64, 14, 8)).toEqual([42, 56]);
  });
  it("위아래 가장자리 밖은 안으로 들임", () => {
    expect(spreadLabels([-5], 64, 14, 8)).toEqual([8]);
    expect(spreadLabels([80], 64, 14, 8)).toEqual([56]);
  });
  it("같은 자리면 먼저 받은 것이 위", () => {
    expect(spreadLabels([20, 20], 64, 14, 8)).toEqual([20, 34]);
  });
});
