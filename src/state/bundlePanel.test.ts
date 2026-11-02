import { describe, expect, it } from "vitest";
import { makeRun, makeSlot } from "../test/fixtures";
import type { RunSlot } from "../types";
import { bundleLegs, bundleTotals, changeSeries } from "./bundlePanel";

function leg(index: number, over: Partial<RunSlot> = {}, key = "k1"): RunSlot {
  return makeSlot({ id: `run-${index}`, bundle: { key, index, size: 3 }, ...over });
}

describe("묶음 레그 모으기", () => {
  it("같은 묶음만, 번호 순으로 모은다", () => {
    const runs = [leg(2), makeSlot({ id: "solo", bundle: null }), leg(0), leg(1, {}, "other"), leg(1)];
    expect(bundleLegs(runs, "k1").map((r) => r.id)).toEqual(["run-0", "run-1", "run-2"]);
  });

  it("묶음이 없으면 빈 목록", () => {
    expect(bundleLegs([makeSlot({ bundle: null })], "k1")).toEqual([]);
  });
});

describe("묶음 전체 손익", () => {
  it("투입금으로 나눈다 — 비중이 달라도 맞다", () => {
    const legs = [
      leg(0, { run: makeRun({ inPosition: true, invested: 300_000, unrealized: 30_000, realizedNet: 0 }) }),
      leg(1, { run: makeRun({ inPosition: true, invested: 100_000, unrealized: -10_000, realizedNet: 0 }) }),
    ];
    const t = bundleTotals(legs);
    expect(t.invested).toBe(400_000);
    expect(t.unrealized).toBe(20_000);
    expect(t.pct).toBeCloseTo(5, 6);
    expect(t.holding).toBe(2);
  });

  it("실현손익은 손익률에 안 섞는다 — 분모가 어긋나 몇백 %가 나온다", () => {
    // 실측(`?demo=bundle`) — 투입금 100에 실현 -408을 섞으니 -407.83%가 나옴.
    const legs = [leg(0, { run: makeRun({ inPosition: true, invested: 100, unrealized: 0, realizedNet: -408 }) })];
    const t = bundleTotals(legs);
    expect(t.pct).toBe(0);
    expect(t.realized).toBe(-408);
  });

  it("아무도 안 들고 있으면 평단 대비가 null — 0%는 '본전'으로 읽힌다", () => {
    const legs = [leg(0, { run: makeRun({ inPosition: false, invested: 0, unrealized: 0, realizedNet: 1_000 }) })];
    const t = bundleTotals(legs);
    expect(t.pct).toBeNull();
    expect(t.realized).toBe(1_000);
  });

  it("보유 중인 레그 하나라도 수수료를 모르면 전체가 '수수료 전'", () => {
    const legs = [
      leg(0, { run: makeRun({ inPosition: true, invested: 1, unrealizedFeeAware: true }) }),
      leg(1, { run: makeRun({ inPosition: true, invested: 1, unrealizedFeeAware: false }) }),
    ];
    expect(bundleTotals(legs).feeAware).toBe(false);
  });

  it("안 들고 있는 레그의 수수료 기준은 전체를 흐리지 않는다", () => {
    const legs = [
      leg(0, { run: makeRun({ inPosition: true, invested: 1, unrealizedFeeAware: true }) }),
      leg(1, { run: makeRun({ inPosition: false, invested: 0, unrealizedFeeAware: false }) }),
    ];
    expect(bundleTotals(legs).feeAware).toBe(true);
  });

  it("아직 안 띄운 레그가 섞여도 깨지지 않는다", () => {
    expect(bundleTotals([leg(0, { run: null })]).invested).toBe(0);
  });
});

describe("등락률 흐름", () => {
  it("첫 점 대비 퍼센트로 바꾼다", () => {
    expect(changeSeries([100, 110, 90])).toEqual([0, 10, -10]);
  });

  it("점이 모자라거나 첫 점이 0이면 빈 배열", () => {
    expect(changeSeries([100])).toEqual([]);
    expect(changeSeries([0, 10])).toEqual([]);
  });
});
