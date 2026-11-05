import { cleanup, fireEvent, render, within } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { RunnerContext, type RunnerContextValue } from "../../state/context";
import { binanceMacro, makeRun, makeSlot, makeState } from "../../test/fixtures";
import type { RunSlot } from "../../types";
import { IDLE_KEY, IDLE_PEEK, RunList } from "./RunList";

// 여러 개 고르기의 클릭·키 동작. 끌어 고르기(사각형)는 배치가 있어야 해서 e2e/layout.spec.ts에서 확인.

beforeEach(() => window.localStorage.setItem(IDLE_KEY, "open"));
afterEach(() => {
  cleanup();
  window.localStorage.removeItem(IDLE_KEY);
});

const SYMBOLS = ["BTCUSDT", "ETHUSDT", "XRPUSDT", "SOLUSDT", "ADAUSDT"];
const MORE = ["DOTUSDT", "TRXUSDT", "LTCUSDT"];

function slots(symbols: string[], running: string[] = []): RunSlot[] {
  return [
    makeSlot({
      id: "live",
      macro: { ...binanceMacro, symbol: "DOGEUSDT" },
      phase: "running",
      run: makeRun(),
      status: { text: "실행 중", tone: "ok" },
    }),
    ...symbols.map((symbol, i) => {
      const id = `i${i + 1}`;
      return running.includes(id)
        ? makeSlot({ id, macro: { ...binanceMacro, symbol }, phase: "running", run: makeRun() })
        : makeSlot({ id, macro: { ...binanceMacro, symbol } });
    }),
  ];
}

function renderList(symbols = SYMBOLS) {
  const ctx = {
    state: makeState({ runs: slots(symbols) }),
    select: vi.fn(),
    openLibrary: vi.fn(),
    act: vi.fn(),
    askRemove: vi.fn(),
    askRemoveMany: vi.fn(),
    startRun: vi.fn(),
    askStop: vi.fn(),
  } as unknown as RunnerContextValue;
  const view = (value: RunnerContextValue) => (
    <RunnerContext.Provider value={value}>
      <RunList selectedId={null} />
    </RunnerContext.Provider>
  );
  const { container, rerender } = render(view(ctx));
  const row = (id: string) => container.querySelector<HTMLButtonElement>(`.run-card-select[data-run-id="${id}"]`)!;
  const picked = () =>
    Array.from(container.querySelectorAll(".run-card.is-picked .run-card-select")).map(
      (el) => (el as HTMLElement).dataset.runId,
    );
  const head = () => container.querySelector<HTMLElement>(".panel-head")!;
  /** 같은 목록을 새 상태로 다시 그림(Rust가 새 스냅샷을 보낸 때). */
  const update = (runs: RunSlot[]) => rerender(view({ ...ctx, state: makeState({ runs }) }));
  return { ctx, row, picked, head, container, update };
}

