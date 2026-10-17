import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { RunnerContext, type RunnerContextValue } from "../../state/context";
import { binanceMacro, makeEntryWait, makeRun, makeSlot, makeState, upbitMacro } from "../../test/fixtures";
import { RunCard } from "./RunCard";
import { DETAILS_KEY, StatusCard } from "./StatusCard";

afterEach(cleanup);
// 세부 표(현재가 · 평단 · 투입금 …)는 기본으로 접혀 있다 — 표를 읽는 검사들은 펼친 상태로 본다.
beforeEach(() => window.localStorage.setItem(DETAILS_KEY, "open"));

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

  it("보유가 없으면 투입금·보유 수량은 '없음', 청산 기준은 칩 한 줄", () => {
    const { container } = render(
      <StatusCard run={makeRun()} macro={binanceMacro} phase="running" status={ok} prices={[]} />,
    );
    expect(screen.getByText("투입금").nextElementSibling?.textContent).toBe("없음");
    expect(screen.getByText("보유 수량").nextElementSibling?.textContent).toBe("없음");
    const chips = [...container.querySelectorAll(".exit-chip")].map((chip) => [chip.className, chip.textContent]);
    expect(chips).toEqual([
      ["exit-chip tone-up", "익절 +5%"],
      ["exit-chip tone-down", "손절 -3%"],
    ]);
    expect(screen.getByRole("group", { name: "청산 기준 · 익절 +5% · 손절 -3% (평단 대비 가격)" })).toBeTruthy();
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
  it("안 샀으면 평가손익 자리에 지금 상태 한 줄 · 짧은 설명 · 막대, 차트에 사는 선", () => {
    const { container } = render(
      <StatusCard
        run={makeRun({ entryWait: makeEntryWait({ headline: "102.00 넘으면 사요" }) })}
        macro={binanceMacro}
        phase="running"
        mode="mock"
        status={{ text: "진입 대기", tone: "ok" }}
        prices={[100, 100.5, 101]}
      />,
    );
    expect(screen.getByText("1시간 봉 · 모의")).toBeTruthy();
    expect(container.querySelector(".ew-sentence")?.textContent).toBe("102.00 넘으면 사요");
    expect(container.querySelector(".ew-distance")?.textContent).toBe("지금 101.00 · +1.0% 더 오르면");
    expect(screen.getByRole("img", { name: /이 봉 시가 100.00에서 사는 선 102.00까지 중 지금 101.00/ })).toBeTruthy();
    expect(container.querySelector(".ew-conds")).toBeNull(); // 조건이 하나뿐이면 체크 목록 없음
    expect(container.querySelector(".ew-ladder")).toBeNull();
    expect(container.querySelector(".chart-tag.is-buy")?.textContent).toContain("사는 선 102.00");
    expect(screen.queryByText(/평가손익/)).toBeNull();
  });

  it("옛 서버(headline 없음)면 조건 문장을 마침표 없이 큰 글씨로", () => {
    const { container } = render(
      <StatusCard
        run={makeRun({ entryWait: makeEntryWait() })}
        macro={binanceMacro}
        phase="running"
        status={ok}
        prices={[]}
      />,
    );
    expect(container.querySelector(".ew-sentence")?.textContent).toBe("가격이 102.00 위로 올라가면 사요");
  });

  it("사는 선은 넘었지만 20봉 평균선 아래 — 숫자 하나 · 가격 막대에 두 선 · 체크(10/17 KRW-FORT)", () => {
    const { container } = render(
      <StatusCard
        run={makeRun({
          entryWait: makeEntryWait({
            headline: "26.523 위에서 끝나면 사요",
            distance: "지금 26.2 · +1.2% 더 · 사는 선은 넘었고 20봉 평균선이 남았어요",
            touched: true,
            reached: false,
            gap: "평균선 −1.2%",
            next_check_ms: Date.now() + 5 * 60_000 + 20_000,
            ladder: {
              marks: [
                { key: "line", label: "사는 선", value: "26.118", pos: 0.27, main: false },
                { key: "ma", label: "20봉 평균", value: "26.523", pos: 0.9, main: true },
              ],
              now: { label: "지금", value: "26.2", pos: 0.4 },
              zone: 0.9,
            },
            conditions: [
              { key: "line", label: "사는 선 26.118 넘기", now: "넘음 · 고가 26.26", ok: true, at_close: false },
              { key: "ma", label: "20봉 평균 26.523 위에서 끝나기", now: "−1.2%", ok: false, at_close: true },
            ],
          }),
        })}
        macro={binanceMacro}
        phase="running"
        status={ok}
        prices={[]}
      />,
    );
    expect(container.querySelector(".ew-sentence")?.textContent).toBe("26.523 위에서 끝나면 사요");
    expect(screen.getByRole("img", { name: "사는 선 26.118 · 20봉 평균 26.523 중 지금 26.2" })).toBeTruthy();
    expect(container.querySelector(".ew-meter")).toBeNull(); // 막대는 하나만
    const rows = [...container.querySelectorAll(".ew-conds li")].map((row) => [row.className, row.textContent]);
    expect(rows).toEqual([
      ["is-ok", "사는 선 26.118 넘기넘음 · 고가 26.26"],
      ["is-wait", "20봉 평균 26.523 위에서 끝나기−1.2%"],
    ]);
    expect(container.querySelector(".status-head")?.textContent).toMatch(/\d\d:\d\d 마감까지 5분/);
  });

  it("RSI 처럼 내려가야 사는 규칙은 값 눈금에 사는 구역을 사는 선 왼쪽(아래)에 칠함", () => {
    const { container } = render(
      <StatusCard
        run={makeRun({
          entryWait: makeEntryWait({
            rule: "F",
            sentence: "RSI(14)가 40 아래로 내려가면 사요.",
            scale: {
              marks: [
                { key: "line", label: "사는 선", value: "40", pos: 0.12, main: true },
                { key: "exit", label: "청산 기준", value: "70", pos: 0.88, main: false },
              ],
              now: { label: "지금", value: "64.5", pos: 0.74 },
              zone: 0.12,
              zone_side: "below",
            },
          }),
        })}
        macro={binanceMacro}
        phase="running"
        status={ok}
        prices={[]}
      />,
    );
    expect(screen.getByRole("img", { name: "사는 선 40 · 청산 기준 70 중 지금 64.5" })).toBeTruthy();
    expect(container.querySelector(".ew-ladder-zone")?.className).toContain("is-below");
    expect(container.querySelector(".ew-meter")).toBeNull();
  });

  it("실시간 규칙(밴드)은 맨 윗줄에 '실시간 확인'", () => {
    render(
      <StatusCard
        run={makeRun({ entryWait: makeEntryWait({ realtime: true, checks: null, typical: null }) })}
        macro={binanceMacro}
        phase="running"
        status={ok}
        prices={[]}
      />,
    );
    expect(screen.getByText("실시간 확인")).toBeTruthy();
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

describe("세부 표 접기", () => {
  it("처음엔 접혀 있고, 누르면 펼쳐져 이 PC에 기억된다", () => {
    window.localStorage.removeItem(DETAILS_KEY);
    const { container } = render(
      <StatusCard run={makeRun()} macro={binanceMacro} phase="running" status={ok} prices={[]} />,
    );
    expect(container.querySelector(".run-table")).toBeNull();
    const toggle = screen.getByRole("button", { name: "현재가 · 평단 · 수량 자세히" });
    expect(toggle.getAttribute("aria-expanded")).toBe("false");
    fireEvent.click(toggle);
    expect(container.querySelector(".run-table")).toBeTruthy();
    expect(window.localStorage.getItem(DETAILS_KEY)).toBe("open");
    fireEvent.click(screen.getByRole("button", { name: "자세히 접기" }));
    expect(container.querySelector(".run-table")).toBeNull();
  });
});
