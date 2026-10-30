import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { ConfirmPayload } from "./ipc";
import { appLogLine, createMockBackend, TICK_MS } from "./mock";
import { keyText, MOCK_NEXT_VERSION, MOCK_VERSION, WEB_FILE_NAME } from "./mockData";
import type { Activity, AppLogEntry, AppState, Notice, StartReply } from "./types";
import { EV_ACTIVITY, EV_APP_LOG, EV_CONFIRM, EV_NOTICE } from "./types";

describe("mock 백엔드 (계약 v2)", () => {
  beforeEach(() => vi.useFakeTimers());
  afterEach(() => vi.useRealTimers());

  const make = (q = "") => createMockBackend(new URLSearchParams(q));
  const state = async (b: ReturnType<typeof make>) => (await b.invoke("get_state")) as AppState;

  it("처음엔 두 매크로(바이낸스 파일 · 업비트 웹)와 매크로 폴더 · 설정 칸 넷", async () => {
    const s = await state(make());
    expect(s.runs.map((r) => [r.macro.symbol, r.macro.source, r.webLinked])).toEqual([
      ["BTCUSDT", "file", false],
      ["KRW-BTC", "web", true],
    ]);
    expect(s.runs[1].modeChoices).toEqual(["mock", "live"]);
    expect(s.settings.slots.map((x) => x.slot)).toEqual([
      "binance",
      "binance_testnet",
      "binance_demo",
      "upbit",
      "bithumb",
    ]);
    expect(s.library.files.some((f) => !f.valid && f.error)).toBe(true);
    expect(s.library.files.find((f) => f.fileName.startsWith("BTC 추세"))?.inUse).toBe(1);
    // 웹 연결로 받은 매크로도 매크로 폴더에 파일로 남고 그 파일과 이어짐(서명은 없음).
    const web = s.library.files.find((f) => f.fileName === WEB_FILE_NAME);
    expect([web?.inUse, web?.valid, web?.signed]).toEqual([1, true, false]);
    expect((await state(make("demo=empty"))).runs).toEqual([]);
  });

  it("매크로 폴더에서 추가하면 새 runId 를 돌려주고, 읽지 못한 파일은 거절한다", async () => {
    const b = make();
    const s = await state(b);
    const bithumb = s.library.files.find((f) => f.exchange === "bithumb")!;
    const id = (await b.invoke("add_run_from_library", { fileName: bithumb.fileName })) as string;
    expect((await state(b)).runs.at(-1)?.id).toBe(id);
    const invalid = s.library.files.find((f) => !f.valid)!;
    await expect(b.invoke("add_run_from_library", { fileName: invalid.fileName })).rejects.toMatch(/읽지 못했어요/);
    const other = (await b.invoke("add_macro_file")) as string;
    expect((await state(b)).runs.find((r) => r.id === other)).toBeTruthy();
    expect(await b.invoke("library_import")).toBe(1);
  });

  it("모의 실행은 활동을 내고(원문 줄 포함) 매수 문장을 만든다 · 종료하면 finished", async () => {
    const b = make();
    const seen: Activity[] = [];
    await b.listen(EV_ACTIVITY, (a) => seen.push(a as Activity));
    const [run] = (await state(b)).runs;
    expect(await b.invoke("start_run", { runId: run.id })).toEqual({ kind: "started" });
    await vi.advanceTimersByTimeAsync(800 + TICK_MS * 20);
    expect((await state(b)).runs[0].phase).toBe("running");
    expect(seen[1]).toMatchObject({ kind: "start", title: "실행을 시작했어요 · 모의", dev: false });
    expect(seen[0].dev).toBe(true);
    expect(seen.find((a) => a.kind === "buy" && !a.dev)?.title).toMatch(/^BTC [\d.]+개를 [\d,.]+ USDT에 샀어요$/);
    const history = (await b.invoke("get_activity", { runId: run.id })) as Activity[];
    expect(history.map((a) => a.id)).toEqual(seen.map((a) => a.id));
    await expect(b.invoke("remove_run", { runId: run.id })).rejects.toMatch(/뺄 수 없어요/);
    await b.invoke("stop_run", { runId: run.id, mode: "close_and_stop" });
    await vi.advanceTimersByTimeAsync(1500);
    const done = (await state(b)).runs[0];
    expect(done.phase).toBe("finished");
    expect(done.run?.finished?.note).toBe("청산 완료 후 종료");
    expect(done.run?.inPosition).toBe(false);
  });

  it("같은 거래소 · 같은 종목은 동시에 하나만", async () => {
    const b = make();
    const s = await state(b);
    const dup = s.library.files.find((f) => f.symbol === "BTCUSDT" && f.fileName !== s.library.files[0].fileName)!;
    const second = (await b.invoke("add_run_from_library", { fileName: dup.fileName })) as string;
    await b.invoke("start_run", { runId: s.runs[0].id });
    const msg = "같은 거래소 · 종목(바이낸스 BTCUSDT)은 한 번에 하나만 실행해요. 지금 다른 매크로가 실행 중이에요.";
    expect(await b.invoke("start_run", { runId: second })).toEqual({ kind: "blocked", message: msg });
    const slot = (await state(b)).runs.find((r) => r.id === second)!;
    expect(slot.readiness).toEqual({ canStart: false, blocker: msg, needs: null });
  });

  it("실전은 연결 확인 뒤 runner:confirm 으로 묻고, start_run_confirmed 로 시작한다", async () => {
    const b = make();
    const confirms: ConfirmPayload[] = [];
    await b.listen(EV_CONFIRM, (p) => confirms.push(p as ConfirmPayload));
    const [run] = (await state(b)).runs;
    await b.invoke("set_slot_key", { slot: "binance", field: "access", value: "fresh-key" });
    await b.invoke("set_run_mode", { runId: run.id, mode: "live" });
    expect(await b.invoke("start_run", { runId: run.id })).toEqual({ kind: "checking" });
    await vi.advanceTimersByTimeAsync(1600);
    expect(confirms[0]).toMatchObject({ runId: run.id, title: "바이낸스 실전으로 시작할까요?" });
    expect(((await b.invoke("start_run", { runId: run.id })) as StartReply).kind).toBe("confirm_live");
    expect(await b.invoke("start_run_confirmed", { runId: run.id })).toEqual({ kind: "started" });
  });

  it("연습 키가 없으면 그 칸을 가리키고, 국내 실전은 연결 도우미를 연다", async () => {
    const b = make();
    const [bn, up] = (await state(b)).runs;
    expect(bn.macro.market).toBe("futures");
    await b.invoke("set_run_mode", { runId: bn.id, mode: "testnet" });
    const s = await state(b);
    expect(s.runs[0].keySlot).toBe("binance_demo");
    expect(s.runs[0].readiness.needs).toEqual({ kind: "key", slot: "binance_demo" });
    await b.invoke("set_run_mode", { runId: up.id, mode: "live" });
    expect(await b.invoke("start_run", { runId: up.id })).toEqual({ kind: "wizard" });
    const w = (await state(b)).wizard!;
    expect([w.exchange, w.stage, w.rememberSupported]).toEqual(["upbit", "ask", true]);
    await b.invoke("wizard_choose", { path: "new" });
    await vi.advanceTimersByTimeAsync(1000);
    expect((await state(b)).wizard?.ip.address).toMatch(/^\d+\.\d+\.\d+\.\d+$/);
    await b.invoke("wizard_set_remember", { remember: true });
    expect((await state(b)).settings.slots.find((x) => x.slot === "upbit")?.remembered).toBe(true);
  });

  it("바이낸스 테스트넷 선물은 데모 사이트 도우미를 열고, 허용 IP 단계 없이 키 만들기부터", async () => {
    const b = make();
    const [bn] = (await state(b)).runs;
    expect(bn.macro.market).toBe("futures");
    await b.invoke("set_run_mode", { runId: bn.id, mode: "testnet" });
    expect(await b.invoke("start_run", { runId: bn.id })).toEqual({ kind: "wizard" });
    let w = (await state(b)).wizard!;
    expect([w.slot, w.env, w.label, w.homeLabel]).toEqual(["binance_demo", "demo", "바이낸스 데모", "데모"]);
    await b.invoke("wizard_choose", { path: "new" });
    w = (await state(b)).wizard!;
    expect([w.stage, w.ip.status]).toEqual(["perm", "idle"]);
    await b.invoke("wizard_close");
    await b.invoke("wizard_open", { exchange: "binance", slot: "binance" });
    expect((await state(b)).wizard?.env).toBe("live");
    await b.invoke("wizard_close");
    await b.invoke("wizard_open", { exchange: "binance", slot: "binance_testnet" });
    w = (await state(b)).wizard!;
    expect([w.env, w.market, w.label]).toEqual(["testnet", "spot", "바이낸스 테스트넷"]);
  });

  it("바이낸스 실거래 '처음이에요'의 결과 화면에서 [이전]은 마지막 단계(허용 IP)로", async () => {
    const b = make();
    await b.invoke("wizard_open", { exchange: "binance", slot: "binance" });
    await b.invoke("wizard_choose", { path: "new" });
    await b.invoke("wizard_go", { stage: "result_fail" });
    await b.invoke("wizard_back");
    expect((await state(b)).wizard?.stage).toBe("ip");
  });

  it("키는 실행 중에도 바꿀 수 있고, 쓰는 매크로 수를 알린다", async () => {
    const b = make();
    const [run] = (await state(b)).runs;
    await b.invoke("set_run_mode", { runId: run.id, mode: "live" });
    await b.invoke("start_run", { runId: run.id });
    await b.invoke("start_run_confirmed", { runId: run.id });
    let s = await state(b);
    expect(s.settings.slots[0].inUseBy).toBe(1);
    expect(s.settings.memberInUseBy).toBe(1);
    await b.invoke("set_slot_key", { slot: "binance", field: "access", value: "new-key" });
    expect(await b.invoke("reveal_slot_key", { slot: "binance", field: "access" })).toBe("new-key");
    s = await state(b);
    expect(s.settings.slots[0].status).toBe("unchecked");
  });

  it("업데이트: 확인 → 새 버전 → 내려받기 → 다시 시작(실행 중이면 거절)", async () => {
    const b = make();
    await vi.advanceTimersByTimeAsync(2000);
    let u = (await state(b)).update;
    expect(u.status).toBe("available");
    await b.invoke("update_install");
    expect((await state(b)).update).toMatchObject({ status: "downloading", progress: -1 });
    await vi.advanceTimersByTimeAsync(1100);
    u = (await state(b)).update;
    expect(u.status).toBe("downloading");
    expect(u.progress).toBeGreaterThan(0);
    expect(u.downloaded).toBeGreaterThan(0);
    expect(u.downloaded).toBeLessThan(u.total);
    await vi.advanceTimersByTimeAsync(4000);
    expect((await state(b)).update.status).toBe("ready");
    const [run] = (await state(b)).runs;
    await b.invoke("start_run", { runId: run.id });
    await expect(b.invoke("update_restart")).rejects.toMatch(/지금은 설치할 수 없어요/);
    expect((await state(b)).update.status).toBe("ready");
    expect((await state(make("update=unsupported"))).update.status).toBe("idle");
  });

  it("업데이트: 매크로가 없으면 다시 시작해 설치 → 이전 버전을 알린다 · 디스크 이미지에서는 막는다", async () => {
    const b = make();
    await vi.advanceTimersByTimeAsync(2000);
    await b.invoke("update_install");
    await vi.advanceTimersByTimeAsync(6000);
    await b.invoke("update_restart");
    expect((await state(b)).update.status).toBe("installing");
    await vi.advanceTimersByTimeAsync(2000);
    expect((await state(b)).update).toMatchObject({
      status: "none",
      current: MOCK_NEXT_VERSION,
      updatedFrom: MOCK_VERSION,
    });

    const dmg = make("update=dmg");
    await vi.advanceTimersByTimeAsync(2000);
    expect((await state(dmg)).update.blocked).toMatch(/응용 프로그램 폴더/);
  });
});

