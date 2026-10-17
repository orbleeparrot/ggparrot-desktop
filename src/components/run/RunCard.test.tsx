import { cleanup, fireEvent, render, screen, within } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { RunnerContext, type RunnerContextValue } from "../../state/context";
import { MODE_LABELS } from "../../state/runs";
import { makeEntryWait, makeRun, makeSlot, makeState } from "../../test/fixtures";
import type { RunSlot } from "../../types";
import { RunCard } from "./RunCard";

afterEach(cleanup);

function renderCard(slot: RunSlot) {
  const value = {
    state: makeState(),
    select: vi.fn(),
    askRemove: vi.fn(),
    startRun: vi.fn(),
    askStop: vi.fn(),
  } as unknown as RunnerContextValue;
  const { container } = render(
    <RunnerContext.Provider value={value}>
      <ul>
        <RunCard slot={slot} selected onKeyNav={vi.fn()} />
      </ul>
    </RunnerContext.Provider>,
  );
  return { ctx: value, card: container.querySelector("li")! };
}

const symbol = makeSlot().macro.symbol;

describe("내 매크로 카드(두 줄)", () => {
  it("대기 카드: × 는 고르기 버튼 밖의 단추 — 누르면 빼기만 묻고 카드는 고르지 않음", () => {
    const { ctx, card } = renderCard(makeSlot());
    const remove = screen.getByRole("button", { name: `${symbol} 목록에서 빼기` });
    const select = card.querySelector<HTMLButtonElement>(".run-card-select")!;
    expect(select.contains(remove)).toBe(false);
    fireEvent.click(remove);
    expect(ctx.askRemove).toHaveBeenCalledWith("run-1");
    expect(ctx.select).not.toHaveBeenCalled();
    // × 자리는 가리키지 않아도 늘 비워 둠
    expect(select.querySelector(".run-card-x-slot")).not.toBeNull();
  });

  it("실행 중 카드: 오른쪽 칸은 손익 %, × 는 없고 주 버튼은 청산(모의 보유 중)", () => {
    const run = makeRun({ inPosition: true, entryPrice: 100, qty: 1, unrealizedPct: 1.37 });
    const { card } = renderCard(makeSlot({ phase: "running", run, status: { text: "실행 중", tone: "ok" } }));
    expect(screen.queryByRole("button", { name: /목록에서 빼기/ })).toBeNull();
    expect(card.querySelector(".run-card-pct")?.className).toContain("tone-up");
    expect(card.querySelector(".run-card-x-slot")).toBeNull();
    expect(screen.getByRole("button", { name: `${symbol} 청산` })).toBeTruthy();
  });

  it("상태 문구는 앞·뒷부분으로 나눠 두고(좁으면 뒷부분을 숨김) 전체는 title 로", () => {
    const run = makeRun({ online: false });
    const status = { text: "실행 중 · 서버 재시도 중", tone: "warn" as const };
    const { card } = renderCard(makeSlot({ phase: "running", run, status }));
    const head = card.querySelector(".run-card-status")!;
    const rest = card.querySelector(".run-card-status-rest")!;
    expect(head.getAttribute("title")).toBe(status.text);
    expect(rest.getAttribute("title")).toBe(status.text);
    expect(head.textContent).toBe("실행 중");
    expect(rest.textContent).toBe("서버 재시도 중");
  });

  it("잔고 부족 상태는 주의 색 한 줄 — 앞부분 '실행 중', 뒷부분 '잔고 부족 · 주문 못 함'", () => {
    const run = makeRun({ fundsShort: true });
    const status = { text: "실행 중 · 잔고 부족 · 주문 못 함", tone: "warn" as const };
    const { card } = renderCard(makeSlot({ phase: "running", run, status }));
    const head = card.querySelector(".run-card-status")!;
    const rest = card.querySelector(".run-card-status-rest")!;
    expect(head.className).toContain("tone-warn");
    expect(head.textContent).toBe("실행 중");
    expect(rest.textContent).toBe("잔고 부족 · 주문 못 함");
  });

  it("전략은 자리(바깥)와 글자(안쪽, 구분점 포함)로 나눠 둠 — 좁으면 구분점과 함께 통째로 숨음", () => {
    const slot = makeSlot();
    const { card } = renderCard(slot);
    const strategy = card.querySelector(".run-card-strategy")!;
    expect(strategy.querySelector(".run-card-strategy-text")?.textContent).toBe(
      slot.macro.strategy || slot.macro.title,
    );
  });

  it("전략은 줄임표로 잘려도 title 에 전략과 매크로 이름이 다 있음", () => {
    const slot = makeSlot();
    const { card } = renderCard(slot);
    const strategy = card.querySelector(".run-card-strategy")!;
    expect(strategy.getAttribute("title")).toContain(slot.macro.strategy);
    expect(strategy.getAttribute("title")).toContain(slot.macro.title);
  });

  it("모드 배지는 점으로 줄어도 title 과 글자로 모드 이름이 남음", () => {
    const { card } = renderCard(makeSlot({ mode: "testnet" }));
    const mode = card.querySelector(".run-card-mode")!;
    expect(mode.getAttribute("title")).toBe(MODE_LABELS.testnet);
    expect(within(card.querySelector<HTMLElement>(".run-card-select")!).getByText(MODE_LABELS.testnet)).toBeTruthy();
    expect(mode.querySelector(".mode-dot")?.getAttribute("aria-hidden")).toBe("true");
  });

  it("종료 처리 중이면 주 버튼은 꺼진 진행 표시(이름에 진행 문구)", () => {
    const run = makeRun();
    const { card } = renderCard(makeSlot({ phase: "stopping", run, status: { text: "종료 처리 중", tone: "warn" } }));
    const busy = within(card.querySelector<HTMLElement>(".run-card-action")!).getByRole("button");
    expect(busy).toHaveProperty("disabled", true);
    expect(busy.getAttribute("aria-label")).toMatch(new RegExp(`^${symbol} `));
    // 시작·종료 중에는 오른쪽 칸을 비움
    expect(card.querySelector(".run-card-pct, .run-card-flat, .run-card-x-slot")).toBeNull();
  });
});

describe("내 매크로 카드 — 진입 대기", () => {
  it("안 샀으면 '무포지션' 대신 사는 선까지 남은 거리, 가리키면 조건 문장", () => {
    const run = makeRun({ entryWait: makeEntryWait() });
    const { card } = renderCard(makeSlot({ phase: "running", run, status: { text: "진입 대기", tone: "ok" } }));
    const gap = card.querySelector(".run-card-wait")!;
    expect(gap.textContent).toBe("+1.0%");
    expect(gap.getAttribute("title")).toContain("가격이 102.00 위로 올라가면 사요.");
    expect(within(card).queryByText("무포지션")).toBeNull();
  });

  it("짧은 거리가 없으면(옛 서버) '진입 대기'", () => {
    const run = makeRun({ entryWait: makeEntryWait({ gap: "" }) });
    const { card } = renderCard(makeSlot({ phase: "running", run, status: { text: "진입 대기", tone: "ok" } }));
    expect(card.querySelector(".run-card-wait")!.textContent).toBe("진입 대기");
  });
});
