import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import type { ReactNode } from "react";
import { RunnerContext, type RunnerContextValue } from "../../state/context";
import { binanceMacro, makeSlot, makeState, upbitMacro } from "../../test/fixtures";
import { CapitalRow } from "./CapitalRow";

afterEach(cleanup);

function withContext(node: ReactNode, ok = vi.fn().mockResolvedValue(true)) {
  const value = {
    state: makeState(),
    act: vi.fn().mockResolvedValue(undefined),
    ok,
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

describe("투자 금액", () => {
  it("금액과 1회 주문 상한을 보이고, 바꾸면 set_run_capital 을 보낸다", async () => {
    const ctx = withContext(<CapitalRow slot={makeSlot({ macro: binanceMacro })} />);
    expect(screen.getByText("500 USDT")).toBeTruthy();
    // 줄글 대신 칸 — 이름 · 값.
    expect(screen.getByText("한 번에 최대").nextElementSibling?.textContent).toBe("100 USDT");
    expect(screen.getByText("손절").nextElementSibling?.textContent).toBe("평단 대비 가격 -3%");
    fireEvent.click(screen.getByRole("button", { name: "바꾸기" }));
    expect(screen.getByText(/수정된 파일/)).toBeTruthy();
    const input = screen.getByLabelText("투자 금액(USDT)");
    fireEvent.change(input, { target: { value: "1,250" } });
    fireEvent.click(screen.getByRole("button", { name: "저장" }));
    await waitFor(() => expect(ctx.ok).toHaveBeenCalledWith("set_run_capital", { runId: "run-1", amount: 1250 }));
    await waitFor(() => expect(screen.queryByLabelText("투자 금액(USDT)")).toBeNull());
  });

  it("숫자가 아니거나 0 이하면 보내지 않고 알린다", () => {
    const ctx = withContext(<CapitalRow slot={makeSlot({ macro: binanceMacro })} />);
    fireEvent.click(screen.getByRole("button", { name: "바꾸기" }));
    fireEvent.change(screen.getByLabelText("투자 금액(USDT)"), { target: { value: "0" } });
    fireEvent.click(screen.getByRole("button", { name: "저장" }));
    expect(screen.getByRole("alert").textContent).toContain("0보다 큰 금액");
    expect(ctx.ok).not.toHaveBeenCalled();
  });

  it("웹에서 연결한 매크로 · 실행 중에는 바꿀 수 없다", () => {
    withContext(<CapitalRow slot={makeSlot({ macro: upbitMacro })} />);
    const button = screen.getByRole("button", { name: "바꾸기" }) as HTMLButtonElement;
    expect(button.disabled).toBe(true);
    // 바꿀 수 없는 이유는 단추의 풍선 도움말.
    expect(button.title).toContain("웹에서 고친 뒤 다시 연결하세요");
    cleanup();
    withContext(<CapitalRow slot={makeSlot({ macro: binanceMacro, phase: "running" })} />);
    expect((screen.getByRole("button", { name: "바꾸기" }) as HTMLButtonElement).disabled).toBe(true);
  });

  it("바꾼 금액은 원본과 함께 보이고 되돌릴 수 있다", async () => {
    const ctx = withContext(
      <CapitalRow slot={makeSlot({ macro: { ...binanceMacro, capital: 800, capitalOriginal: 500 } })} />,
    );
    expect(screen.getByText(/원본 500 USDT/)).toBeTruthy();
    fireEvent.click(screen.getByRole("button", { name: "원본으로" }));
    await waitFor(() => expect(ctx.ok).toHaveBeenCalledWith("set_run_capital", { runId: "run-1", amount: null }));
  });

  it("투자 금액이 없는 매크로면 그 칸만 빠지고 나머지 칸은 그대로", () => {
    withContext(<CapitalRow slot={makeSlot({ macro: { ...binanceMacro, capital: null } })} />);
    expect(screen.queryByText("투자 금액")).toBeNull();
    expect(screen.getByText("한 번에 최대")).toBeTruthy();
  });
});
