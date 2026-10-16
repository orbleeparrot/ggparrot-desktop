import { cleanup, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { RunnerContext, type RunnerContextValue } from "../../state/context";
import { binanceMacro, makeEntryWait, makeRun, makeSlot, makeState, upbitMacro } from "../../test/fixtures";
import { RunCard } from "./RunCard";
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
    // 서버 칸의 점 색은 상태를 따름 — 다시 연결 중이면 주의 색.
    expect(container.querySelector(".server-meta")?.className).toContain("tone-warn");
    expect(container.querySelector(".server-meta .server-dot")).toBeTruthy();
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

  it("잔고 부족이면 할 일을 주의 색 띠로 알린다(코드는 띠에 없음)", () => {
    render(
      <StatusCard
        run={makeRun({ fundsShort: true })}
        macro={upbitMacro}
        phase="running"
        status={{ text: "실행 중 · 잔고 부족 · 주문 못 함", tone: "warn" }}
        prices={[]}
      />,
    );
    const banner = screen.getByRole("alert");
    expect(banner.textContent).toBe(
      "업비트 원화 잔고가 모자라 주문하지 못했어요. 잔고를 채우거나 투자 금액을 줄이세요. 다음 신호에서 다시 주문해요.",
    );
    expect(banner.className).toContain("is-warn");
    expect(screen.getByText("실행 중 · 잔고 부족 · 주문 못 함")).toBeTruthy();
  });

  it("첫 상태를 받기 전에는 현재가 0 대신 —, 서버는 확인 중", () => {
    render(<StatusCard run={makeRun({ updatedAt: 0 })} macro={binanceMacro} phase="running" status={ok} prices={[]} />);
    expect(screen.getByText(/서버 확인 중/)).toBeTruthy();
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

  it("실현손익은 수수료 반영 값(Rust realizedNet), 모르면 서버 값에 '수수료 제외 전'", () => {
    const spot = { ...binanceMacro, market: "spot" as const, leverage: 1 };
    const cell = () => screen.getByText("이번 실행 실현손익").nextElementSibling;
    const { rerender } = render(
      <StatusCard
        run={makeRun({ realized: 5, realizedNet: 4.895, fees: 0.105 })}
        macro={spot}
        phase="running"
        status={ok}
        prices={[]}
      />,
    );
    expect(cell()?.textContent).toBe("+4.90 USDT");
    expect(screen.getByText("낸 수수료").nextElementSibling?.textContent).toBe("0.11 USDT");
    expect(screen.getByText(/웹 기록은 수수료 전 금액이에요/).textContent).toBe(
      "손익은 수수료를 반영했어요 · 웹 기록은 수수료 전 금액이에요",
    );
    rerender(
      <StatusCard
        run={makeRun({ realized: 5, realizedNet: null, fees: null })}
        macro={binanceMacro}
        phase="running"
        status={ok}
        prices={[]}
      />,
    );
    expect(cell()?.textContent).toBe("+5.00 USDT수수료 제외 전");
    expect(screen.getByText("낸 수수료").nextElementSibling?.textContent).toBe("알 수 없음(선물)");
    rerender(
      <StatusCard
        run={makeRun({ realized: 0, realizedNet: null, fees: null })}
        macro={binanceMacro}
        phase="running"
        status={ok}
        prices={[]}
      />,
    );
    expect(cell()?.textContent).toBe("0.00 USDT");
  });

  it("반올림해 0이면 부호 없이 흐린 색 — -0.004 USDT는 0.00 USDT", () => {
    render(
      <StatusCard
        run={makeRun({ realized: -0.004, realizedNet: -0.004, fees: 0 })}
        macro={binanceMacro}
        phase="running"
        status={ok}
        prices={[]}
      />,
    );
    const cell = screen.getByText("이번 실행 실현손익").nextElementSibling!;
    expect(cell.textContent).toBe("0.00 USDT");
    expect(cell.className).toContain("tone-flat");
  });

  it("맨 윗줄에 상태·경과 시간·서버 연결, 표에는 투입금·보유 수량·평단·현재가·낸 수수료·실현손익", () => {
    const { container } = render(
      <StatusCard
        run={makeRun({ inPosition: true, qty: 10, entryPrice: 100, price: 100.15, invested: 1001, fees: 1 })}
        macro={{ ...binanceMacro, market: "spot", leverage: 1 }}
        phase="running"
        status={ok}
        prices={[]}
      />,
    );
    const head = container.querySelector(".status-head")!;
    expect(head.textContent).toContain("실행 중");
    expect(head.textContent).toMatch(/경과 01:0\d/);
    expect(head.textContent).toContain("서버 연결됨");
    const terms = [...container.querySelectorAll(".run-table dt")].map((d) => d.textContent);
    expect(terms).toEqual(["현재가", "평단", "투입금", "보유 수량", "낸 수수료", "이번 실행 실현손익"]);
    expect(screen.getByText("투입금").nextElementSibling?.textContent).toBe("1,001 USDT");
    expect(screen.getByText("보유 수량").nextElementSibling?.textContent).toBe("10 BTC");
  });

  it("보유가 없으면 투입금·보유 수량은 '없음', 청산 기준은 요약 한 줄", () => {
    render(<StatusCard run={makeRun()} macro={binanceMacro} phase="running" status={ok} prices={[]} />);
    expect(screen.getByText("투입금").nextElementSibling?.textContent).toBe("없음");
    expect(screen.getByText("보유 수량").nextElementSibling?.textContent).toBe("없음");
    expect(screen.getByText(/청산 기준/).closest("p")?.textContent).toBe(
      "청산 기준 익절 +5% · 손절 -3% (평단 대비 가격)",
    );
  });

  it("수수료 반영 평가손익 — 바이낸스 현물 +0.15%는 지금 정리하면 -0.50 USDT, '투입금 대비'", () => {
    const { container } = render(
      <StatusCard
        run={makeRun({
          inPosition: true,
          qty: 10,
          entryPrice: 100,
          price: 100.15,
          unrealized: -0.5015,
          unrealizedPct: -0.0501,
          unrealizedFeeAware: true,
          invested: 1001,
          fees: 1,
        })}
        macro={{ ...binanceMacro, market: "spot", leverage: 1 }}
        phase="running"
        status={ok}
        prices={[]}
      />,
    );
    expect(screen.getByText("평가손익 · 지금 정리하면")).toBeTruthy();
    expect(container.querySelector(".pnl-amount")?.textContent).toBe("-0.50 USDT");
    expect(container.querySelector(".pnl-amount")?.className).toContain("tone-down");
    expect(container.querySelector(".pnl-pct")?.textContent).toBe("-0.05%");
    expect(container.querySelector(".pnl-basis")?.textContent).toBe("투입금 대비");
    expect(container.querySelector(".pnl-margin")).toBeNull();
  });

  it("선물은 증거금 대비 %(레버리지)를 한 줄 더, 수수료를 모르면 평단 대비 · 수수료 제외 전", () => {
    const { container } = render(
      <StatusCard
        run={makeRun({
          inPosition: true,
          qty: 1,
          entryPrice: 100,
          price: 101.5,
          unrealized: 1.5,
          unrealizedPct: 1.5,
          unrealizedFeeAware: false,
          invested: 100,
        })}
        macro={binanceMacro}
        phase="running"
        status={ok}
        prices={[]}
      />,
    );
    expect(container.querySelector(".pnl-basis")?.textContent).toBe("평단 대비 · 수수료 제외 전");
    expect(container.querySelector(".pnl-margin")?.textContent).toBe("증거금 대비 +4.50% (3배)");
  });

  it("보유 중이면 청산 기준 막대 — 손절가·평단·현재가·익절가", () => {
    const { container } = render(
      <StatusCard
        run={makeRun({ inPosition: true, qty: 1, entryPrice: 100, price: 102 })}
        macro={{ ...binanceMacro, market: "spot", leverage: 1 }}
        phase="running"
        status={ok}
        prices={[]}
      />,
    );
    expect(container.querySelector(".exit-head")?.textContent).toBe("청산 기준 · 평단 대비 가격");
    const ends = [...container.querySelectorAll(".exit-end")].map((e) => e.textContent);
    expect(ends).toEqual(["손절 -3%97.00", "익절 +5%105.00"]);
    // 평단은 막대 아래 작은 눈금 이름(가격은 풍선 도움말), 현재가 꼬리표는 평단 대비 %와 함께.
    const entry = container.querySelector(".exit-entry") as HTMLElement;
    expect(entry.textContent).toBe("평단");
    expect(entry.title).toBe("평단 100.00 USDT");
    expect(container.querySelector(".exit-tag.is-price")?.textContent).toBe("현재 102.00 · 평단 대비 +2.00%");
    expect(container.querySelector(".exit-dist")?.textContent).toBe("손절까지 -4.90% · 익절까지 +2.94%");
    expect(parseFloat((container.querySelector(".exit-dot") as HTMLElement).style.left)).toBeCloseTo(62.5, 9);
    // 평단 왼쪽은 손실 쪽, 오른쪽은 이익 쪽 — 막대 바탕의 경계가 평단 위치.
    expect((container.querySelector(".exit-track") as HTMLElement).style.getPropertyValue("--entry")).toBe("37.5%");
  });

  it("보유 수량은 받은 자릿수 그대로(줄이지 않음), 풍선 도움말에도 같은 값", () => {
    render(
      <StatusCard
        run={makeRun({ inPosition: true, qty: 499.75012494, entryPrice: 200, price: 201 })}
        macro={{ ...upbitMacro, baseAsset: "ZAMA" }}
        phase="running"
        status={ok}
        prices={[]}
      />,
    );
    const cell = screen.getByText("보유 수량").nextElementSibling as HTMLElement;
    expect(cell.textContent).toBe("499.75012494 ZAMA");
    expect(cell.title).toBe("499.75012494 ZAMA");
  });

  it("그래프 아래 덮는 시간 — 실행 시작부터, 앱을 실행 뒤에 열었으면 앱을 연 뒤부터", () => {
    const now = Date.now();
    const start = now - 80 * 60_000;
    const run = makeRun({ startedAt: start, updatedAt: now, inPosition: false });
    const { container, rerender } = render(
      <StatusCard
        run={run}
        macro={binanceMacro}
        phase="running"
        status={ok}
        prices={[100, 101]}
        priceTimes={[start + 5_000, now]}
      />,
    );
    expect(container.querySelector(".chart-span")?.textContent).toBe("실행 시작부터 · 1시간 20분");
    rerender(
      <StatusCard
        run={run}
        macro={binanceMacro}
        phase="running"
        status={ok}
        prices={[100, 101]}
        priceTimes={[now - 12 * 60_000, now]}
      />,
    );
    expect(container.querySelector(".chart-span")?.textContent).toBe("앱을 연 뒤부터 · 12분");
  });

  it("국내 시세에 함께 온 전일 대비 등락률을 현재가 옆에 보인다", () => {
    render(
      <StatusCard
        run={makeRun({ price: 118_350_000, dayChangePct: -1.234 })}
        macro={upbitMacro}
        phase="running"
        status={ok}
        prices={[]}
      />,
    );
    const change = screen.getByText(/전일/);
    expect(change.textContent?.trim()).toBe("전일 -1.23%");
    expect(change.className).toContain("tone-down");
  });
});

describe("목록 카드와 상태 카드의 %", () => {
  it("둘 다 같은 값(run.unrealizedPct)을 같은 모양으로 보임", () => {
    const run = makeRun({
      inPosition: true,
      qty: 10,
      entryPrice: 100,
      price: 100.15,
      unrealized: -0.5015,
      unrealizedPct: -0.0501,
      unrealizedFeeAware: true,
      invested: 1001,
    });
    const slot = makeSlot({ phase: "running", run, status: ok, mode: "live" });
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
        <StatusCard run={run} macro={slot.macro} phase="running" status={ok} prices={[]} />
      </RunnerContext.Provider>,
    );
    const list = container.querySelector(".run-card-pct")?.textContent;
    const card = container.querySelector(".pnl-pct")?.textContent;
    expect(list).toBe("-0.05%");
    expect(card).toBe(list);
  });
});

