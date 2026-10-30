import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { useState, type ReactNode } from "react";
import { RunnerContext, type RunnerContextValue } from "../../state/context";
import { makeState, slotCard } from "../../test/fixtures";
import type { SlotCard } from "../../types";
import { amountOf, SlotCardView } from "./SlotCardView";

afterEach(cleanup);

/** 설정 화면처럼 펼침 상태를 바깥에서 들고 있는 칸. */
function Slot({ slot }: { slot: SlotCard }) {
  const [open, setOpen] = useState(false);
  return <SlotCardView slot={slot} open={open} onToggle={setOpen} />;
}

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
  it("이름 옆 [↗]는 그 칸의 거래소 사이트를 연다(머리줄에서 옮겨 옴)", () => {
    const ctx = withContext(<Slot slot={slotCard("binance_demo")} />);
    fireEvent.click(screen.getByRole("button", { name: "바이낸스 데모 사이트 열기" }));
    expect(ctx.act).toHaveBeenCalledWith("open_slot_site", { slot: "binance_demo" });
  });

  it("바이낸스 칸도 [연결 도우미]가 있고, 누르면 그 칸(사이트)으로 도우미를 연다", () => {
    const ctx = withContext(<Slot slot={slotCard("binance_demo")} />);
    expect(screen.getByRole("heading", { name: "바이낸스 데모" })).toBeTruthy();
    fireEvent.click(screen.getByRole("button", { name: "연결 도우미" }));
    expect(ctx.act).toHaveBeenCalledWith("wizard_open", { exchange: "binance", slot: "binance_demo" });
  });

  it("도우미를 쓸 수 없는 칸이면 [연결 도우미]가 없다", () => {
    withContext(<Slot slot={slotCard("upbit", { wizardAvailable: false })} />);
    expect(screen.queryByRole("button", { name: "연결 도우미" })).toBeNull();
  });
});

describe("키 칸 열고 닫기 · 잔고 · 안내 색", () => {
  it("키 칸은 처음엔 닫혀 있고 ▾ 단추(aria-expanded)로 엶", () => {
    withContext(<Slot slot={slotCard("binance_testnet")} />);
    const toggle = screen.getByRole("button", { name: /키 칸 펼치기/ });
    expect(toggle.getAttribute("aria-expanded")).toBe("false");
    const body = document.getElementById(toggle.getAttribute("aria-controls")!)!;
    expect(body.hidden).toBe(true);
    expect(screen.getByRole("button", { name: "연결 도우미" })).toBeTruthy();
    fireEvent.click(toggle);
    expect(toggle.getAttribute("aria-expanded")).toBe("true");
    expect(body.hidden).toBe(false);
  });

  it("연결된 칸도 키 칸은 닫혀 있고, 오른쪽 끝에 금액 한 줄 · 이름 옆에 확인 시각 · 보유, 세 확인 띠", () => {
    withContext(
      <Slot
        slot={slotCard("upbit", {
          status: "connected",
          accessFilled: true,
          secretFilled: true,
          headline: "✓ 연결됨",
          checks: { auth: "ok", ip: "ok", order: "ok" },
          balance: "사용 가능 잔고 12,300 KRW · 보유 BTC 0.01",
          checkedAt: new Date(2026, 9, 17, 16, 40).getTime(),
        })}
      />,
    );
    expect(screen.getByRole("button", { name: /키 칸 펼치기/ }).getAttribute("aria-expanded")).toBe("false");
    // 금액: 작은 라벨 '잔고' + 숫자 + 단위(전체 이름은 title), '+ BTC'를 붙이지 않음
    const amount = document.querySelector<HTMLElement>(".slot-balance")!;
    expect(amount.title).toBe("사용 가능 잔고");
    expect(amount.textContent).toBe("잔고12,300KRW");
    // 확인 시각·보유 코인은 이름 옆 상태 줄
    const head = document.querySelector<HTMLElement>(".slot-head")!;
    expect(head.textContent).toMatch(/4:40 확인/);
    expect(head.textContent).toContain("보유 BTC 0.01");
    expect(screen.getByRole("list", { name: "키 확인 결과" }).textContent).toContain("주문 권한");
  });

  it("막힌 칸은 빨간 칸의 이유와 이 기기 주소 한 줄, 잔고 자리에 노란 [연결 도우미]", () => {
    withContext(
      <Slot
        slot={slotCard("bithumb", {
          status: "blocked",
          accessFilled: true,
          secretFilled: true,
          headline: "● 허용 IP 확인 필요",
          checks: { auth: "ok", ip: "bad", order: "wait" },
        })}
      />,
    );
    expect(screen.getByText(/허용 IP · 등록된 IP와 다름/)).toBeTruthy();
    expect(screen.getByRole("button", { name: "연결 도우미" }).className).toContain("btn-primary");
  });

  it("고쳐야 할 안내(warn)는 연결됨 칸이어도 주의 색 줄로, 접힌 칸에서도 보임", () => {
    withContext(
      <Slot
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
    expect(screen.getByText("주문할 원화 잔고가 모자라요.").className).toContain("slot-why");
    cleanup();
    withContext(<Slot slot={slotCard("bithumb", { note: "키 저장소를 읽지 못했어요.", noteTone: "warn" })} />);
    expect(screen.getByRole("button", { name: /키 칸 펼치기/ })).toBeTruthy();
    expect(screen.getByText("키 저장소를 읽지 못했어요.").className).toContain("slot-why");
  });
});

describe("금액 나누기", () => {
  it("숫자와 단위를 나눔 — 단위는 작게 그림", () => {
    expect(amountOf("1,234.5 USDT")).toEqual({ num: "1,234.5", unit: "USDT" });
    expect(amountOf("12,300 KRW")).toEqual({ num: "12,300", unit: "KRW" });
    expect(amountOf("확인 못 함")).toEqual({ num: "확인 못 함", unit: "" });
  });
});
