import { cleanup, fireEvent, render, within } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { RunnerContext, type RunnerContextValue } from "../../state/context";
import { binanceMacro, makeRun, makeSlot, makeState } from "../../test/fixtures";
import type { RunSlot } from "../../types";
import { IDLE_KEY, IDLE_PEEK, RunList } from "./RunList";

// 선택 모드의 클릭·키 동작. 배치(머리 한 줄·흐린 줄·손잡이)는 e2e/run-picking.spec.ts에서 확인.

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
  const title = () => within(head()).getByRole("heading").textContent;
  /** [선택]으로 선택 모드에 들어감. */
  const enter = () => fireEvent.click(within(head()).getByRole("button", { name: "선택" }));
  /** 같은 목록을 새 상태로 다시 그림(Rust가 새 스냅샷을 보낸 때). */
  const update = (runs: RunSlot[]) => rerender(view({ ...ctx, state: makeState({ runs }) }));
  return { ctx, row, picked, head, title, enter, container, update };
}

describe("내 매크로 — 선택 모드", () => {
  it("평소에는 줄을 누르면 그 매크로를 보여 줌 — 아무것도 고르지 않음", () => {
    const { ctx, row, picked, title } = renderList();
    fireEvent.click(row("i1"));
    expect(ctx.select).toHaveBeenCalledWith("i1");
    expect(picked()).toEqual([]);
    expect(title()).toBe("내 매크로");
  });

  it("[선택] → 줄을 누를 때마다 체크·해제, 상세는 그대로", () => {
    const { ctx, row, picked, title, enter } = renderList();
    enter();
    expect(title()).toBe("0개 선택");
    fireEvent.click(row("i1"));
    fireEvent.click(row("i3"));
    expect(picked()).toEqual(["i1", "i3"]);
    expect(title()).toBe("2개 선택");
    fireEvent.click(row("i1"));
    expect(picked()).toEqual(["i3"]);
    expect(ctx.select).not.toHaveBeenCalled();
    expect(row("i3").getAttribute("aria-pressed")).toBe("true");
    expect(row("i2").getAttribute("aria-pressed")).toBe("false");
  });

  it("선택 모드에서 고를 수 있는 줄은 빈 원, 고른 줄은 체크 · 읽는 이름에 '선택됨'", () => {
    const { row, enter } = renderList();
    expect(row("i1").querySelector(".run-card-ring")).toBeNull();
    enter();
    expect(row("i1").querySelector(".run-card-ring")).not.toBeNull();
    expect(row("i1").querySelector(".coin-mark")).toBeNull();
    fireEvent.click(row("i1"));
    expect(row("i1").querySelector(".run-card-check")).not.toBeNull();
    expect(row("i1").textContent).toContain("선택됨");
  });

  it("실행 중 카드는 흐리고 눌러도 안 골라지며 상세도 안 바뀜", () => {
    const { ctx, row, picked, enter } = renderList();
    enter();
    const card = row("live").closest(".run-card")!;
    expect(card.classList.contains("is-pickable")).toBe(false);
    expect(row("live").querySelector(".coin-mark")).not.toBeNull();
    fireEvent.click(row("live"));
    expect(picked()).toEqual([]);
    expect(ctx.select).not.toHaveBeenCalled();
  });

  it("Shift+클릭은 마지막으로 누른 줄부터 여기까지", () => {
    const { row, picked, enter } = renderList();
    enter();
    fireEvent.click(row("i1"));
    fireEvent.click(row("i4"), { shiftKey: true });
    expect(picked()).toEqual(["i1", "i2", "i3", "i4"]);
  });

  it("들어가면 줄여 보이던 대기 묶음을 다 펼침", () => {
    const { row, enter } = renderList();
    expect(row(`i${IDLE_PEEK + 1}`)).toBeNull();
    enter();
    expect(row(`i${IDLE_PEEK + 1}`)).not.toBeNull();
  });

  it("머리 동그라미는 전체 선택·해제 — Ctrl+A도 전체", () => {
    const { head, row, picked, enter } = renderList();
    enter();
    const all = () => within(head()).getByRole("button", { name: /개 선택/ });
    fireEvent.click(all());
    expect(picked()).toEqual(["i1", "i2", "i3", "i4", "i5"]);
    expect(all().getAttribute("aria-pressed")).toBe("true");
    fireEvent.click(all());
    expect(picked()).toEqual([]);
    fireEvent.keyDown(row("i1"), { key: "a", ctrlKey: true });
    expect(picked()).toEqual(["i1", "i2", "i3", "i4", "i5"]);
  });

  it("[목록에서 빼기]는 하나도 안 골랐으면 눌리지 않고, 고르면 목록 차례대로 묻는다 — Delete도 같음", () => {
    const { ctx, head, row, enter } = renderList();
    enter();
    const remove = () => within(head()).getByRole("button", { name: "목록에서 빼기" });
    expect((remove() as HTMLButtonElement).disabled).toBe(true);
    fireEvent.click(row("i3"));
    fireEvent.click(row("i1"));
    fireEvent.click(remove());
    expect(ctx.askRemoveMany).toHaveBeenCalledWith(["i1", "i3"]);
    fireEvent.keyDown(row("i1"), { key: "Delete" });
    expect(ctx.askRemoveMany).toHaveBeenCalledTimes(2);
  });

  it("[취소]·Esc는 나가기 — 고른 것을 비우고 머리가 돌아옴", () => {
    const { head, row, picked, title, enter } = renderList();
    enter();
    fireEvent.click(row("i1"));
    fireEvent.click(within(head()).getByRole("button", { name: "취소" }));
    expect(picked()).toEqual([]);
    expect(title()).toBe("내 매크로");
    enter();
    fireEvent.click(row("i2"));
    fireEvent.keyDown(row("i2"), { key: "Escape" });
    expect(picked()).toEqual([]);
    expect(title()).toBe("내 매크로");
  });

  it("찾는 중 전체 선택은 찾은 줄만, 찾기 칸의 Ctrl+A는 글자 편집 그대로", () => {
    const { container, picked, row, enter } = renderList([...SYMBOLS, ...MORE]);
    enter();
    const input = container.querySelector<HTMLInputElement>("input[type=search]")!;
    fireEvent.keyDown(input, { key: "a", ctrlKey: true });
    expect(picked()).toEqual([]);
    fireEvent.change(input, { target: { value: "ETH" } });
    fireEvent.keyDown(row("i2"), { key: "a", ctrlKey: true });
    expect(picked()).toEqual(["i2"]);
  });
});

describe("고른 것 정리", () => {
  it("실행이 시작되거나 빠진 줄은 고른 것에서 빠지고, 다 빠지면(빼기 끝) 선택 모드도 끝남", () => {
    const { row, picked, title, enter, update } = renderList();
    enter();
    for (const id of ["i1", "i2", "i3"]) fireEvent.click(row(id));
    // i1은 실행 시작, i3은 빠짐
    update(slots(SYMBOLS, ["i1"]).filter((r) => r.id !== "i3"));
    expect(picked()).toEqual(["i2"]);
    expect(title()).toBe("1개 선택");
    // i2도 빠짐 — 빼기가 끝난 것
    update(slots(SYMBOLS, ["i1"]).filter((r) => r.id !== "i3" && r.id !== "i2"));
    expect(title()).toBe("내 매크로");
  });

  it("고를 줄이 하나도 없으면 [선택]을 두지 않음", () => {
    const { head, update } = renderList();
    update(slots([]));
    expect(within(head()).queryByRole("button", { name: "선택" })).toBeNull();
  });
});
