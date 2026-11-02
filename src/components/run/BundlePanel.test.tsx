import { cleanup, fireEvent, render, screen, within } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { makeRun, makeSlot, upbitMacro } from "../../test/fixtures";
import type { RunSlot } from "../../types";
import { BundlePanel } from "./BundlePanel";

afterEach(cleanup);

function leg(index: number, symbol: string, over: Partial<RunSlot> = {}): RunSlot {
  return makeSlot({
    id: `run-${index}`,
    macro: { ...upbitMacro, symbol, baseAsset: symbol.replace("KRW-", "") },
    bundle: { key: "k1", index, size: 3 },
    ...over,
  });
}

const LEGS = [
  leg(0, "KRW-BTC", {
    status: { text: "실행 중", tone: "ok" },
    run: makeRun({ inPosition: true, invested: 100_000, unrealized: 3_000, unrealizedPct: 3, realizedNet: 0 }),
  }),
  leg(1, "KRW-ETH", {
    status: { text: "실행 중", tone: "ok" },
    run: makeRun({ inPosition: true, invested: 100_000, unrealized: -1_000, unrealizedPct: -1, realizedNet: 0 }),
  }),
  leg(2, "KRW-XRP", { status: { text: "진입 대기", tone: "muted" }, run: makeRun({ inPosition: false, invested: 0 }) }),
];

const PRICES = {
  "run-0": { points: [100, 103], times: [1, 2] },
  "run-1": { points: [50, 49.5], times: [1, 2] },
  "run-2": { points: [10, 10.5], times: [1, 2] },
};

function show(open = vi.fn(), current = LEGS[0]) {
  render(<BundlePanel slot={current} runs={LEGS} prices={PRICES} onOpen={open} />);
  return open;
}

describe("묶음 판", () => {
  it("묶음이 아니면 아무것도 안 그린다", () => {
    const { container } = render(
      <BundlePanel slot={makeSlot({ bundle: null })} runs={[]} prices={{}} onOpen={vi.fn()} />,
    );
    expect(container.firstChild).toBeNull();
  });

  it("전체 손익률을 투입금 기준으로 적는다", () => {
    show();
    // (3,000 - 1,000) / 200,000 = +1.00%
    expect(screen.getByTestId("bundle-total-pct").textContent).toContain("+1.00%");
  });

  it("종목마다 탭이 하나씩, 등락률이 함께 붙는다", () => {
    show();
    const tabs = screen.getAllByRole("tab");
    expect(tabs.map((t) => t.textContent)).toEqual(["BTC+3.00%", "ETH-1.00%", "XRP+5.00%"]);
  });

  it("탭을 누르면 그 종목 차트로 바뀐다 — 레그를 옮겨 다니지 않는다", () => {
    const open = show();
    fireEvent.click(screen.getAllByRole("tab")[1]);
    expect(screen.getByTestId("bundle-chart").getAttribute("data-symbol")).toBe("KRW-ETH");
    expect(open).not.toHaveBeenCalled();
  });

  it("처음에는 지금 보고 있는 레그가 켜져 있다", () => {
    show(vi.fn(), LEGS[2]);
    expect(screen.getByTestId("bundle-chart").getAttribute("data-symbol")).toBe("KRW-XRP");
  });

  it("종목마다 상태 · 평단 대비 · 투입금을 한 줄로 적는다", () => {
    show();
    const row = screen.getByTestId("bundle-row-run-1");
    expect(within(row).getByText("ETH")).toBeTruthy();
    expect(row.textContent).toContain("실행 중");
    expect(row.textContent).toContain("-1.00%");
  });

  it("안 들고 있는 레그는 손익 자리를 비워 둔다 — 0%로 적으면 본전으로 읽힌다", () => {
    show();
    expect(screen.getByTestId("bundle-row-run-2").textContent).toContain("—");
  });

  it("줄을 누르면 그 레그 상세로 간다", () => {
    const open = show();
    fireEvent.click(screen.getByTestId("bundle-row-run-2"));
    expect(open).toHaveBeenCalledWith("run-2");
  });

  it("묶음 전체 종료가 판 안에 있다", () => {
    show();
    expect(screen.getByRole("button", { name: /묶음 전체/ })).toBeTruthy();
  });
});
