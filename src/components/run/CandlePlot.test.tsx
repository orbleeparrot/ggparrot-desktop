// 조건 차트 — 엔진이 준 값을 **그대로** 넘기는지, 없는 것을 지어내지 않는지.
//
// lightweight-charts 는 캔버스가 있어야 돌아가 jsdom 에서 그릴 수 없다. 그래서 그리기는
// 가짜로 두고, **무엇을 넘기는지** 를 본다 — 화면이 값을 다시 계산하면 규칙 산수가 두
// 벌이 되고 그중 하나는 엔진이 보는 것과 다르다.
import { cleanup, render, screen } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { ChartData, ChartLevels } from "../../state/candles";

const setData = vi.fn();
const createPriceLine = vi.fn();
const addSeries = vi.fn();

vi.mock("lightweight-charts", () => {
  const series = () => ({
    setData,
    createPriceLine,
    applyOptions: vi.fn(),
    priceScale: () => ({ applyOptions: vi.fn() }),
  });
  return {
    CandlestickSeries: "candles",
    LineSeries: "line",
    CrosshairMode: { Normal: 0 },
    LineStyle: { Dashed: 2, Dotted: 1 },
    createChart: () => ({
      addSeries: (...args: unknown[]) => {
        addSeries(...args);
        return series();
      },
      timeScale: () => ({
        fitContent: vi.fn(),
        subscribeVisibleLogicalRangeChange: vi.fn(),
        setVisibleLogicalRange: vi.fn(),
      }),
      remove: vi.fn(),
    }),
  };
});

const { CandlePlot } = await import("./CandlePlot");

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

const HOUR = 3_600_000;
function chartOf(over: Partial<ChartData> = {}): ChartData {
  return {
    interval: "1h",
    bars: Array.from({ length: 5 }, (_, i) => [i * HOUR, 100 + i, 102 + i, 99 + i, 101 + i]),
    levels: EMPTY,
    ...over,
  };
}

beforeEach(() => {
  setData.mockClear();
  createPriceLine.mockClear();
  addSeries.mockClear();
});
afterEach(cleanup);

describe("조건 차트", () => {
  it("봉이 모자라면 그리지 않고 왜 비었는지 말한다", () => {
    render(<CandlePlot chart={chartOf({ bars: [] })} price={100} quote="KRW" intervalLabel="1시간" />);
    expect(screen.getByText("봉을 받는 중이에요.")).toBeTruthy();
  });

  it("첫 봉 마감 전이면 조건선이 아직이라고 말한다", () => {
    // "이 규칙엔 그 선이 없다" 와 "아직 모른다" 는 다른 말이다.
    render(
      <CandlePlot
        chart={chartOf({ bars: [], levels: { ...EMPTY, pending: true } })}
        price={100}
        quote="KRW"
        intervalLabel="1시간"
      />,
    );
    expect(screen.getByText("첫 봉이 마감되면 조건선이 나와요.")).toBeTruthy();
  });

  it("엔진이 준 지표값을 그대로 넘긴다", () => {
    const chart = chartOf({
      series: [{ id: "ma_fast", label: "단기 3", values: [null, null, 101, 102, 103] }],
    });
    render(<CandlePlot chart={chart} price={105} quote="KRW" intervalLabel="1시간" />);
    const line = setData.mock.calls.find(([rows]) => Array.isArray(rows) && "value" in (rows[2] ?? {}));
    expect(line?.[0].map((row: { value?: number }) => row.value)).toEqual([undefined, undefined, 101, 102, 103]);
  });

  it("값이 없는 앞쪽은 0 으로 메우지 않는다", () => {
    // 0 으로 채우면 화면이 그 0 을 진짜 지표값으로 그린다.
    const chart = chartOf({ series: [{ id: "ma_slow", label: "장기 5", values: [null, null, null, null, 100] }] });
    render(<CandlePlot chart={chart} price={100} quote="KRW" intervalLabel="1시간" />);
    const line = setData.mock.calls.find(([rows]) => Array.isArray(rows) && rows.length === 5 && !("open" in rows[0]));
    expect(line?.[0][0]).not.toHaveProperty("value");
  });

  it("엔진이 낸 가격선만 긋는다", () => {
    render(
      <CandlePlot
        chart={chartOf({ levels: { ...EMPTY, entry_target: 99, stop_loss: 95 } })}
        price={100}
        quote="KRW"
        intervalLabel="1시간"
      />,
    );
    const titles = createPriceLine.mock.calls.map(([opt]) => opt.title);
    expect(titles).toEqual(["사는 선", "손절선"]);
  });

  it("선이 없는 규칙에는 아무 선도 긋지 않는다", () => {
    render(<CandlePlot chart={chartOf()} price={100} quote="KRW" intervalLabel="1시간" />);
    expect(createPriceLine).not.toHaveBeenCalled();
  });

  it("범례에 지표선과 가격선 이름이 함께 나온다", () => {
    // 색만으로 가르면 색을 못 가리는 사람에게는 아무 뜻이 없다.
    render(
      <CandlePlot
        chart={chartOf({
          series: [{ id: "band_upper", label: "위 밴드 20", values: [1, 2, 3, 4, 5] }],
          levels: { ...EMPTY, entry_target: 99 },
        })}
        price={100}
        quote="KRW"
        intervalLabel="1시간"
      />,
    );
    expect(screen.getByText("위 밴드 20")).toBeTruthy();
    expect(screen.getByText(/사는 선/)).toBeTruthy();
  });

  it("RSI 는 아래 칸에 따로 그리고 문턱을 함께 긋는다", () => {
    render(
      <CandlePlot
        chart={chartOf({
          oscillator: { label: "RSI 14", values: [null, 40, 50, 60, 70], entry: 30, exit: 70 },
        })}
        price={100}
        quote="KRW"
        intervalLabel="1시간"
      />,
    );
    const titles = createPriceLine.mock.calls.map(([opt]) => opt.title);
    expect(titles).toEqual(["사는 문턱", "파는 문턱"]);
    expect(screen.getByText("RSI 14")).toBeTruthy();
  });
});
