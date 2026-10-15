import { cleanup, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it } from "vitest";
import { binanceMacro, makeRun } from "../../test/fixtures";
import { StatusCard } from "./StatusCard";

afterEach(cleanup);

const ok = { text: "실행 중", tone: "ok" as const };

describe("실행 상태 카드", () => {
  it("목록 카드와 같은 상태 문구를 쓰고, 저하 상태면 점이 깜박이지 않는다", () => {
    const { container } = render(
      <StatusCard
        run={makeRun({ online: false, offlineSince: Date.now() - 120_000 })}
        macro={binanceMacro}
        phase="running"
        status={{ text: "실행 중 · 서버 재시도 중", tone: "warn" }}
        prices={[]}
      />,
    );
    expect(screen.getByText("실행 중 · 서버 재시도 중")).toBeTruthy();
    expect(container.querySelector(".status-dot.is-live")).toBeNull();
    expect(screen.getByText(/다시 연결하는 중 · 2분째/)).toBeTruthy();
  });

  it("포지션을 든 채 시세가 끊기면 손절을 못 본다고 띠로 알린다", () => {
    render(
      <StatusCard
        run={makeRun({ inPosition: true, qty: 1, entryPrice: 100, priceStaleSince: Date.now() - 5_000 })}
        macro={binanceMacro}
        phase="running"
        status={{ text: "실행 중 · 시세 끊김 · 손절 확인 멈춤", tone: "danger" }}
        prices={[]}
      />,
    );
    expect(screen.getByRole("alert").textContent).toContain("손절 · 안전 규칙을 확인하지 못하고 있어요");
    expect(screen.getByText(/마지막 확인 가격/)).toBeTruthy();
  });

  it("첫 상태를 받기 전에는 현재가 0 대신 —, 서버는 확인 중", () => {
    render(<StatusCard run={makeRun({ updatedAt: 0 })} macro={binanceMacro} phase="running" status={ok} prices={[]} />);
    expect(screen.getByText("확인 중")).toBeTruthy();
    expect(screen.getByText("현재가").nextElementSibling?.textContent).toBe("—");
  });

  it("상태가 20초 넘게 오지 않으면 언제 갱신됐는지 보인다", () => {
    render(
      <StatusCard
        run={makeRun({ updatedAt: Date.now() - 45_000 })}
        macro={binanceMacro}
        phase="running"
        status={ok}
        prices={[]}
      />,
    );
    expect(screen.getByText(/45초 전 갱신/)).toBeTruthy();
  });
});
