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

describe("쓰지 않는 칸 접기 · 안내 색", () => {
  it("빈 칸은 한 줄로 접히고 › 단추(aria-expanded)로 키 칸을 펼침", () => {
    withContext(<SlotCardView slot={slotCard("binance_testnet")} />);
    const toggle = screen.getByRole("button", { name: /키 칸 펼치기/ });
    expect(toggle.getAttribute("aria-expanded")).toBe("false");
    const body = document.getElementById(toggle.getAttribute("aria-controls")!)!;
    expect(body.hidden).toBe(true);
    expect(screen.getByRole("button", { name: "연결 도우미" })).toBeTruthy();
    fireEvent.click(toggle);
    expect(toggle.getAttribute("aria-expanded")).toBe("true");
    expect(body.hidden).toBe(false);
  });

  it("쓰는 칸은 접지 않음(펼치기 단추 없음)", () => {
    withContext(
      <SlotCardView slot={slotCard("binance", { status: "connected", accessFilled: true, secretFilled: true })} />,
    );
    expect(screen.queryByRole("button", { name: /키 칸 펼치기|키 칸 접기/ })).toBeNull();
  });

  it("[설정에서 입력]으로 데려오면(openSeq) 접힌 칸이 펼쳐짐", () => {
    withContext(<SlotCardView slot={slotCard("upbit")} openSeq={1} />);
    expect(screen.getByRole("button", { name: /키 칸 접기/ }).getAttribute("aria-expanded")).toBe("true");
  });

  it("고쳐야 할 안내(warn)는 연결됨 칸이어도 주의 색 줄로, 접힌 칸에서도 보임", () => {
    withContext(
      <SlotCardView
        slot={slotCard("upbit", {
          status: "connected",
          accessFilled: true,
          secretFilled: true,
          headline: "✓ 연결됨",
          note: "주문할 원화 잔고가 모자라요.",
          noteTone: "warn",
        })}
      />,
    );
    expect(screen.getByText("주문할 원화 잔고가 모자라요.").className).toContain("note-warn");
    cleanup();
    withContext(<SlotCardView slot={slotCard("bithumb", { note: "키 저장소를 읽지 못했어요.", noteTone: "warn" })} />);
    expect(screen.getByRole("button", { name: /키 칸 펼치기/ })).toBeTruthy();
    expect(screen.getByText("키 저장소를 읽지 못했어요.").className).toContain("note-warn");
  });
});
