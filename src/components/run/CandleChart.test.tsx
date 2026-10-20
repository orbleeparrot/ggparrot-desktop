// 조건 차트가 실제로 무엇을 그리는지 — 선의 개수가 아니라 **사용자가 읽는 글자**를 본다.
import { cleanup, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it } from "vitest";
import { CandleChart } from "./CandleChart";
import type { Candle, ChartData, ChartLevels } from "../../state/candles";

afterEach(cleanup);

const HOUR = 3_600_000;
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

function bars(count: number): Candle[] {
  return Array.from({ length: count }, (_, i): Candle => [i * HOUR, 100 + i, 102 + i, 99 + i, 101 + i]);
}

function chart(over: Partial<ChartData> = {}): ChartData {
  return { interval: "1h", bars: bars(30), levels: EMPTY, ...over };
}

describe("조건 차트", () => {
  it("사는 선과 손절선을 이름과 함께 보인다", () => {
    // 색만으로 가르면 색을 못 가리는 사람에게는 아무 뜻이 없다.
    render(
      <CandleChart
        chart={chart({ levels: { ...EMPTY, entry_target: 120, stop_loss: 95 } })}
        price={110}
        quote="USDT"
        intervalLabel="1시간"
      />,
    );
    expect(screen.getByText(/사는 선/)).toBeTruthy();
    expect(screen.getByText(/손절선/)).toBeTruthy();
  });

  it("없는 선은 아예 그리지 않는다", () => {
    render(<CandleChart chart={chart()} price={110} quote="USDT" intervalLabel="1시간" />);
    expect(screen.queryByText(/사는 선/)).toBeNull();
    expect(screen.queryByText(/손절선/)).toBeNull();
    // 지금 가격은 늘 보인다.
    expect(screen.getByText(/^지금 /)).toBeTruthy();
  });

  it("봉을 다 그린다", () => {
    const { container } = render(<CandleChart chart={chart()} price={110} quote="USDT" intervalLabel="1시간" />);
    expect(container.querySelectorAll(".candle").length).toBe(30);
  });

  it("첫 봉 전에는 '곧 나온다' 고 말한다", () => {
    // "값이 없음" 과 "이 규칙엔 그 선이 없음" 은 다르다.
    render(
      <CandleChart
        chart={{ interval: "1h", bars: [], levels: { ...EMPTY, pending: true } }}
        price={110}
        quote="USDT"
        intervalLabel="1시간"
      />,
    );
    expect(screen.getByText(/첫 봉이 마감되면/)).toBeTruthy();
  });

  it("RSI 는 가격선이 아니라 아래 줄에 값으로 나온다", () => {
    const { container } = render(
      <CandleChart
        chart={chart({
          levels: {
            ...EMPTY,
            indicator: { name: "RSI", value: 23.14, entry_threshold: 30, exit_threshold: 70 },
          },
        })}
        price={110}
        quote="USDT"
        intervalLabel="1시간"
      />,
    );
    const note = container.querySelector(".candle-note")?.textContent ?? "";
    expect(note).toContain("RSI 23.1");
    expect(note).toContain("사는 선 30");
  });

  it("보유 중이면 평단을 겹친다", () => {
    const { container } = render(
      <CandleChart chart={chart()} price={110} entry={105} quote="USDT" intervalLabel="1시간" />,
    );
    expect(screen.getByText(/평단/)).toBeTruthy();
    expect(container.querySelector(".candle-line.is-entry")).toBeTruthy();
  });

  it("봉 간격과 개수를 적는다", () => {
    const { container } = render(<CandleChart chart={chart()} price={110} quote="USDT" intervalLabel="1시간" />);
    expect(container.querySelector(".candle-note")?.textContent).toContain("1시간 봉 30개");
  });
});