describe("내 매크로 — 여러 개 고르기", () => {
  it("Ctrl+클릭으로 넣고 빼고, 그동안 상세는 바뀌지 않음", () => {
    const { ctx, row, picked, head } = renderList();
    fireEvent.click(row("i1"), { ctrlKey: true });
    fireEvent.click(row("i3"), { metaKey: true });
    expect(picked()).toEqual(["i1", "i3"]);
    expect(within(head()).getByRole("heading").textContent).toBe("2개 선택");
    fireEvent.click(row("i1"), { ctrlKey: true });
    expect(picked()).toEqual(["i3"]);
    expect(ctx.select).not.toHaveBeenCalled();
  });

  it("고른 줄은 코인 표시 대신 체크 — 읽는 이름에도 '선택됨'", () => {
    const { row } = renderList();
    fireEvent.click(row("i2"), { ctrlKey: true });
    expect(row("i2").querySelector(".run-card-check")).not.toBeNull();
    expect(row("i2").querySelector(".coin-mark")).toBeNull();
    expect(row("i2").textContent).toContain("선택됨");
    expect(row("i1").querySelector(".coin-mark")).not.toBeNull();
  });

  it("Shift+클릭은 마지막으로 고른 줄부터 여기까지", () => {
    const { row, picked } = renderList();
    fireEvent.click(row("i1"), { ctrlKey: true });
    fireEvent.click(row("i3"), { shiftKey: true });
    expect(picked()).toEqual(["i1", "i2", "i3"]);
  });

  it("실행 중 카드는 Ctrl·Shift로도 안 골라지고 그냥 보여 줌", () => {
    const { ctx, row, picked } = renderList();
    fireEvent.click(row("live"), { ctrlKey: true });
    expect(picked()).toEqual([]);
    expect(ctx.select).toHaveBeenCalledWith("live");
  });

  it("그냥 누르면 고른 것을 비우고 그 매크로를 보여 줌", () => {
    const { ctx, row, picked, head } = renderList();
    fireEvent.click(row("i1"), { ctrlKey: true });
    fireEvent.click(row("i2"));
    expect(picked()).toEqual([]);
    expect(ctx.select).toHaveBeenCalledWith("i2");
    expect(within(head()).getByRole("heading").textContent).toBe("내 매크로");
  });

  it("Ctrl+A는 뺄 수 있는 줄 전부 — 줄여 보이던 대기 묶음도 펼침", () => {
    const { row, picked } = renderList();
    expect(row(`i${IDLE_PEEK + 1}`)).toBeNull();
    fireEvent.keyDown(row("i1"), { key: "a", ctrlKey: true });
    expect(picked()).toEqual(["i1", "i2", "i3", "i4", "i5"]);
  });

  it("Esc는 비움, Delete는 목록 차례대로 빼기를 물음", () => {
    const { ctx, row, picked } = renderList();
    fireEvent.click(row("i3"), { ctrlKey: true });
    fireEvent.click(row("i1"), { ctrlKey: true });
    fireEvent.keyDown(row("i1"), { key: "Delete" });
    expect(ctx.askRemoveMany).toHaveBeenCalledWith(["i1", "i3"]);
    fireEvent.keyDown(row("i1"), { key: "Escape" });
    expect(picked()).toEqual([]);
  });

  it("고르는 동안 머리는 [취소]·[목록에서 빼기] — 새 줄 없이 같은 자리", () => {
    const { ctx, row, picked, head, container } = renderList();
    fireEvent.click(row("i2"), { ctrlKey: true });
    fireEvent.click(row("i1"), { ctrlKey: true });
    expect(container.querySelectorAll(".panel-head").length).toBe(1);
    expect(within(head()).queryByRole("button", { name: "매크로 추가" })).toBeNull();
    fireEvent.click(within(head()).getByRole("button", { name: "목록에서 빼기" }));
    expect(ctx.askRemoveMany).toHaveBeenCalledWith(["i1", "i2"]);
    fireEvent.click(within(head()).getByRole("button", { name: "취소" }));
    expect(picked()).toEqual([]);
    expect(within(head()).getByRole("button", { name: "매크로 추가" })).toBeTruthy();
  });

  it("찾기 칸에서 Ctrl+A는 글자 편집 그대로, 찾는 중 Ctrl+A는 찾은 줄만", () => {
    const { container, picked, row } = renderList([...SYMBOLS, ...MORE]);
    const input = container.querySelector<HTMLInputElement>("input[type=search]")!;
    fireEvent.keyDown(input, { key: "a", ctrlKey: true });
    expect(picked()).toEqual([]);
    fireEvent.change(input, { target: { value: "usdt" } });
    fireEvent.change(input, { target: { value: "ETH" } });
    fireEvent.keyDown(row("i2"), { key: "a", ctrlKey: true });
    expect(picked()).toEqual(["i2"]);
  });
});

describe("고른 것 정리", () => {
  it("새 상태에서 실행이 시작되거나 목록에서 빠진 줄은 고른 것에서 빠짐", () => {
    const { row, picked, head, update } = renderList();
    for (const id of ["i1", "i2", "i3"]) fireEvent.click(row(id), { ctrlKey: true });
    expect(picked()).toEqual(["i1", "i2", "i3"]);
    // i1은 실행 시작, i3은 빠짐
    update(slots(SYMBOLS, ["i1"]).filter((r) => r.id !== "i3"));
    expect(picked()).toEqual(["i2"]);
    expect(within(head()).getByRole("heading").textContent).toBe("1개 선택");
  });
});