describe("진입 대기", () => {
  it("안 샀으면 평가손익 자리에 조건 · 거리 · 막대, 아래에 세 칸, 차트에 사는 선", () => {
    const { container } = render(
      <StatusCard
        run={makeRun({ entryWait: makeEntryWait() })}
        macro={binanceMacro}
        phase="running"
        mode="mock"
        status={{ text: "진입 대기", tone: "ok" }}
        prices={[100, 100.5, 101]}
      />,
    );
    expect(screen.getByText("진입 조건 · 모의")).toBeTruthy();
    expect(screen.getByText("가격이 102.00 위로 올라가면 사요.")).toBeTruthy();
    expect(screen.getByText("지금 101.00 · +1.0% 더 오르면")).toBeTruthy();
    expect(screen.getByRole("img", { name: /이 봉 시가 100.00에서 사는 선 102.00까지 중 지금 101.00/ })).toBeTruthy();
    expect(screen.getByText("평소 기다린 시간")).toBeTruthy();
    expect(screen.getByText("약 9시간")).toBeTruthy();
    expect(screen.getByText("3번")).toBeTruthy();
    expect(container.querySelector(".chart-tag.is-buy")?.textContent).toContain("사는 선 102.00");
    expect(screen.queryByText(/평가손익/)).toBeNull();
  });

  it("실시간 규칙(밴드)은 다음 확인 대신 '실시간'", () => {
    render(
      <StatusCard
        run={makeRun({ entryWait: makeEntryWait({ realtime: true, checks: null, typical: null }) })}
        macro={binanceMacro}
        phase="running"
        status={ok}
        prices={[]}
      />,
    );
    expect(screen.getByText("실시간")).toBeTruthy();
    expect(screen.getByText("가격이 닿는 순간")).toBeTruthy();
    expect(screen.getByText("계속")).toBeTruthy();
  });

  it("샀으면 평소처럼 평가손익", () => {
    render(
      <StatusCard
        run={makeRun({ inPosition: true, qty: 1, entryPrice: 102, entryWait: makeEntryWait() })}
        macro={binanceMacro}
        phase="running"
        status={ok}
        prices={[]}
      />,
    );
    expect(screen.getByText("평가손익 · 지금 정리하면")).toBeTruthy();
    expect(screen.queryByText("가격이 102.00 위로 올라가면 사요.")).toBeNull();
  });
});
