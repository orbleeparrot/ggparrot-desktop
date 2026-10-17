import { act, cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import App from "./App";
import { call, on } from "./ipc";
import { act as activity, binanceMacro, makeRun, makeSettings, makeSlot, makeState, upbitMacro } from "./test/fixtures";
import type { Activity, AppLogEntry, AppState, LibraryFile, StartReply } from "./types";
import { EV_CONFIRM, EV_NAVIGATE, EV_NOTICE, EV_STATE } from "./types";

vi.mock("./ipc", async (importOriginal) => {
  const actual = await importOriginal<typeof import("./ipc")>();
  return { ...actual, call: vi.fn(), on: vi.fn() };
});

const handlers = new Map<string, (p: unknown) => void>();
let current: AppState;
let startReply: StartReply;
let history: Activity[];
let appLog: AppLogEntry[];

beforeEach(() => {
  handlers.clear();
  current = makeState();
  startReply = { kind: "started" };
  history = [];
  appLog = [];
  vi.mocked(on).mockImplementation(async (name, handler) => {
    handlers.set(name, handler as (p: unknown) => void);
    return () => handlers.delete(name);
  });
  vi.mocked(call).mockImplementation((async (cmd: string) => {
    if (cmd === "get_state") return current;
    if (cmd === "get_activity") return history;
    if (cmd === "get_app_log") return appLog;
    if (cmd === "start_run") return startReply;
    if (cmd === "start_run_confirmed") return { kind: "started" };
    if (cmd === "add_run_from_library") return "run-2";
    return undefined;
  }) as typeof call);
});

afterEach(() => {
  cleanup();
  vi.clearAllMocks();
});

const calledWith = (cmd: string) => vi.mocked(call).mock.calls.filter((c) => c[0] === cmd);
const pushState = (s: AppState) => act(() => handlers.get(EV_STATE)!(s));
const ready = () => waitFor(() => expect(handlers.has(EV_STATE)).toBe(true));

const libFile: LibraryFile = {
  fileName: "업비트.ggm.json",
  valid: true,
  error: "",
  title: "업비트 눌림목",
  exchange: "upbit",
  exchangeLabel: "업비트",
  symbol: "KRW-BTC",
  side: "long",
  summary: "",
  strategy: "RSI 조건 매매",
  unsupported: "",
  signed: true,
  modifiedAt: Date.now(),
  inUse: 0,
};

describe("껄무새 웹", () => {
  it("머리줄 [껄무새 웹 열기]는 open_web을 보낸다", async () => {
    render(<App />);
    await ready();
    fireEvent.click(await screen.findByRole("button", { name: "껄무새 웹 열기" }));
    expect(calledWith("open_web")).toHaveLength(1);
  });
});

describe("매크로 추가", () => {
  it("폴더에서 고르면 add_run_from_library 를 보내고 새 슬롯을 고른다", async () => {
    current = makeState({ library: { dir: "/m", files: [libFile], skipped: 0, truncated: 0 } });
    render(<App />);
    fireEvent.click(await screen.findByRole("button", { name: "매크로 추가" }));
    const sheet = await screen.findByRole("dialog", { name: "매크로 추가" });
    fireEvent.click(within(sheet).getByRole("button", { name: /업비트 눌림목/ }));
    await waitFor(() =>
      expect(calledWith("add_run_from_library")).toEqual([["add_run_from_library", { fileName: "업비트.ggm.json" }]]),
    );
    await ready();
    await pushState(
      makeState({ runs: [makeSlot(), makeSlot({ id: "run-2", macro: upbitMacro, modeChoices: ["mock", "live"] })] }),
    );
    await waitFor(() => expect(screen.queryByRole("dialog")).toBeNull());
    const selected = document.querySelector('[aria-current="true"]');
    expect(selected?.textContent).toContain("KRW-BTC");
    expect(screen.getByRole("heading", { level: 2, name: /KRW-BTC/ })).toBeTruthy();
    await waitFor(() =>
      expect(calledWith("get_activity").some((c) => (c[1] as { runId: string }).runId === "run-2")).toBe(true),
    );
  });

  it("목록이 비면 첫 실행 안내와 [매크로 추가]", async () => {
    current = makeState({ runs: [] });
    render(<App />);
    expect(await screen.findByRole("heading", { name: "실행할 매크로를 추가하세요" })).toBeTruthy();
    expect(screen.getByText("설정에서 키 연결")).toBeTruthy();
    fireEvent.click(screen.getByRole("button", { name: "매크로 추가" }));
    expect(await screen.findByRole("dialog", { name: "매크로 추가" })).toBeTruthy();
  });
});

describe("실전 확인 시트", () => {
  it("start_run 이 confirm_live 면 시트를 띄우고, [실전으로 시작] 으로 start_run_confirmed 를 보낸다", async () => {
    current = makeState({ runs: [makeSlot({ mode: "live" })] });
    startReply = { kind: "confirm_live", title: "바이낸스 실전으로 시작할까요?", message: "실제 자금으로 주문해요." };
    render(<App />);
    fireEvent.click(await screen.findByRole("button", { name: "실전으로 시작" }));
    const sheet = await screen.findByRole("dialog", { name: "바이낸스 실전으로 시작할까요?" });
    // 제목 바로 아래는 칸 — Rust가 보낸 설명 글(회색 줄글)은 다시 적지 않음.
    expect(sheet.textContent).not.toContain("실제 자금으로 주문해요.");
    expect(within(sheet).getByText("투자 금액")).toBeTruthy();
    expect(within(sheet).getByText("한 번에 최대")).toBeTruthy();
    expect(within(sheet).queryByText("이 매크로는 바로 사지 않아요")).toBeNull();
    expect(document.activeElement?.textContent).toBe("취소");
    fireEvent.click(within(sheet).getByRole("button", { name: "실전으로 시작" }));
    await waitFor(() =>
      expect(calledWith("start_run_confirmed")).toEqual([["start_run_confirmed", { runId: "run-1" }]]),
    );
    expect(screen.queryByRole("dialog")).toBeNull();
  });

  it("기다리는 실전 매크로는 한 장에 '바로 사지 않아요' 칸 · 숫자 세 칸 · 알림 — 안내 시트를 따로 띄우지 않음", async () => {
    current = makeState({ runs: [makeSlot({ mode: "live", waits: true })] });
    startReply = { kind: "confirm_live", title: "바이낸스 실전으로 시작할까요?", message: "실제 자금으로 주문해요." };
    vi.mocked(call).mockImplementation((async (cmd: string) => {
      if (cmd === "get_state") return current;
      if (cmd === "get_activity") return history;
      if (cmd === "get_app_log") return appLog;
      if (cmd === "entry_preview")
        return {
          waiting: true,
          sentence: "RSI(14)가 30 아래로 내려가면 사요.",
          typical: { episodes: 21, span_ms: 5 * 3_600_000, avg_ms: 14 * 60_000, longest_ms: 52 * 60_000 },
        };
      if (cmd === "start_run") return startReply;
      if (cmd === "start_run_confirmed") return { kind: "started" };
      return undefined;
    }) as typeof call);
    render(<App />);
    fireEvent.click(await screen.findByRole("button", { name: "실전으로 시작" }));
    const sheet = await screen.findByRole("dialog", { name: "바이낸스 실전으로 시작할까요?" });
    expect(screen.getAllByRole("dialog")).toHaveLength(1);
    const box = within(sheet).getByRole("region", { name: "이 매크로는 바로 사지 않아요" });
    expect(box.textContent).toContain("RSI(14)가 30 아래로 내려가면 사요. 조건이 맞을 때만 사요.");
    expect(within(box).getByText("21번 진입")).toBeTruthy();
    fireEvent.click(within(sheet).getByRole("checkbox", { name: "사면 알림으로 알려 주기" }));
    fireEvent.click(within(sheet).getByRole("button", { name: "실전으로 시작" }));
    await waitFor(() => expect(calledWith("start_run_confirmed")).toHaveLength(1));
    expect(calledWith("set_entry_notify")).toEqual([["set_entry_notify", { runId: "run-1", on: false }]]);
  });

  it("검사가 끝난 뒤 온 runner:confirm 도 같은 시트 · Esc 로 닫으면 시작하지 않는다", async () => {
    render(<App />);
    await waitFor(() => expect(handlers.has(EV_CONFIRM)).toBe(true));
    act(() => handlers.get(EV_CONFIRM)!({ runId: "run-1", title: "실전 확인", message: "계속할까요?" }));
    const sheet = await screen.findByRole("dialog", { name: "실전 확인" });
    fireEvent.keyDown(sheet, { key: "Escape" });
    expect(screen.queryByRole("dialog")).toBeNull();
    expect(calledWith("start_run_confirmed")).toHaveLength(0);
  });
});

describe("막힌 이유 → 설정", () => {
  it("키가 없으면 [시작] 자리가 [○○ 키 연결하기] — 누르면 그 칸의 연결 도우미를 연다", async () => {
    current = makeState({
      runs: [
        makeSlot({
          mode: "testnet",
          keySlot: "binance_testnet",
          readiness: {
            canStart: false,
            blocker: "바이낸스 테스트넷 API 키가 필요해요. 설정에서 입력하세요.",
            needs: { kind: "key", slot: "binance_testnet" },
          },
        }),
      ],
    });
    render(<App />);
    fireEvent.click(await screen.findByRole("button", { name: "바이낸스 테스트넷 키 연결하기" }));
    await waitFor(() =>
      expect(calledWith("wizard_open")).toEqual([["wizard_open", { exchange: "binance", slot: "binance_testnet" }]]),
    );
    // 막힌 이유 띠는 따로 두지 않음.
    expect(screen.queryByText("바이낸스 테스트넷 API 키가 필요해요. 설정에서 입력하세요.")).toBeNull();
  });

  it("blocked 답은 대화상자로, 필요한 칸이 있으면 [설정에서 입력] 도 준다", async () => {
    current = makeState({
      runs: [makeSlot({ readiness: { canStart: false, blocker: "", needs: { kind: "member" } } })],
    });
    startReply = { kind: "blocked", message: "회원 키가 필요해요. 설정 › 껄무새 계정에서 입력하세요." };
    render(<App />);
    fireEvent.click(await screen.findByRole("button", { name: "BTCUSDT 시작" }));
    const dlg = await screen.findByRole("dialog", { name: "시작할 수 없어요" });
    fireEvent.click(within(dlg).getByRole("button", { name: "설정에서 입력" }));
    await waitFor(() => expect(document.activeElement?.id).toBe("member-key"));
  });
});

describe("지원하지 않는 거래소", () => {
  it("대기 중이면 '시작 전 확인'의 막힌 줄에만 한 번 적는다", async () => {
    const sentence = "이 앱이 지원하지 않는 거래소(okx)예요. 바이낸스 · 업비트 · 빗썸 매크로만 실행할 수 있어요.";
    current = makeState({
      runs: [
        makeSlot({
          macro: { ...binanceMacro, exchange: null, exchangeName: "okx", exchangeLabel: "" },
          readiness: { canStart: false, blocker: sentence, needs: null },
        }),
      ],
    });
    render(<App />);
    const checks = await screen.findByRole("region", { name: "시작 전 확인" });
    const detail = document.getElementById("run-detail")!;
    expect(detail.textContent!.split("지원하지 않는 거래소")).toHaveLength(2);
    expect(within(checks).getByText(sentence)).toBeTruthy();
  });
});

describe("목록에서 빼기", () => {
  it("먼저 묻고(포커스는 취소), [목록에서 빼기]로 remove_run 을 보낸다", async () => {
    render(<App />);
    fireEvent.click(await screen.findByRole("button", { name: "BTCUSDT 목록에서 빼기" }));
    const dlg = await screen.findByRole("dialog", { name: "목록에서 뺄까요?" });
    expect(dlg.textContent).toContain("매크로 파일은 지우지 않아요");
    expect(document.activeElement?.textContent).toBe("취소");
    fireEvent.click(within(dlg).getByRole("button", { name: "목록에서 빼기" }));
    await waitFor(() => expect(calledWith("remove_run")).toEqual([["remove_run", { runId: "run-1" }]]));
    expect(screen.queryByRole("dialog")).toBeNull();
  });

  it("웹 매크로는 웹에 그대로 있다고 말하고, [취소]면 보내지 않는다", async () => {
    current = makeState({ runs: [makeSlot({ macro: upbitMacro })] });
    render(<App />);
    fireEvent.click(await screen.findByRole("button", { name: "KRW-BTC 목록에서 빼기" }));
    const dlg = await screen.findByRole("dialog", { name: "목록에서 뺄까요?" });
    expect(dlg.textContent).toContain("웹에 저장한 매크로는 그대로예요");
    fireEvent.click(within(dlg).getByRole("button", { name: "취소" }));
    expect(screen.queryByRole("dialog")).toBeNull();
    expect(calledWith("remove_run")).toHaveLength(0);
  });
});

describe("활동 기록 · 개발자 로그", () => {
  it("원문 줄은 숨겨 두고, 설정에서 켜면 보인다", async () => {
    history = [
      activity({
        id: 1,
        kind: "buy",
        title: "BTC 0.00122개를 81,806 USDT에 샀어요",
        raw: "BUY 체결: id=abc 수량=0.00122",
      }),
      activity({ id: 2, kind: "info", title: "heartbeat ok · price=81806", dev: true }),
    ];
    render(<App />);
    expect(await screen.findByText("BTC 0.00122개를 81,806 USDT에 샀어요")).toBeTruthy();
    expect(screen.queryByText("heartbeat ok · price=81806")).toBeNull();
    const toggle = screen.getByRole("button", { name: "자세히 보기" });
    expect(toggle.getAttribute("aria-expanded")).toBe("false");
    fireEvent.click(toggle);
    expect(toggle.getAttribute("aria-expanded")).toBe("true");
    expect(toggle.getAttribute("aria-label")).toBe("접기");
    // 펼친 칸(둘째 말 전체 · 원문)이 단추가 가리키는 칸 안에 있음.
    const more = document.getElementById(toggle.getAttribute("aria-controls") ?? "");
    expect(more?.contains(screen.getByText("BUY 체결: id=abc 수량=0.00122"))).toBe(true);

    fireEvent.click(screen.getByRole("button", { name: "설정" }));
    fireEvent.click(await screen.findByRole("switch", { name: "개발자 로그 보기" }));
    await waitFor(() => expect(calledWith("set_show_dev_log")).toEqual([["set_show_dev_log", { show: true }]]));
    await pushState(makeState({ settings: makeSettings({ showDevLog: true }) }));
    expect((screen.getByRole("switch", { name: "개발자 로그 보기" }) as HTMLElement).getAttribute("aria-checked")).toBe(
      "true",
    );
    fireEvent.click(screen.getByRole("button", { name: "실행 화면으로" }));
    expect(await screen.findByText("heartbeat ok · price=81806")).toBeTruthy();
  });
});

describe("종료 · 이동", () => {
  it("[■ 종료 ▾] 에서 고르면 '{label}할까요?' 를 묻고 stop_run 을 보낸다", async () => {
    const holding = makeRun({ inPosition: true, qty: 1, entryPrice: 100 });
    current = makeState({ runs: [makeSlot({ phase: "running", mode: "live", run: holding })] });
    render(<App />);
    fireEvent.click(await screen.findByRole("button", { name: "BTCUSDT 종료" }));
    fireEvent.click(await screen.findByRole("menuitem", { name: "청산 후 종료" }));
    const dlg = await screen.findByRole("dialog", { name: "청산 후 종료할까요?" });
    expect(dlg.textContent).toContain("보유 포지션을 시장가로 정리한 뒤 종료해요.");
    // 제목 바로 아래 칸 — 지금 정리하면 얼마인지 · 정리할 수량 · 되돌릴 수 없음.
    expect(within(dlg).getByText("평가손익 · 지금 정리하면")).toBeTruthy();
    expect(within(dlg).getByText("정리할 수량").nextElementSibling?.textContent).toBe("1 BTC · 시장가");
    expect(within(dlg).getByText("되돌릴 수 없어요")).toBeTruthy();
    fireEvent.click(within(dlg).getByRole("button", { name: "청산 후 종료" }));
    await waitFor(() =>
      expect(calledWith("stop_run")).toEqual([["stop_run", { runId: "run-1", mode: "close_and_stop" }]]),
    );
  });

  it("모의 · 보유 없음이면 고를 게 없어 [■ 종료] 하나 — 모의는 실제 주문이 없다고 말한다", async () => {
    const holding = makeRun({ inPosition: true, qty: 1, entryPrice: 100 });
    current = makeState({ runs: [makeSlot({ phase: "running", mode: "mock", run: holding })] });
    render(<App />);
    fireEvent.click(await screen.findByRole("button", { name: "BTCUSDT 종료" }));
    expect(screen.queryByRole("menuitem")).toBeNull();
    const dlg = await screen.findByRole("dialog", { name: "종료할까요?" });
    expect(dlg.textContent).toContain("실제 주문은 보내지 않아요");
    expect(dlg.textContent).not.toContain("청산");
    fireEvent.click(within(dlg).getByRole("button", { name: "종료" }));
    await waitFor(() =>
      expect(calledWith("stop_run")).toEqual([["stop_run", { runId: "run-1", mode: "close_and_stop" }]]),
    );
  });

  it("실전이라도 보유가 없으면 두 방법이 같아 [■ 종료] 하나", async () => {
    current = makeState({ runs: [makeSlot({ phase: "running", mode: "live", run: makeRun() })] });
    render(<App />);
    fireEvent.click(await screen.findByRole("button", { name: "BTCUSDT 종료" }));
    const dlg = await screen.findByRole("dialog", { name: "종료할까요?" });
    expect(dlg.textContent).toContain("보유 포지션이 없어요");
  });

  it("실행 중이면 모드 세그먼트 대신 모드 배지 — 조작 줄은 배지와 종료 단추뿐, 대기면 세그먼트", async () => {
    current = makeState({ runs: [makeSlot({ phase: "running", mode: "live", run: makeRun() })] });
    render(<App />);
    const detail = await screen.findByRole("region", { name: /BTCUSDT/ });
    const row = detail.querySelector(".detail-controls")!;
    expect(within(row as HTMLElement).queryByRole("radiogroup")).toBeNull();
    expect(row.querySelector(".detail-mode-badge")?.textContent).toBe("실전으로 실행 중");
    expect(within(row as HTMLElement).getByRole("button", { name: "종료" })).toBeTruthy();
    pushState(makeState({ runs: [makeSlot({ phase: "idle", mode: "mock" })] }));
    expect(await within(row as HTMLElement).findByRole("radiogroup", { name: "실행 모드" })).toBeTruthy();
    expect(row.querySelector(".detail-mode-badge")).toBeNull();
  });

  it("runner:navigate 와 ⌘/Ctrl+, 로 화면을 바꾼다", async () => {
    current = makeState({ runs: [makeSlot(), makeSlot({ id: "run-9", macro: upbitMacro })] });
    render(<App />);
    await screen.findByRole("heading", { level: 2, name: /BTCUSDT/ });
    await waitFor(() => expect(handlers.has(EV_NAVIGATE)).toBe(true));
    act(() => handlers.get(EV_NAVIGATE)!({ to: "run:run-9" }));
    expect(await screen.findByRole("heading", { level: 2, name: /KRW-BTC/ })).toBeTruthy();
    fireEvent.keyDown(window, { key: ",", ctrlKey: true });
    expect(await screen.findByRole("heading", { name: "설정" })).toBeTruthy();
    fireEvent.keyDown(window, { key: "Escape" });
    await waitFor(() => expect(screen.queryByRole("heading", { name: "설정" })).toBeNull());
  });
});

describe("설정 목차 · 기억한 키", () => {
  const openSettings = async () => {
    render(<App />);
    fireEvent.click(await screen.findByRole("button", { name: "설정" }));
    return screen.findByRole("navigation", { name: "설정 목차" });
  };

  it("목차는 읽는 묶음을 aria-current로 밝힘 — 제목이 위쪽 35% 선을 넘은 마지막 묶음, 맨 아래면 절반 선, 누르면 바로", async () => {
    // jsdom에는 배치가 없어 묶음 제목 위치·본문 높이를 직접 정함.
    const tops: Record<string, number> = {
      "settings-exchanges": 20,
      "settings-account": 700,
      "settings-library": 900,
      "settings-display": 1100,
      "settings-about": 1300,
      "settings-log": 1500,
    };
    const rect = vi.spyOn(Element.prototype, "getBoundingClientRect").mockImplementation(function (this: Element) {
      return { top: tops[this.id] ?? 0 } as DOMRect;
    });
    try {
      const nav = await openSettings();
      const current = () =>
        within(nav)
          .getAllByRole("button")
          .filter((b) => b.getAttribute("aria-current") === "true")
          .map((b) => b.textContent);
      expect(current()).toEqual(["거래소 연결"]);
      const root = document.querySelector<HTMLElement>(".settings-body")!;
      const box = { clientHeight: 600, scrollHeight: 2000, scrollTop: 300 };
      for (const [k, v] of Object.entries(box)) Object.defineProperty(root, k, { configurable: true, value: v });
      const scrollTo = (t: Record<string, number>) => {
        Object.assign(tops, t);
        fireEvent.scroll(root);
      };
      // 선(600 × 0.35 = 210) 위로 올라온 마지막 묶음 — 앞 묶음 꼬리가 보여도 매크로 폴더.
      scrollTo({
        "settings-exchanges": -900,
        "settings-account": -300,
        "settings-library": 120,
        "settings-display": 260,
        "settings-about": 700,
        "settings-log": 1100,
      });
      expect(current()).toEqual(["매크로 폴더"]);
      // 맨 아래 — 선이 절반(300)으로 내려감. 짧은 앱 기록이 아래 절반이면 정보, 위 절반이면 앱 기록.
      Object.defineProperty(root, "scrollTop", { configurable: true, value: 1400 });
      scrollTo({ "settings-display": -200, "settings-about": 40, "settings-log": 420 });
      expect(current()).toEqual(["정보"]);
      scrollTo({ "settings-about": -100, "settings-log": 280 });
      expect(current()).toEqual(["앱 기록"]);
      fireEvent.click(within(nav).getByRole("button", { name: "정보" }));
      expect(current()).toEqual(["정보"]);
    } finally {
      rect.mockRestore();
    }
  });

  it("목차에 '기억한 키'는 없고 맨 끝이 '앱 기록' — 평소에는 경고 카드 없이 보관 위치만 적는다", async () => {
    const nav = await openSettings();
    const items = within(nav)
      .getAllByRole("button")
      .map((b) => b.textContent);
    expect(items).not.toContain("기억한 키");
    expect(items.at(-1)).toBe("앱 기록");
    expect(screen.getByText(/기억한 키는 이 기기의 (키체인|Windows 자격 증명 관리자)에 보관해요/)).toBeTruthy();
    expect(screen.queryByText(/기억한 키를 읽지 못했어요/)).toBeNull();
    expect(screen.queryByRole("button", { name: "다시 읽기" })).toBeNull();
  });

  it("읽기 실패면 거래소 연결 맨 위에 경고 카드와 [다시 읽기]", async () => {
    current = makeState({ settings: makeSettings({ storage: { status: "read_error", note: "잠금을 풀어 주세요." } }) });
    await openSettings();
    const section = document.getElementById("settings-exchanges")!;
    expect(within(section).getByText(/기억한 키를 읽지 못했어요/)).toBeTruthy();
    fireEvent.click(within(section).getByRole("button", { name: "다시 읽기" }));
    await waitFor(() => expect(calledWith("reload_storage")).toHaveLength(1));
    expect(within(section).queryByRole("button", { name: "지우고 새로 만들기" })).toBeNull();
  });

  it("내용이 깨졌으면 [지우고 새로 만들기]를 먼저 묻고 recover_storage", async () => {
    current = makeState({ settings: makeSettings({ storage: { status: "decrypt_error", note: "" } }) });
    await openSettings();
    fireEvent.click(screen.getByRole("button", { name: "지우고 새로 만들기" }));
    const dlg = await screen.findByRole("dialog", { name: "기억한 키를 지우고 새로 만들까요?" });
    fireEvent.click(within(dlg).getByRole("button", { name: "지우고 새로 만들기" }));
    await waitFor(() => expect(calledWith("recover_storage")).toHaveLength(1));
  });

  it("키를 기억할 수 없는 OS면 경고 대신 짧은 안내만", async () => {
    current = makeState({ settings: makeSettings({ storage: { status: "unsupported", note: "" } }) });
    await openSettings();
    expect(screen.getByText("이 OS에서는 키를 이번 실행 동안만 기억해요.")).toBeTruthy();
    expect(screen.queryByText(/에 보관해요\.$/)).toBeNull();
  });
});

describe("알림 토스트 [자세히]", () => {
  it("앱 기록 번호가 있는 알림은 [자세히]로 설정 › 앱 기록의 그 줄을 열고 토스트를 닫는다", async () => {
    appLog = [
      { id: 1, at: Date.now(), level: "info", source: "app", title: "앱을 시작했어요", detail: "" },
      {
        id: 2,
        at: Date.now(),
        level: "info",
        source: "library",
        title: "매크로 파일 2개를 가져왔어요",
        detail: "바이낸스.ggm.json",
      },
    ];
    render(<App />);
    await ready();
    await waitFor(() => expect(handlers.has(EV_NOTICE)).toBe(true));
    act(() =>
      handlers.get(EV_NOTICE)!({
        level: "info",
        title: "매크로 폴더",
        message: "매크로 파일 2개를 가져왔어요",
        logId: 2,
      }),
    );
    const toast = (await screen.findByText("매크로 파일 2개를 가져왔어요")).closest(".toast") as HTMLElement;
    fireEvent.click(within(toast).getByRole("button", { name: "자세히" }));
    expect(await screen.findByRole("heading", { name: "설정" })).toBeTruthy();
    await waitFor(() => expect(document.activeElement?.id).toBe("app-log-2"));
    expect(screen.getByText("바이낸스.ggm.json")).toBeTruthy();
    expect(document.querySelector(".toast")).toBeNull();
  });

  it("앱 기록 번호가 없는 알림에는 [자세히]가 없다", async () => {
    render(<App />);
    await ready();
    await waitFor(() => expect(handlers.has(EV_NOTICE)).toBe(true));
    act(() => handlers.get(EV_NOTICE)!({ level: "info", title: "", message: "그냥 알림이에요" }));
    const toast = (await screen.findByText("그냥 알림이에요")).closest(".toast") as HTMLElement;
    expect(within(toast).queryByRole("button", { name: "자세히" })).toBeNull();
  });
});
