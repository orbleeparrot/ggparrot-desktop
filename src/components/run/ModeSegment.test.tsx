import { cleanup, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { RunnerContext, type RunnerContextValue } from "../../state/context";
import { makeRun, makeSlot } from "../../test/fixtures";
import type { RunSlot } from "../../types";
import { ModeSegment } from "./ModeSegment";

afterEach(cleanup);

function renderSegment(slot: RunSlot) {
  const value = { act: vi.fn().mockResolvedValue(undefined) } as unknown as RunnerContextValue;
  render(
    <RunnerContext.Provider value={value}>
      <ModeSegment slot={slot} />
    </RunnerContext.Provider>,
  );
}

/** aria-describedby가 가리키는 글자. */
function description(el: HTMLElement): string {
  const ids = el.getAttribute("aria-describedby")?.split(" ") ?? [];
  return ids.map((id) => document.getElementById(id)?.textContent ?? "").join(" ");
}

describe("모드 세그먼트", () => {
  it("단추마다 설명이 있음 — 풍선 도움말(title)과 접근성 설명이 같고, 모드마다 다름", () => {
    renderSegment(makeSlot());
    const radios = screen.getAllByRole("radio");
    expect(radios).toHaveLength(3);
    const texts = radios.map((r) => description(r));
    for (const [i, r] of radios.entries()) {
      expect(texts[i].length).toBeGreaterThan(0);
      expect(r.getAttribute("title")).toBe(texts[i]);
    }
    expect(new Set(texts).size).toBe(3);
  });

  it("실행 중에는 잠기고 단추 풍선 도움말 대신 잠긴 이유를 보임(설명은 그대로)", () => {
    renderSegment(makeSlot({ phase: "running", run: makeRun() }));
    const group = screen.getByRole("radiogroup");
    expect(group.getAttribute("aria-disabled")).toBe("true");
    expect(group.getAttribute("title")).toBeTruthy();
    for (const r of screen.getAllByRole("radio")) {
      expect(r.getAttribute("title")).toBeNull();
      expect(description(r).length).toBeGreaterThan(0);
    }
  });
});
