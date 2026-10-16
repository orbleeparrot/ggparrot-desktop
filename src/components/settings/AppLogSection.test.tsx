import { act, cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import type { ReactNode } from "react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { call, on } from "../../ipc";
import { RunnerContext, type RunnerContextValue } from "../../state/context";
import { makeState } from "../../test/fixtures";
import { EV_APP_LOG, type AppLogEntry } from "../../types";
import { AppLogSection } from "./AppLogSection";

vi.mock("../../ipc", async (importOriginal) => {
  const actual = await importOriginal<typeof import("../../ipc")>();
  return { ...actual, call: vi.fn(), on: vi.fn() };
});

const AT = 1_760_000_000_000;
const entry = (over: Partial<AppLogEntry>): AppLogEntry => ({
  id: 1,
  at: AT,
  level: "info",
  source: "app",
  title: "",
  detail: "",
  ...over,
});

let log: AppLogEntry[];
let emit: ((e: AppLogEntry) => void) | null;

beforeEach(() => {
  emit = null;
  log = [
    entry({ id: 1, title: "앱을 시작했어요" }),
    entry({ id: 2, level: "warn", source: "update", title: "업데이트 정보를 확인하지 못했어요", detail: "연결 실패" }),
    entry({ id: 3, level: "error", source: "storage", title: "기억한 키를 저장하지 못했어요" }),
  ];
  vi.mocked(on).mockImplementation(async (name, handler) => {
    if (name === EV_APP_LOG) emit = handler as (e: AppLogEntry) => void;
    return () => undefined;
  });
  vi.mocked(call).mockImplementation((async (cmd: string) => (cmd === "get_app_log" ? log : undefined)) as typeof call);
});

afterEach(() => {
  cleanup();
  vi.clearAllMocks();
});

function withContext(node: ReactNode) {
  const value = { state: makeState(), ok: vi.fn().mockResolvedValue(true) } as unknown as RunnerContextValue;
  render(<RunnerContext.Provider value={value}>{node}</RunnerContext.Provider>);
  return value;
}

const rows = () => within(screen.getByRole("log", { name: "앱 기록" })).queryAllByRole("listitem");

describe("설정 › 앱 기록", () => {
  it("get_app_log를 읽어 최근 것을 위에 — 출처 이름·수준을 보이고, 설명은 [›]로 펼친다", async () => {
    withContext(<AppLogSection focus={null} />);
    await waitFor(() => expect(rows()).toHaveLength(3));
    expect(rows().map((r) => r.id)).toEqual(["app-log-3", "app-log-2", "app-log-1"]);
    expect(rows()[0].textContent).toContain("문제: 저장소기억한 키를 저장하지 못했어요");
    expect(rows()[1].textContent).toContain("업데이트");
    expect(screen.queryByText("연결 실패")).toBeNull();
    const toggle = within(rows()[1]).getByRole("button", { name: "자세히 보기" });
    fireEvent.click(toggle);
    expect(screen.getByText("연결 실패").id).toBe(toggle.getAttribute("aria-controls"));
    // 설명이 없는 줄에는 펼치기 단추가 없음
    expect(within(rows()[0]).queryByRole("button")).toBeNull();
  });

  it("필터 칩 — 주의·문제만 보이고 개수를 단다", async () => {
    withContext(<AppLogSection focus={null} />);
    await waitFor(() => expect(rows()).toHaveLength(3));
    const warn = screen.getByRole("button", { name: /^주의\s*1$/ });
    fireEvent.click(warn);
    expect(warn.getAttribute("aria-pressed")).toBe("true");
    expect(rows().map((r) => r.id)).toEqual(["app-log-2"]);
    fireEvent.click(screen.getByRole("button", { name: /^문제\s*1$/ }));
    expect(rows().map((r) => r.id)).toEqual(["app-log-3"]);
  });

  it("runner:app-log 이벤트는 id로 합친다 — 이미 받은 줄은 두 번 보이지 않음", async () => {
    withContext(<AppLogSection focus={null} />);
    await waitFor(() => expect(rows()).toHaveLength(3));
    act(() => emit!(log[2]));
    act(() => emit!(entry({ id: 4, level: "warn", source: "web", title: "껄무새 웹과 연결이 끊겼어요" })));
    expect(rows().map((r) => r.id)).toEqual(["app-log-4", "app-log-3", "app-log-2", "app-log-1"]);
    expect(screen.getByRole("button", { name: /^주의\s*2$/ })).toBeTruthy();
  });

  it("[기록 복사]는 copy_app_log를 보내고 '복사했어요'로 바뀐다", async () => {
    const ctx = withContext(<AppLogSection focus={null} />);
    await waitFor(() => expect(rows()).toHaveLength(3));
    fireEvent.click(screen.getByRole("button", { name: "기록 복사" }));
    expect(ctx.ok).toHaveBeenCalledWith("copy_app_log");
    expect(await screen.findByRole("button", { name: "복사했어요" })).toBeTruthy();
  });

  it("기록이 없으면 빈 안내, 읽지 못하면 [다시 불러오기]", async () => {
    log = [];
    withContext(<AppLogSection focus={null} />);
    expect(await screen.findByText(/아직 기록이 없어요/)).toBeTruthy();
    expect((screen.getByRole("button", { name: "기록 복사" }) as HTMLButtonElement).disabled).toBe(true);
    cleanup();

    vi.mocked(call).mockRejectedValueOnce("실패");
    withContext(<AppLogSection focus={null} />);
    fireEvent.click(await screen.findByRole("button", { name: "다시 불러오기" }));
    await waitFor(() => expect(screen.queryByRole("alert")).toBeNull());
  });

  it("focus로 온 줄은 필터를 풀고 펼친 뒤 포커스를 둔다", async () => {
    const { rerender } = render(<div />);
    const value = { state: makeState(), ok: vi.fn() } as unknown as RunnerContextValue;
    const view = (focus: { id: number; seq: number } | null) => (
      <RunnerContext.Provider value={value}>
        <AppLogSection focus={focus} />
      </RunnerContext.Provider>
    );
    rerender(view(null));
    await waitFor(() => expect(rows()).toHaveLength(3));
    fireEvent.click(screen.getByRole("button", { name: /^문제\s*1$/ }));
    rerender(view({ id: 2, seq: 1 }));
    await waitFor(() => expect(document.activeElement?.id).toBe("app-log-2"));
    expect(screen.getByRole("button", { name: "전체" }).getAttribute("aria-pressed")).toBe("true");
    expect(screen.getByText("연결 실패")).toBeTruthy();
  });
});
