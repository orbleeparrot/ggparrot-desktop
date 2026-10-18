import { cleanup, render } from "@testing-library/react";
import { afterEach, describe, expect, it } from "vitest";
import { makeRun, makeSlot } from "../../test/fixtures";
import { RunState } from "./RunState";

afterEach(cleanup);

describe("상세 머리 상태", () => {
  it("잘 돌면 '실행 중' · 경과만 — 모드 · 서버 연결됨은 말하지 않음", () => {
    const { container } = render(
      <RunState
        slot={makeSlot({ phase: "running", mode: "live", status: { text: "실행 중", tone: "ok" }, run: makeRun() })}
      />,
    );
    const el = container.querySelector(".run-state")!;
    expect(el.className).toContain("tone-ok");
    expect(el.textContent).toMatch(/^실행 중경과 01:0\d$/);
    expect(el.querySelector(".run-state-dot")?.className).toContain("is-live");
    expect(container.textContent).not.toMatch(/서버|실전/);
  });

  it("서버가 끊기면 경과 대신 그 자리에 끊긴 시간(주의 색) — '서버 재시도 중'은 겹쳐 말하지 않음", () => {
    const { container } = render(
      <RunState
        slot={makeSlot({
          phase: "running",
          status: { text: "실행 중 · 서버 재시도 중", tone: "warn" },
          run: makeRun({ online: false, offlineSince: Date.now() - 120_000 }),
        })}
      />,
    );
    const el = container.querySelector(".run-state")!;
    expect(el.textContent).toBe("서버 다시 연결하는 중 · 2분째");
    expect(el.className).toContain("tone-warn");
    expect(el.querySelector(".is-live")).toBeNull();
  });

  it("잔고 부족은 목록 카드와 같은 말로, 회원 키 거절은 빨강", () => {
    const { container, rerender } = render(
      <RunState
        slot={makeSlot({
          phase: "running",
          mode: "live",
          status: { text: "실행 중 · 잔고 부족 · 주문 못 함", tone: "warn" },
          run: makeRun({ fundsShort: true }),
        })}
      />,
    );
    expect(container.querySelector(".run-state")?.textContent).toBe("잔고 부족 · 주문 못 함");
    rerender(
      <RunState
        slot={makeSlot({
          phase: "running",
          status: { text: "실행 중 · 회원 키 거절", tone: "danger" },
          run: makeRun({ authFailed: true, online: false }),
        })}
      />,
    );
    expect(container.querySelector(".run-state")?.className).toContain("tone-danger");
    expect(container.querySelector(".run-state")?.textContent).toContain("서버 회원 키 거절됨");
  });

  it("첫 상태 전(서버 확인 중)에는 서버를 말하지 않음", () => {
    const { container } = render(
      <RunState
        slot={makeSlot({ phase: "running", status: { text: "진입 대기", tone: "ok" }, run: makeRun({ updatedAt: 0 }) })}
      />,
    );
    expect(container.textContent).not.toContain("서버");
  });

  it("시작 전 확인 · 시작하는 중에는 경과 없이 진행 글자만", () => {
    const { container } = render(<RunState slot={makeSlot({ phase: "checking", mode: "live", run: null })} />);
    expect(container.querySelector(".run-state")?.textContent).toBe("시작 전 확인 중");
    expect(container.querySelector(".run-state")?.className).toContain("tone-muted");
    expect(container.querySelector(".run-state-elapsed")).toBeNull();
  });
});
