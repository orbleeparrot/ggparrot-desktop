import { cleanup, render, screen, within } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { RunnerContext, type RunnerContextValue } from "../../state/context";
import { binanceMacro, makeSlot, makeState, upbitMacro } from "../../test/fixtures";
import type { RunSlot } from "../../types";
import { DetailHeader } from "./DetailHeader";

afterEach(cleanup);

function renderHead(slot: RunSlot) {
  const value = { state: makeState(), ok: vi.fn() } as unknown as RunnerContextValue;
  return render(
    <RunnerContext.Provider value={value}>
      <DetailHeader slot={slot} />
    </RunnerContext.Provider>,
  );
}

describe("상세 머리줄 — 코인 표시 · 종목", () => {
  it("제목은 기초 자산(BTC) + 모드 배지, 시장 쌍(KRW-BTC)은 없고 거래소는 제목 안 가려진 글자로 읽힘", () => {
    const { container } = renderHead(makeSlot({ macro: upbitMacro, mode: "live" }));
    // 상세 영역의 이름도 이 제목 — "업비트 BTC 실전"
    const title = screen.getByRole("heading", { level: 2, name: /^업비트 BTC/ });
    expect(title.querySelector(".detail-ticker")?.textContent).toBe("BTC");
    expect(title.querySelector(".sr-only")?.textContent?.trim()).toBe("업비트");
    expect(within(title).getByText("실전")).toBeTruthy();
    expect(title.textContent).not.toContain("KRW");
    // 코인 표시는 장식 — 거래소 이름이 두 번 읽히지 않게
    expect(container.querySelector(".detail-head > .coin-mark")).not.toBeNull();
    expect(screen.queryAllByRole("img")).toHaveLength(0);
  });

  it("거래소 · 현물/선물은 아래 칩이 그대로 말함", () => {
    renderHead(makeSlot({ macro: binanceMacro }));
    const chips = screen.getByRole("list", { name: "매크로 정보" });
    expect(within(chips).getByText("바이낸스 선물")).toBeTruthy();
    expect(within(chips).getByText("롱 3배")).toBeTruthy();
    expect(screen.getByRole("heading", { level: 2 }).textContent).not.toContain("USDT");
  });
});
