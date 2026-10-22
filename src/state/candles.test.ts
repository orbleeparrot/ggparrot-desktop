// 봉 차트 눈금 — 선이 차트 밖으로 나가거나, 없는 선을 그리거나 하지 않는지.
import { describe, expect, it } from "vitest";
import {
  LINE_ROOM,
  candleRange,
  candleWidth,
  candleX,
  candleY,
  chartSpanMs,
  priceLines,
  type Candle,
  type ChartLevels,
} from "./candles";

const HOUR = 3_600_000;

function bars(count: number, base = 100): Candle[] {
  return Array.from({ length: count }, (_, i): Candle => [
    i * HOUR,
    base + i,
    base + i + 2,
    base + i - 1,
    base + i + 1,
  ]);
}

const EMPTY: ChartLevels = {
  entry_target: null,
  exit_target: null,
  stop_loss: null,
  trail_stop: null,
  trail_activation: null,
  armed: false,
  reference: null,
  band_upper: null,
  band_lower: null,
  next_buy_ms: null,
  indicator: null,
  pending: false,
};

describe("가격선", () => {
  it("엔진이 낸 선만 그린다", () => {
    // 없는 선을 추정으로 메우면 그 추정선이 실제 조건인 척한다 — 한 번 겪은 일이다.
    expect(priceLines(EMPTY, 100)).toEqual([]);
    expect(priceLines(null, 100)).toEqual([]);
    expect(priceLines(undefined, 100)).toEqual([]);
  });

  it("사는 선과 파는 선을 색으로 가른다", () => {
    const lines = priceLines({ ...EMPTY, entry_target: 90, exit_target: 110 }, 100);
    expect(lines.map((l) => [l.key, l.tone])).toEqual([
      ["entry", "buy"],
      ["exit", "sell"],
    ]);
  });

  it("가격이 아닌 값은 선이 되지 않는다", () => {
    const bad = { ...EMPTY, entry_target: 0, exit_target: -1, stop_loss: Number.NaN };
    expect(priceLines(bad, 100)).toEqual([]);
  });

  it("그리드 칸은 지금 가격에서 가까운 것만 그린다", () => {
    // 칸을 전부 그리면 차트가 줄무늬가 되어 아무것도 읽을 수 없다.
    const levels = { ...EMPTY, buy_levels: [10, 50, 95, 99, 300], sell_levels: [101, 105, 500] };
    const lines = priceLines(levels, 100);
    expect(lines.filter((l) => l.key.startsWith("grid-buy")).map((l) => l.value)).toEqual([99, 95]);
    expect(lines.filter((l) => l.key.startsWith("grid-sell")).map((l) => l.value)).toEqual([101, 105]);
  });

  it("트레일 발동선은 발동 전에만 온다", () => {
    // 켜진 뒤에는 엔진이 null 을 보낸다 — 화면이 임의로 지우지 않는다.
    const before = priceLines({ ...EMPTY, trail_activation: 105 }, 100);
    expect(before.map((l) => l.key)).toContain("arm");
    const after = priceLines({ ...EMPTY, trail_stop: 98, armed: true }, 100);
    expect(after.map((l) => l.key)).toEqual(["trail"]);
  });
});

describe("눈금", () => {
  it("가격선이 차트 밖으로 나가지 않는다", () => {
    // 선이 밖으로 나가면 "어디서 사는지" 를 볼 수 없다.
    const rows = bars(20, 100);
    const lines = priceLines({ ...EMPTY, entry_target: 50, exit_target: 400 }, 110);
    const range = candleRange(rows, lines);
    for (const line of lines) {
      const y = candleY(line.value, range, 120);
      expect(y).toBeGreaterThanOrEqual(0);
      expect(y).toBeLessThanOrEqual(120);
    }
  });

  it("먼 가격선이 봉을 납작하게 만들지 않는다", () => {
    // BTC 에서 익절 +5% · 손절 -3% 가 5시간치 봉 범위보다 훨씬 넓어 봉 300개가 한 줄이 됐다.
    const rows = bars(20, 100); // 고가 102~121 · 저가 99~118 → 범위 22
    const lines = priceLines({ ...EMPTY, stop_loss: 10, exit_target: 1000 }, 110);
    const range = candleRange(rows, lines);
    const candleSpan = 121 - 99;
    expect(range.max - range.min).toBeLessThanOrEqual(candleSpan * (1 + LINE_ROOM * 2));
    // 봉이 세로의 상당 부분을 차지한다.
    expect(candleSpan / (range.max - range.min)).toBeGreaterThan(0.2);
  });

  it("가까운 가격선은 그대로 담는다", () => {
    const rows = bars(20, 100);
    const lines = priceLines({ ...EMPTY, entry_target: 95, exit_target: 125 }, 110);
    const range = candleRange(rows, lines);
    expect(range.min).toBeLessThanOrEqual(95);
    expect(range.max).toBeGreaterThanOrEqual(125);
  });

  it("평평한 봉에서도 범위가 0 이 되지 않는다", () => {
    const flat: Candle[] = [
      [0, 100, 100, 100, 100],
      [HOUR, 100, 100, 100, 100],
    ];
    const range = candleRange(flat);
    expect(range.max).toBeGreaterThan(range.min);
    expect(Number.isFinite(candleY(100, range, 120))).toBe(true);
  });

  it("봉이 없으면 안전한 범위를 준다", () => {
    const range = candleRange([]);
    expect(Number.isFinite(range.min) && Number.isFinite(range.max)).toBe(true);
    expect(range.max).toBeGreaterThan(range.min);
  });

  it("높은 값이 위에 온다", () => {
    const range = { min: 0, max: 100 };
    expect(candleY(100, range, 120)).toBeLessThan(candleY(0, range, 120));
  });

  it("봉이 순서대로 왼쪽에서 오른쪽으로 간다", () => {
    const xs = [0, 1, 2, 3].map((i) => candleX(i, 4, 320));
    expect(xs).toEqual([...xs].sort((a, b) => a - b));
    expect(xs[0]).toBeGreaterThanOrEqual(0);
    expect(xs[3]).toBeLessThanOrEqual(320);
  });

  it("봉이 많아도 너비가 0 이 되지 않는다", () => {
    expect(candleWidth(300, 320)).toBeGreaterThanOrEqual(1);
    expect(candleWidth(0, 320)).toBeGreaterThanOrEqual(1);
  });

  it("차트가 덮는 시간을 센다", () => {
    expect(chartSpanMs(bars(25))).toBe(24 * HOUR);
    expect(chartSpanMs([])).toBe(0);
  });
});
