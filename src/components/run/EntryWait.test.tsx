import { cleanup, render } from "@testing-library/react";
import { afterEach, describe, expect, it } from "vitest";
import { makeEntryWait } from "../../test/fixtures";
import { closeChipText, CloseChip, intervalMs } from "./EntryWait";

afterEach(cleanup);

describe("봉 마감 칩", () => {
  it("봉 간격을 ms로 — 모르는 꼴이면 null", () => {
    expect(intervalMs("15m")).toBe(900_000);
    expect(intervalMs("4h")).toBe(14_400_000);
    expect(intervalMs("1d")).toBe(86_400_000);
    expect(intervalMs("1M")).toBeNull();
  });

  it("판정 시각 · 남은 시간, 1분 안이면 '곧', 지정가는 '실시간 확인', 시각을 모르면 없음", () => {
    const now = new Date(2026, 9, 18, 3, 3, 0).getTime();
    const at = new Date(2026, 9, 18, 3, 15, 0).getTime();
    expect(closeChipText(makeEntryWait({ next_check_ms: at }), now)).toEqual({ at: "03:15 판정", left: "12분 남음" });
    expect(closeChipText(makeEntryWait({ next_check_ms: at }), at - 30_000)).toEqual({ at: "03:15 판정", left: "곧" });
    expect(closeChipText(makeEntryWait({ realtime: true }), now)).toEqual({ at: "실시간 확인", left: "" });
    expect(closeChipText(makeEntryWait({ next_check_ms: null }), now)).toBeNull();
  });

  it("막대는 이번 봉이 지난 만큼 — 1시간 봉에서 15분 남았으면 75%", () => {
    const { container } = render(
      <CloseChip wait={makeEntryWait({ interval: "1h", next_check_ms: Date.now() + 15 * 60_000 + 500 })} />,
    );
    const fill = container.querySelector<HTMLElement>(".ew-close-bar i")!;
    expect(parseFloat(fill.style.width)).toBeCloseTo(75, 0);
    expect(container.textContent).toMatch(/^\d\d:\d\d 판정· 15분 남음$/);
  });

  it("지정가(실시간)는 막대 없이 글자만", () => {
    const { container } = render(<CloseChip wait={makeEntryWait({ realtime: true })} />);
    expect(container.querySelector(".ew-close-bar")).toBeNull();
    expect(container.textContent).toBe("실시간 확인");
  });
});
