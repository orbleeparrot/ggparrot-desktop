import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import type { ReactNode } from "react";
import { RunnerContext, type RunnerContextValue } from "../../state/context";
import { makeState, slotCard } from "../../test/fixtures";
import { SlotCardView } from "./SlotCardView";

afterEach(cleanup);

function withContext(node: ReactNode) {
  const value = {
    state: makeState(),
    act: vi.fn().mockResolvedValue(undefined),
    ok: vi.fn().mockResolvedValue(true),
    toast: vi.fn(),
    openSettings: vi.fn(),
    openLibrary: vi.fn(),
    select: vi.fn(),
    startRun: vi.fn(),
    askStop: vi.fn(),
    askRemove: vi.fn(),
  } as unknown as RunnerContextValue;
  render(<RunnerContext.Provider value={value}>{node}</RunnerContext.Provider>);
  return value;
}

describe("거래소 연결 칸", () => {
  it("바이낸스 칸도 [연결 도우미]가 있고, 누르면 그 칸(사이트)으로 도우미를 연다", () => {
    const ctx = withContext(<SlotCardView slot={slotCard("binance_demo")} />);
    expect(screen.getByRole("heading", { name: "바이낸스 데모" })).toBeTruthy();
    fireEvent.click(screen.getByRole("button", { name: "연결 도우미" }));
    expect(ctx.act).toHaveBeenCalledWith("wizard_open", { exchange: "binance", slot: "binance_demo" });
  });

  it("도우미를 쓸 수 없는 칸이면 [연결 도우미]가 없다", () => {
    withContext(<SlotCardView slot={slotCard("upbit", { wizardAvailable: false })} />);
    expect(screen.queryByRole("button", { name: "연결 도우미" })).toBeNull();
  });
});
