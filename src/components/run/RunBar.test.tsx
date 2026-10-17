import { cleanup, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it } from "vitest";
import { makeEntryWait, makeRun, makeSlot } from "../../test/fixtures";
import { RunBar } from "./RunBar";

afterEach(cleanup);

describe("실행 중 상태 한 줄", () => {
  it("모드 배지 · 경과 · 서버 연결을 한 줄로", () => {
    const { container } = render(
      <RunBar
        slot={makeSlot({ phase: "running", mode: "live", status: { text: "실행 중", tone: "ok" }, run: makeRun() })}
      />,
    );
    expect(screen.getByText("실전으로 실행 중").className).toContain("is-live");
    expect(container.textContent).toMatch(/경과 01:0\d/);
    expect(screen.getByText(/서버 연결됨/)).toBeTruthy();
    expect(container.querySelector(".run-bar-alert")).toBeNull();
  });

  it("저하 상태면 카드 상태 뒷부분을 주의 칩으로, 서버 칸은 끊긴 시간 · 주의 색", () => {
    const { container } = render(
      <RunBar
        slot={makeSlot({
          phase: "running",
          status: { text: "실행 중 · 서버 재시도 중", tone: "warn" },
          run: makeRun({ online: false, offlineSince: Date.now() - 120_000 }),
        })}
      />,
    );
    expect(container.querySelector(".run-bar-alert")?.textContent).toBe("서버 재시도 중");
    expect(screen.getByText("모의로 실행 중").className).not.toContain("is-live");
    expect(screen.getByText(/다시 연결하는 중 · 2분째/)).toBeTruthy();
    expect(container.querySelector(".server-meta")?.className).toContain("tone-warn");
  });

  it("잔고 부족은 목록 카드와 같은 말로", () => {
    const { container } = render(
      <RunBar
        slot={makeSlot({
          phase: "running",
          mode: "live",
          status: { text: "실행 중 · 잔고 부족 · 주문 못 함", tone: "warn" },
          run: makeRun({ fundsShort: true }),
        })}
      />,
    );
    expect(container.querySelector(".run-bar-alert")?.textContent).toBe("잔고 부족 · 주문 못 함");
  });

  it("첫 상태 전에는 서버 확인 중, 진입 대기면 봉 마감까지", () => {
    const { rerender } = render(
      <RunBar
        slot={makeSlot({ phase: "running", status: { text: "진입 대기", tone: "ok" }, run: makeRun({ updatedAt: 0 }) })}
      />,
    );
    expect(screen.getByText(/서버 확인 중/)).toBeTruthy();
    rerender(
      <RunBar
        slot={makeSlot({
          phase: "running",
          status: { text: "진입 대기", tone: "ok" },
          run: makeRun({ entryWait: makeEntryWait({ next_check_ms: Date.now() + 5 * 60_000 + 20_000 }) }),
        })}
      />,
    );
    expect(screen.getByText(/\d\d:\d\d 마감까지 5분/)).toBeTruthy();
    rerender(
      <RunBar
        slot={makeSlot({
          phase: "running",
          status: { text: "진입 대기", tone: "ok" },
          run: makeRun({ entryWait: makeEntryWait({ realtime: true, checks: null, typical: null }) }),
        })}
      />,
    );
    expect(screen.getByText("실시간 확인")).toBeTruthy();
  });

  it("시작 전 확인 · 시작하는 중에는 경과 · 서버 없이 진행 글자만", () => {
    const { container } = render(<RunBar slot={makeSlot({ phase: "checking", mode: "live", run: null })} />);
    expect(screen.getByText("실전으로 시작 전 확인 중")).toBeTruthy();
    expect(container.querySelector(".run-bar-meta")).toBeNull();
  });
});
