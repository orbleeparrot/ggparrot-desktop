import { cleanup, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it } from "vitest";
import { act as activity, binanceMacro, makeRun } from "../../test/fixtures";
import type { Activity } from "../../types";
import { FinishedCard, tradeCounts } from "./FinishedCard";

afterEach(cleanup);

const spot = { ...binanceMacro, market: "spot" as const, leverage: 1, capital: 500 };

function act(id: number, kind: Activity["kind"], at: number): Activity {
  return activity({ id, kind, at, title: kind });
}

describe("이번 실행 결과", () => {
  it("실현손익(수수료 반영) 큰 숫자 · 투자 금액 대비 % · 거래 · 낸 수수료 · 실행 시간 · 마지막 가격", () => {
    const start = Date.now() - 14 * 60_000 - 4_000;
    const run = makeRun({
      startedAt: start,
      realized: -0.4,
      realizedNet: -0.55,
      fees: 0.4,
      price: 81815.07,
      finished: { status: "stopped", note: "청산 완료 후 종료", at: start + 14 * 60_000 + 4_000 },
    });
    const items = [
      act(1, "start", start),
      act(2, "buy", start + 1),
      act(3, "sell", start + 2),
      act(4, "buy", start + 3),
      act(5, "sell", start + 4),
    ];
    const { container } = render(<FinishedCard run={run} macro={spot} mode="mock" activity={items} />);
    expect(container.querySelector(".finished-chip")?.textContent).toBe("종료됨");
    expect(screen.getByText("청산 완료 후 종료")).toBeTruthy();
    expect(screen.getByText("이번 실행 실현손익 · 모의")).toBeTruthy();
    expect(container.querySelector(".pnl-pct")?.textContent).toBe("-0.55 USDT");
    expect(container.querySelector(".pnl-amount")?.textContent).toBe("투자 금액 대비 -0.11%");
    const cells = [...container.querySelectorAll(".finished-stats dd")].map((d) => d.textContent);
    expect(cells).toEqual(["4번진입 2 · 정리 2", "0.40 USDT", "14:04", "81,815.07"]);
    expect(container.querySelector(".pnl-tag")).toBeNull();
  });

  it("수수료를 모르면 서버 값에 '수수료 제외 전', 오류로 멈춤은 빨간 칩", () => {
    const run = makeRun({
      realized: 5,
      realizedNet: null,
      fees: null,
      finished: { status: "error", note: "", at: Date.now() },
    });
    const { container } = render(<FinishedCard run={run} macro={binanceMacro} mode="live" activity={[]} />);
    expect(container.querySelector(".finished-chip")?.textContent).toBe("오류로 멈춤");
    expect(container.querySelector(".finished-chip")?.className).toContain("tone-danger");
    expect(container.querySelector(".pnl-tag")?.textContent).toBe("수수료 제외 전");
    // 이번 시작 줄이 기록에 없으면 거래 수를 지어 보이지 않음.
    expect(container.querySelector(".finished-stats dd")?.textContent).toBe("—");
    expect(screen.getByText("알 수 없음(선물)")).toBeTruthy();
  });

  it("거래 수는 이번 시작 줄부터 — 지난 실행의 체결은 세지 않음", () => {
    const now = Date.now();
    const items = [
      act(1, "start", now - 90_000),
      act(2, "buy", now - 80_000),
      act(3, "start", now - 1_000),
      act(4, "buy", now),
    ];
    expect(tradeCounts(items, now - 1_000)).toEqual({ buys: 1, sells: 0 });
    expect(tradeCounts(items.slice(3), now - 1_000)).toBeNull();
  });
});
