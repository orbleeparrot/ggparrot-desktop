// 봉 차트 눈금 — 선이 차트 밖으로 나가거나, 없는 선을 그리거나 하지 않는지.
import { describe, expect, it } from "vitest";
import { LINE_ROOM, candleRange, priceLines, type Candle, type ChartLevels } from "./candles";

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
    // 없는 선을 추정으로 메우면 그 추정선이 실제 조건인 척함.
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
    // 칸을 전부 그리면 차트가 줄무늬가 되어 아무것도 읽을 수 없음.
    const levels = { ...EMPTY, buy_levels: [10, 50, 95, 99, 300], sell_levels: [101, 105, 500] };
    const lines = priceLines(levels, 100);
    expect(lines.filter((l) => l.key.startsWith("grid-buy")).map((l) => l.value)).toEqual([99, 95]);
    expect(lines.filter((l) => l.key.startsWith("grid-sell")).map((l) => l.value)).toEqual([101, 105]);
  });

  it("트레일 발동선은 발동 전에만 온다", () => {
    // 켜진 뒤에는 엔진이 null을 보냄 — 화면이 임의로 지우지 않음.
    const before = priceLines({ ...EMPTY, trail_activation: 105 }, 100);
    expect(before.map((l) => l.key)).toContain("arm");
    const after = priceLines({ ...EMPTY, trail_stop: 98, armed: true }, 100);
    expect(after.map((l) => l.key)).toEqual(["trail"]);
  });
});

describe("눈금", () => {
  it("먼 가격선이 봉을 납작하게 만들지 않는다", () => {
    // BTC에서 익절 +5%·손절 -3%가 5시간치 봉 범위보다 훨씬 넓어 봉 300개가 한 줄이 됨.
    const rows = bars(20, 100); // 고가 102~121·저가 99~118 → 범위 22
    const lines = priceLines({ ...EMPTY, stop_loss: 10, exit_target: 1000 }, 110);
    const range = candleRange(rows, lines);
    const candleSpan = 121 - 99;
    expect(range.max - range.min).toBeLessThanOrEqual(candleSpan * (1 + LINE_ROOM * 2));
    // 봉이 세로의 상당 부분을 차지함.
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
    expect(Number.isFinite(range.min) && Number.isFinite(range.max)).toBe(true);
  });

  it("봉이 없으면 안전한 범위를 준다", () => {
    const range = candleRange([]);
    expect(Number.isFinite(range.min) && Number.isFinite(range.max)).toBe(true);
    expect(range.max).toBeGreaterThan(range.min);
  });
});