describe("mock 백엔드 — 앱 기록", () => {
  beforeEach(() => vi.useFakeTimers());
  afterEach(() => vi.useRealTimers());

  it("켤 때 시작 줄과 읽지 못한 매크로 파일 경고를 남기고, get_app_log는 오래된 것 → 최근 순서", async () => {
    const b = createMockBackend(new URLSearchParams(""));
    const log = (await b.invoke("get_app_log")) as AppLogEntry[];
    expect(log[0]).toMatchObject({ level: "info", source: "app", title: `앱을 시작했어요 — v${MOCK_VERSION}` });
    const warn = log.find((e) => e.level === "warn");
    expect(warn).toMatchObject({ source: "library" });
    expect(warn?.detail).toMatch(/^깨진 파일\.ggm\.json: /);
    expect(log.map((e) => e.id)).toEqual([...log.map((e) => e.id)].sort((a, b) => a - b));
    expect(Object.keys(log[0]).sort()).toEqual(["at", "detail", "id", "level", "source", "title"]);
  });

  it("알림은 앱 기록에 먼저 남고(첫 줄 = 제목) 알림에 logId가 실린다", async () => {
    const b = createMockBackend(new URLSearchParams(""));
    const logs: AppLogEntry[] = [];
    const notices: Notice[] = [];
    await b.listen(EV_APP_LOG, (e) => logs.push(e as AppLogEntry));
    await b.listen(EV_NOTICE, (n) => notices.push(n as Notice));
    await b.invoke("library_open_folder");
    expect(notices).toHaveLength(1);
    expect(logs.at(-1)).toMatchObject({ id: notices[0].logId, level: "info", title: notices[0].message });
    const all = (await b.invoke("get_app_log")) as AppLogEntry[];
    expect(all.at(-1)?.id).toBe(notices[0].logId);
  });

  it("업데이트 확인 실패는 앱 기록에 경고로 남는다", async () => {
    const b = createMockBackend(new URLSearchParams("update=error"));
    await vi.advanceTimersByTimeAsync(3000);
    const log = (await b.invoke("get_app_log")) as AppLogEntry[];
    expect(log.some((e) => e.source === "update" && e.level === "warn")).toBe(true);
  });

  it("앱 기록 복사 줄은 연도 · 날짜 · 수준 · 출처 · 제목 — 설명(줄바꿈은 /)", () => {
    const line = appLogLine({
      id: 1,
      at: new Date(2026, 9, 16, 9, 5, 7).getTime(),
      level: "warn",
      source: "web",
      title: "웹 연결 요청을 확인하지 못했어요.",
      detail: "첫째\n둘째",
    });
    expect(line).toBe("2026-10-16 09:05:07  주의 [web] 웹 연결 요청을 확인하지 못했어요. — 첫째 / 둘째");
  });
});

describe("keyText — 키 입력 칸 값", () => {
  it("앞뒤 공백을 빼고, 숫자는 글자로, null·객체는 빈 글자로", () => {
    expect(keyText("  abc  ")).toBe("abc");
    expect(keyText(123456)).toBe("123456");
    expect(keyText(null)).toBe("");
    expect(keyText(undefined)).toBe("");
    expect(keyText({ k: 1 })).toBe("");
  });
});
