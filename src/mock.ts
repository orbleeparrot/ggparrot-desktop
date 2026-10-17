// 브라우저 미리보기용 mock 백엔드 — `window.__TAURI_INTERNALS__`가 없을 때만 ipc.ts가 불러옴.
// 계약 v2(types.ts)를 그대로 흉내 냄: 실행 목록·모드·시작/실전 확인·실행 루프와 활동·종료·설정 키 칸·자동 연결 확인·
// 연결 도우미·매크로 폴더·앱 업데이트. 디자인 검토용 바로가기: ?demo=empty | running | many | settings | applog | wizard | library | wait
// | wizard-testnet | wizard-demo | wizard-live(바이낸스 도우미). many: 매크로 24개, 그중 몇 개는 실행 중.
import type { Backend, ConfirmPayload, Unlisten } from "./ipc";
import {
  emptyKey,
  EXCHANGE_LABELS,
  initialModel,
  IMPORT_SAMPLES,
  MOCK_NEXT_VERSION,
  MOCK_VERSION,
  newRun,
  SLOT_EXCHANGE,
  SLOT_LABELS,
  uniqueName,
  type MockRun,
  type Model,
} from "./mockData";
import {
  activityText,
  beginRun,
  holdAt,
  mockEntryWait,
  stopActivities,
  tickRun,
  waitsForEntry,
  type Emit,
} from "./mockActivity";
import { conflictMessage, filled, isActive, keySlotOf, marketPageOf, snapshot } from "./mockSnapshot";
import { createWizard, type WizardTarget } from "./mockWizard";
import { formatClock } from "./state/format";
import { MAX_ACTIVITY } from "./state/activity";
import { SIDE_LABELS } from "./state/runs";
import type {
  Activity,
  EntryPreview,
  AppLogEntry,
  AppLogLevel,
  AppLogSource,
  Exchange,
  KeyField,
  KeySlot,
  Market,
  Notice,
  RunMode,
  StartReply,
  StopMode,
} from "./types";
import { EV_ACTIVITY, EV_APP_LOG, EV_CONFIRM, EV_NOTICE, EV_STATE } from "./types";
import { WEB_URL } from "./platform";

type Handler = (payload: unknown) => void;
type Args = Record<string, unknown>;

export const TICK_MS = 2000;
/** 앱 기록 줄 수 한도(Rust APP_LOG_LIMIT과 같음). */
export const APP_LOG_LIMIT = 500;

const LOG_LEVEL_LABELS: Record<AppLogLevel, string> = { info: "알림", warn: "주의", error: "오류" };

/** 앱 기록 복사 한 줄 — 앱(Rust app_log_line)과 같은 모양. 설명의 줄바꿈은 " / "로 이음. */
export function appLogLine(e: AppLogEntry): string {
  const d = new Date(e.at);
  const p = (n: number) => String(n).padStart(2, "0");
  const t = `${p(d.getMonth() + 1)}-${p(d.getDate())} ${p(d.getHours())}:${p(d.getMinutes())}:${p(d.getSeconds())}`;
  const detail = e.detail
    .split("\n")
    .map((l) => l.trim())
    .filter(Boolean)
    .join(" / ");
  return `${t}  ${LOG_LEVEL_LABELS[e.level]} [${e.source}] ${e.title}${detail ? ` — ${detail}` : ""}`;
}

export function createMockBackend(
  params = new URLSearchParams(globalThis.location?.search ?? ""),
  opts: { dom?: boolean } = {},
): Backend {
  const m: Model = initialModel(params);
  const handlers = new Map<string, Set<Handler>>();
  const activity = new Map<string, Activity[]>();
  const keyTimers = new Map<KeySlot, ReturnType<typeof setTimeout>>();
  let nextActivity = 0;
  const appLog: AppLogEntry[] = [];

  const emit = (event: string, payload: unknown) => handlers.get(event)?.forEach((h) => h(payload));
  const push = () => emit(EV_STATE, snapshot(m));
  /** 앱 기록 한 줄 — 앱처럼 최근 500줄만 두고 runner:app-log로 보냄. */
  const logApp = (
    level: AppLogLevel,
    source: AppLogSource,
    title: string,
    detail = "",
    at = Date.now(),
  ): AppLogEntry => {
    const entry: AppLogEntry = { id: (appLog.at(-1)?.id ?? 0) + 1, at, level, source, title, detail };
    appLog.push(entry);
    if (appLog.length > APP_LOG_LIMIT) appLog.shift();
    emit(EV_APP_LOG, entry);
    return entry;
  };
  /** 알림 — 앱처럼 앱 기록에 먼저 남기고(첫 줄 = 제목) 그 번호를 logId로 실음. */
  const notice = (n: Notice, source: AppLogSource = "app") => {
    const [head = "", ...rest] = n.message.trim().split("\n");
    const entry = logApp(n.level, source, head.trim(), rest.join("\n").trim());
    emit(EV_NOTICE, { ...n, logId: entry.id });
  };
  const info = (message: string, title = "") => notice({ level: "info", title, message });
  const later = (ms: number, fn: () => void) => setTimeout(() => (fn(), push()), ms);
  const findRun = (id: unknown): MockRun => {
    const r = m.runs.find((x) => x.id === id);
    if (!r) throw "목록에 없는 매크로예요.";
    return r;
  };

  const say: Emit = (r, a, at) => {
    const item: Activity = {
      id: ++nextActivity,
      runId: r.id,
      at,
      kind: a.kind,
      title: a.title,
      detail: a.detail ?? "",
      raw: a.raw ?? "",
      dev: a.dev ?? false,
      tone: a.tone ?? "muted",
    };
    const list = activity.get(r.id) ?? [];
    list.push(item);
    if (list.length > MAX_ACTIVITY) list.shift();
    activity.set(r.id, list);
    emit(EV_ACTIVITY, item);
  };

  // 앱 기록 견본 — 켤 때 한 줄, 읽지 못한 매크로 파일 경고.
  {
    const s = snapshot(m).settings;
    logApp("info", "app", `앱을 시작했어요 — v${MOCK_VERSION}`, `${s.os} · 서버 연동 레벨 ${s.serverLinkLevel}`);
    const broken = m.files.filter((f) => !f.macro && f.error);
    if (broken.length)
      logApp(
        "warn",
        "library",
        `매크로 폴더에서 읽지 못한 파일이 있어요 (${broken.length}개)`,
        broken.map((f) => `${f.fileName}: ${f.error}`).join("\n"),
      );
  }

  // 웹 연결로 받은 견본 매크로 — 앱처럼 연결 기록과 매크로 폴더 저장 기록을 남김.
  for (const r of m.runs.filter((x) => x.webLinked && x.fileName)) {
    const at = Date.now();
    logApp("info", "library", `웹 매크로를 매크로 폴더에 저장했어요 — ${r.fileName}`);
    say(
      r,
      {
        kind: "info",
        title: "웹에서 매크로를 연결했어요",
        detail: "모드는 모의로 준비했어요. 확인한 뒤 직접 시작하세요.",
        tone: "ok",
      },
      at,
    );
    say(
      r,
      {
        kind: "info",
        title: `매크로 폴더에 저장했어요 — ${r.fileName}`,
        detail: "실행 목록에서 빼도 파일은 매크로 폴더에 남아요.",
      },
      at,
    );
  }

  // --- 키 칸·자동 연결 확인 --------------------------------------------------
  function scheduleCheck(slot: KeySlot, delay = 700) {
    clearTimeout(keyTimers.get(slot));
    const k = m.keys[slot];
    if (!filled(m, slot)) {
      k.status = "empty";
      return;
    }
    k.status = "unchecked";
    keyTimers.set(
      slot,
      later(delay, () => {
        k.status = "checking";
        keyTimers.set(
          slot,
          later(1100, () => finishCheck(slot)),
        );
      }),
    );
  }

  function finishCheck(slot: KeySlot) {
    const k = m.keys[slot];
    // 'bad'가 들어간 Access는 허용 IP 거절, 'nope'은 키 거절로 흉내 냄.
    const a = k.access.toLowerCase();
    k.problem = a.includes("bad") ? "ip" : a.includes("nope") ? "auth" : "";
    k.status = k.problem ? "blocked" : "connected";
    wizard.onChecked(slot);
    for (const r of m.runs.filter((x) => x.startAfterCheck && keySlotOf(x) === slot)) {
      r.startAfterCheck = false;
      r.phase = "idle";
      if (k.status !== "connected") {
        notice(
          {
            level: "warn",
            title: "시작 전 확인",
            message:
              k.problem === "ip"
                ? "허용 IP 확인이 필요해요. 거래소에 이 기기 주소를 등록한 뒤 다시 시작하세요."
                : "키가 맞지 않아요. 설정에서 키를 확인한 뒤 다시 시작하세요.",
          },
          "run",
        );
      } else if (r.mode === "live") {
        const c = liveConfirm(r);
        emit(EV_CONFIRM, { runId: r.id, title: c.title, message: c.message } satisfies ConfirmPayload);
      } else start(r);
    }
  }

  /** 시작 전 검사·도우미 검사 — 기다리지 않고 바로 확인함. */
  function checkNow(slot: KeySlot) {
    clearTimeout(keyTimers.get(slot));
    m.keys[slot].status = "checking";
    keyTimers.set(
      slot,
      later(1500, () => finishCheck(slot)),
    );
  }

  const wizard = createWizard(m, { later, info, checkNow });

  /** 이 실행으로 연 도우미의 대상 — 실행이 쓰는 칸(keySlotOf)과 그 칸의 사이트. 모의는 키가 필요 없어 없음. */
  function runTarget(r: MockRun): WizardTarget | null {
    const slot = keySlotOf(r);
    if (!slot || r.mode === "mock") return null;
    return slotTarget(SLOT_EXCHANGE[slot], slot, r.macro.market);
  }

  /** 설정 카드에서 연 도우미의 대상 — 칸이 곧 사이트(Rust `KeySlot::binance_env`).
   *  연습 칸은 시장도 칸이 정하고, 실거래 칸은 그 칸을 쓰는 맨 앞 실행(실전 · 모의)의 시장. */
  function slotTarget(exchange: Exchange, slot: KeySlot, market?: Market): WizardTarget {
    if (exchange !== "binance") return { exchange, slot, env: "", market: "spot" };
    if (slot === "binance_testnet") return { exchange, slot, env: "testnet", market: "spot" };
    if (slot === "binance_demo") return { exchange, slot, env: "demo", market: "futures" };
    const first = m.runs.find((r) => keySlotOf(r) === slot)?.macro.market ?? "spot";
    return { exchange, slot, env: "live", market: market ?? first };
  }

  // --- 실행 ---------------------------------------------------------------------
  function liveConfirm(r: MockRun): { kind: "confirm_live"; title: string; message: string } {
    const label = r.macro.exchange ? EXCHANGE_LABELS[r.macro.exchange] : r.macro.exchangeName;
    return {
      kind: "confirm_live",
      title: `${label} 실전으로 시작할까요?`,
      message: `실제 자금으로 주문해요.\n${r.macro.symbol} · ${SIDE_LABELS[r.macro.side]}`,
    };
  }

  const loops = new Map<string, ReturnType<typeof setInterval>>();
  function start(r: MockRun, at = Date.now()): StartReply {
    r.phase = "starting";
    beginRun(say, r, at);
    later(700, () => r.phase === "starting" && goLive(r));
    return { kind: "started" };
  }

  function goLive(r: MockRun) {
    r.phase = "running";
    loops.set(
      r.id,
      setInterval(() => (tickRun(say, r, Date.now()), push()), TICK_MS),
    );
  }

  function stop(r: MockRun, mode: StopMode) {
    if (r.phase !== "running" && r.phase !== "starting") throw "실행 중이 아니에요.";
    r.phase = "stopping";
    say(
      r,
      {
        kind: "stop",
        title: mode === "close_and_stop" ? "청산 후 종료 요청을 보냈어요" : "매크로만 종료 요청을 보냈어요",
        detail:
          mode === "close_and_stop"
            ? "보유 포지션을 시장가로 정리한 뒤 끝내요"
            : "열린 포지션은 그대로 두고 매크로만 멈춰요",
      },
      Date.now(),
    );
    later(1400, () => {
      clearInterval(loops.get(r.id));
      loops.delete(r.id);
      stopActivities(say, r, Date.now(), mode === "close_and_stop");
      r.phase = "finished";
    });
  }

  function addFromMacro(macroIndex: number): string {
    const sample = IMPORT_SAMPLES[macroIndex % IMPORT_SAMPLES.length];
    const fileName = uniqueName(m, `${sample.title}.ggm.json`);
    m.files.unshift({ fileName, macro: sample, error: "", unsupported: "", modifiedAt: Date.now() });
    return fileName;
  }

  // --- 앱 업데이트 ----------------------------------------------------------------
  const updateMode = params.get("update");
  function checkUpdate(quiet: boolean) {
    if (updateMode === "unsupported") {
      m.update.status = "unsupported";
      return;
    }
    if (m.update.status === "downloading" || m.update.status === "ready" || m.update.status === "installing") return;
    m.update.status = "checking";
    // ?update=dmg — macOS 디스크 이미지에서 바로 실행한 경우
    if (updateMode === "dmg")
      m.update.blocked =
        "설치 파일(디스크 이미지)이나 다운로드 폴더에서 바로 실행 중이라 업데이트를 설치할 수 없어요. 앱을 응용 프로그램 폴더로 옮긴 뒤 다시 실행하세요.";
    later(quiet ? 400 : 900, () => {
      m.update.checkedAt = Date.now();
      if (updateMode === "error") {
        Object.assign(m.update, {
          status: "error",
          error: "업데이트 서버에 연결하지 못했어요. 인터넷 연결을 확인한 뒤 다시 확인하세요.",
        });
        logApp(
          "warn",
          "update",
          "업데이트 정보를 확인하지 못했어요",
          `${quiet ? "자동" : "직접"} 확인 · (미리보기) 연결 실패`,
        );
      } else if (updateMode === "0") m.update.status = "none";
      else {
        // 같은 새 버전은 한 번만 남김(앱과 같음).
        if (m.update.version !== MOCK_NEXT_VERSION) logApp("info", "update", `새 버전 v${MOCK_NEXT_VERSION}이 있어요`);
        Object.assign(m.update, {
          status: "available",
          version: MOCK_NEXT_VERSION,
          error: "",
          notes:
            "· 활동 기록 복사에 원문 줄을 함께 넣어요\n· 업비트 허용 IP 안내 문구를 다듬었어요\n· 창을 줄였을 때 목록이 잘리던 문제를 고쳤어요",
        });
      }
    });
  }

  // --- 명령 -------------------------------------------------------------------------
  const commands: Record<string, (args: Args) => unknown> = {
    get_state: () => snapshot(m),
    get_activity: ({ runId }) => (activity.get(String(runId)) ?? []).slice(),
    add_macro_file: () => {
      const fileName = addFromMacro(m.imported++);
      const f = m.files.find((x) => x.fileName === fileName)!;
      return newRun(m, f.macro!, fileName).id;
    },
    add_run_from_library: ({ fileName }) => {
      const f = m.files.find((x) => x.fileName === fileName);
      if (!f) throw "매크로 폴더에서 그 파일을 찾지 못했어요. [새로고침]을 눌러 보세요.";
      if (!f.macro) throw `이 파일은 매크로로 읽지 못했어요. ${f.error}`;
      if (f.unsupported) throw f.unsupported;
      return newRun(m, f.macro, f.fileName).id;
    },
    set_run_capital: ({ runId, amount }) => {
      const r = findRun(runId);
      if (isActive(r)) throw "실행 중에는 투자 금액을 바꿀 수 없어요. 종료한 뒤 바꾸세요.";
      if (r.macro.capitalLocked) throw r.macro.capitalLocked;
      const original = r.macro.capitalOriginal ?? r.macro.capital;
      const next = amount === null || amount === undefined ? original : Number(amount);
      if (next === null || !Number.isFinite(next) || next <= 0) throw "투자 금액은 0보다 커야 해요.";
      r.macro = {
        ...r.macro,
        capital: next,
        capitalOriginal: next === original ? null : original,
        signature: next === original ? (r.macro.signature === "modified" ? "signed" : r.macro.signature) : "modified",
      };
    },
    move_run: ({ runId, index }) => {
      const from = m.runs.findIndex((r) => r.id === runId);
      if (from < 0) return;
      const [r] = m.runs.splice(from, 1);
      m.runs.splice(Math.max(0, Math.min(Number(index), m.runs.length)), 0, r);
    },
    remove_run: ({ runId }) => {
      const r = findRun(runId);
      if (isActive(r)) throw "실행 중인 매크로는 목록에서 뺄 수 없어요. 먼저 종료하세요.";
      m.runs = m.runs.filter((x) => x !== r);
      activity.delete(r.id);
    },
    set_run_mode: ({ runId, mode }) => {
      const r = findRun(runId);
      if (isActive(r)) throw "실행 중에는 모드를 바꿀 수 없어요.";
      const next = mode as RunMode;
      r.mode = next === "testnet" && r.macro.exchange !== "binance" ? "mock" : next;
    },
    start_run: ({ runId }): StartReply => {
      const r = findRun(runId);
      if (m.wizard) return { kind: "blocked", message: "연결 도우미를 닫은 뒤 다시 시작하세요." };
      if (isActive(r)) return { kind: "blocked", message: "이미 실행 중이에요." };
      if (!r.macro.exchange)
        return {
          kind: "blocked",
          message: `이 앱이 지원하지 않는 거래소(${r.macro.exchangeName})예요. 바이낸스 · 업비트 · 빗썸 매크로만 실행할 수 있어요.`,
        };
      const conflict = conflictMessage(m, r);
      if (conflict) return { kind: "blocked", message: conflict };
      const slot = keySlotOf(r)!;
      if (r.mode !== "mock" && !filled(m, slot)) {
        const target = runTarget(r);
        if (target) {
          wizard.open(target, "ask");
          return { kind: "wizard" };
        }
        const label = SLOT_LABELS[slot];
        return {
          kind: "blocked",
          message: `이 매크로를 시작하려면 ${label} API 키가 필요해요.\n\n설정 › 거래소 연결에서 입력하세요.`,
        };
      }
      if (!m.member) return { kind: "blocked", message: "회원 키가 필요해요. 설정 › 껄무새 계정에서 입력하세요." };
      if (r.mode !== "mock" && m.keys[slot].status !== "connected") {
        r.startAfterCheck = true;
        r.phase = "checking";
        checkNow(slot);
        return { kind: "checking" };
      }
      if (r.mode === "live") return liveConfirm(r);
      return start(r);
    },
    start_run_confirmed: ({ runId }): StartReply => {
      const r = findRun(runId);
      const conflict = conflictMessage(m, r);
      if (conflict) return { kind: "blocked", message: conflict };
      if (isActive(r)) return { kind: "blocked", message: "이미 실행 중이에요." };
      return start(r);
    },
    entry_preview: ({ runId }): EntryPreview => {
      const r = findRun(runId);
      if (!waitsForEntry(r.macro.strategy)) return { waiting: false };
      const w = mockEntryWait(Date.now(), 0);
      return { waiting: true, sentence: w.sentence, typical: w.typical, interval_label: w.interval_label };
    },
    set_entry_notify: () => undefined,
    stop_run: ({ runId, mode }) => stop(findRun(runId), mode as StopMode),
    copy_activity: ({ runId }) => {
      const text = activityText(activity.get(String(runId)) ?? [], formatClock);
      void globalThis.navigator?.clipboard?.writeText(text).catch(() => undefined);
    },
    refresh_public_ip: ({ force }) => {
      if (m.publicIp.status === "ok" && !force) return;
      m.publicIp = { status: "loading", address: "" };
      later(700, () => {
        m.publicIp = { status: "ok", address: "211.234.101.57" };
      });
    },
    copy_support_info: () => {
      const s = snapshot(m).settings;
      const recent = appLog.slice(-20).map((e) => `- ${appLogLine(e)}`);
      const text = [
        "[껄무새 문의용 정보] (미리보기)",
        `앱 버전: v${MOCK_VERSION}`,
        `서버 연동 레벨: ${s.serverLinkLevel}`,
        `OS: ${s.os}`,
        `공인 IP: ${s.publicIp.address || "확인 전"}`,
        ...(recent.length ? ["최근 앱 기록:", ...recent] : []),
      ].join("\n");
      void globalThis.navigator?.clipboard?.writeText(text).catch(() => undefined);
    },
    get_app_log: () => appLog.slice(),
    copy_app_log: () => {
      const s = snapshot(m).settings;
      const text = [`[껄무새 앱 기록] v${MOCK_VERSION} · ${s.os} (미리보기)`, ...appLog.map(appLogLine)].join("\n");
      void globalThis.navigator?.clipboard?.writeText(text).catch(() => undefined);
    },
    open_market_page: ({ runId }) => {
      const page = marketPageOf(findRun(runId));
      if (page) window.open(page.url, "_blank", "noopener");
    },
    open_run_web_page: ({ runId }) => {
      const sid = findRun(runId).run?.sessionId;
      if (sid) window.open(`${WEB_URL}agents?session=${sid}`, "_blank", "noopener");
    },
    set_slot_key: ({ slot, field, value }) => {
      const s = slot as KeySlot;
      m.keys[s][field as KeyField] = String(value ?? "").trim();
      scheduleCheck(s);
    },
    reveal_slot_key: ({ slot, field }) => m.keys[slot as KeySlot][field as KeyField],
    set_slot_remember: ({ slot, remember }) => {
      m.keys[slot as KeySlot].remembered = Boolean(remember);
    },
    check_slot: ({ slot }) => scheduleCheck(slot as KeySlot, 0),
    set_member_key: ({ value }) => {
      m.member = String(value ?? "").trim();
    },
    reveal_member_key: () => m.member,
    resolve_member_key: ({ accept }) => {
      if (accept && m.memberPending) m.member = m.memberPending;
      m.memberPending = "";
    },
    set_close_to_tray: ({ value }) => {
      m.closeToTray = Boolean(value);
    },
    set_show_dev_log: ({ show }) => {
      m.showDevLog = Boolean(show);
    },
    recover_storage: () => {
      m.storage = { ...m.storage, status: "missing", note: "" };
    },
    reload_storage: () => {
      m.storage = { ...m.storage, status: "loaded", note: "" };
    },
    update_check: () => checkUpdate(false),
    update_install: () => {
      if (m.update.status !== "available") throw "설치할 새 버전이 없어요.";
      const total = 14_680_064;
      Object.assign(m.update, { status: "downloading", progress: -1, downloaded: 0, total });
      const step = () =>
        later(320, () => {
          if (m.update.status !== "downloading") return;
          m.update.progress = m.update.progress < 0 ? 0.08 : Math.min(1, m.update.progress + 0.09);
          m.update.downloaded = Math.round(total * m.update.progress);
          if (m.update.progress >= 1) Object.assign(m.update, { status: "ready", progress: 1 });
          else step();
        });
      later(700, step);
    },
    update_restart: () => {
      if (m.update.status !== "ready") throw "먼저 업데이트를 내려받으세요.";
      const n = m.runs.filter(isActive).length;
      if (n > 0) throw `실행 중인 매크로가 ${n}개 있어 지금은 설치할 수 없어요. 모두 종료한 뒤 다시 누르세요.`;
      m.update.status = "installing";
      later(1600, () =>
        Object.assign(m.update, {
          status: "none",
          updatedFrom: m.update.current,
          current: m.update.version,
          version: "",
          notes: "",
          checkedAt: Date.now(),
        }),
      );
    },
    library_refresh: () => undefined,
    library_open_folder: () => info("(미리보기) Finder에서 매크로 폴더를 열었어요.", "매크로 폴더"),
    open_web: () => {
      window.open(WEB_URL, "_blank", "noopener");
    },
    library_import: () => {
      addFromMacro(m.imported++);
      return 1;
    },
    ...wizard.commands,
    wizard_open: ({ exchange, slot, stage }) => {
      const ex = exchange as Exchange;
      const s = (slot as KeySlot | undefined) ?? (ex === "binance" ? "binance" : ex);
      wizard.open(slotTarget(ex, s), (stage as Parameters<typeof wizard.open>[1]) || "ask");
    },
  };

  // 창에 끌어다 놓기(브라우저 미리보기에서만) — Rust처럼 폴더로 가져오고 알림을 냄.
  if (opts.dom && typeof window !== "undefined") {
    window.addEventListener("drop", (e) => {
      const names = Array.from(e.dataTransfer?.files ?? [])
        .map((f) => f.name)
        .filter((n) => /\.json$/i.test(n));
      if (names.length === 0) return;
      for (let i = 0; i < names.length; i += 1) addFromMacro(m.imported++);
      info(`매크로 파일 ${names.length}개를 매크로 폴더로 가져왔어요.`, "매크로 폴더");
      push();
    });
  }

  // 켤 때 한 번 조용히 업데이트를 확인함.
  later(1200, () => checkUpdate(true));

  // 디자인 검토용 바로가기
  const demo = params.get("demo");
  if (demo === "running") seedRunning();
  if (demo === "pnl") seedPnl();
  if (demo === "wait") seedWait();
  if (demo === "settings") seedSettings();
  if (demo === "many") seedMany();
  if (demo === "wizard") wizard.open(slotTarget("upbit", "upbit"), "ask");
  if (demo === "applog") seedAppLog();
  // 바이낸스 도우미: ?demo=wizard-testnet | wizard-demo | wizard-live
  const site = /^wizard-(testnet|demo|live)$/.exec(demo ?? "")?.[1];
  if (site) {
    // 처음 만드는 길을 보이도록 그 칸은 비워 둠(견본은 실거래 칸에 키가 있음).
    const sites: Record<string, KeySlot> = { testnet: "binance_testnet", demo: "binance_demo", live: "binance" };
    const slot = sites[site];
    m.keys[slot] = emptyKey();
    wizard.open(slotTarget("binance", slot), "ask");
  }

  function seedRunning() {
    const now = Date.now();
    const [a, b] = m.runs;
    if (!a || !b) return;
    // 실전 배지가 보이도록 업비트 매크로는 실전으로(키도 연결된 상태로) 돌림.
    m.keys.upbit = {
      access: "upAccessDemo0000",
      secret: "upSecretDemo0000",
      remembered: true,
      status: "connected",
      problem: "",
    };
    b.mode = "live";
    for (const [r, minutes, ticks] of [
      [a, 14, 40],
      [b, 6, 14],
    ] as const) {
      const t0 = now - minutes * 60_000;
      start(r, t0);
      goLive(r);
      for (let i = 1; i <= ticks; i += 1) tickRun(say, r, t0 + (i * minutes * 60_000) / (ticks + 1));
      if (r.run) r.run.online = true;
    }
  }

  /** ?demo=wait — 진입 대기 검토용. 2026-10-17 빗썸 KRW-FORT 변동성 돌파(15분 · 20봉 평균 필터) 10:40 그대로:
   *  고가가 사는 선을 넘었지만 마감가가 20봉 평균 아래라 아직 안 삼(서버 entry_wait.py 계산 값). */
  function seedWait() {
    const now = Date.now();
    const [, upbit] = m.runs;
    const fort = newRun(
      m,
      {
        ...upbit.macro,
        title: "FORT 변동성 돌파",
        exchange: "bithumb",
        exchangeName: "bithumb",
        exchangeLabel: "빗썸",
        symbol: "KRW-FORT",
        baseAsset: "FORT",
        strategy: "변동성 돌파",
        summary: "15분 봉 변동폭의 0.6배를 넘어 오르고 20봉 평균 위에서 마감하면 사요.",
        exits: {
          sl: null,
          tp: null,
          trailing: { activation: 0, trail: 2, label: "고점 -2%" },
          kind: "two",
          summary: "트레일링 고점 -2%",
        },
      },
      "FORT 변동성 돌파.ggm.json",
    );
    fort.mode = "mock";
    fort.fixedWait = {
      rule: "I",
      interval: "15m",
      interval_label: "15분",
      realtime: false,
      sentence: "26.523 위에서 끝나면 사요",
      headline: "26.523 위에서 끝나면 사요",
      distance: "지금 26.2 · +1.2% 더 · 사는 선은 넘었고 20봉 평균선이 남았어요",
      gap: "평균선 −1.2%",
      remain: "",
      touched: true,
      reached: false,
      conditions: [
        { key: "line", label: "사는 선 26.118 넘기", now: "넘음 · 고가 26.26", ok: true, at_close: false, wait: "" },
        {
          key: "ma",
          label: "20봉 평균 26.523 위에서 끝나기",
          now: "−1.2%",
          ok: false,
          at_close: true,
          wait: "20봉 평균 26.523 위에서 끝나야 사요",
        },
      ],
      last_bar: { at_ms: now - 10 * 60_000, entered: false, why: "고가 26.12가 사는 선 26.336에 못 미쳤어요" },
      meter: {
        from: { label: "이 봉 시가", value: "26.01" },
        to: { label: "사는 선", value: "26.118" },
        now: "26.2",
        progress: 1,
      },
      ladder: {
        marks: [
          { key: "line", label: "사는 선", value: "26.118", pos: 0.2666, main: false },
          { key: "ma", label: "20봉 평균", value: "26.523", pos: 0.9032, main: true },
        ],
        now: { label: "지금", value: "26.2", pos: 0.3955 },
        zone: 0.9032,
      },
      buy_line: { price: 26.118, label: "사는 선 26.118" },
      next_check_ms: Math.ceil(now / 900_000) * 900_000,
      checks: 1,
      last_check_ms: null,
      typical: { episodes: 30, span_ms: 177_300_000, avg_ms: 5_910_000, longest_ms: 42_300_000 },
      log: [],
    };
    beginRun(say, fort, now - 3 * 60_000);
    goLive(fort);
    tickRun(say, fort, now);
  }

  /** ?demo=pnl — 손익·청산 기준 막대 검토용. 정해진 평단·현재가로 보유 중인 매크로 여섯 개(신호 없이 시세만 흔듦).
   *  업비트 원화 현물(실전·수수료 반영, 수량 0.00127) · 바이낸스 선물 롱 트레일링(실전·수수료 모름) · 선물 숏(모의) ·
   *  바이낸스 현물 USDT 익절만(+0.15% — 수수료 반영하면 손실) · 빗썸 손절만(익절은 신호) ·
   *  업비트 싼 코인(KRW-ZAMA, 수량 499.75012494 — 긴 수량이 잘리지 않는지). */
  function seedPnl() {
    const now = Date.now();
    m.keys.upbit = {
      access: "upAccessDemo0000",
      secret: "upSecretDemo0000",
      remembered: true,
      status: "connected",
      problem: "",
    };
    const byTitle = (title: string) => {
      const existing = m.runs.find((r) => r.macro.title === title);
      if (existing) return existing;
      const f = m.files.find((x) => x.macro?.title === title)!;
      return newRun(m, f.macro!, f.fileName);
    };
    const [futures, upbit] = m.runs;
    const zama = newRun(
      m,
      {
        ...upbit.macro,
        title: "ZAMA 단타",
        symbol: "KRW-ZAMA",
        baseAsset: "ZAMA",
        capital: 100_000,
        capitalOriginal: null,
      },
      "ZAMA 단타.ggm.json",
    );
    const plan: [MockRun, RunMode, number, number, number?][] = [
      [upbit, "live", 118_000_000, 118_590_000, 0.00127],
      [byTitle("BTC 단타 (내 수정)"), "mock", 81_806, 81_928.71],
      [futures, "live", 81_000, 82_900],
      [byTitle("ETH 숏 연습"), "mock", 3_150, 3_100.4],
      [byTitle("빗썸 이더 단타"), "mock", 4_812_000, 4_790_000],
      [zama, "mock", 200.1, 201.3, 499.75012494],
    ];
    for (const [r, mode, entry, price, qty] of plan) {
      r.mode = mode;
      beginRun(say, r, now - 9 * 60_000);
      goLive(r);
      holdAt(r, entry, price, qty);
    }
  }

  /** ?demo=settings — 설정 검토용. 업비트는 연결됐지만 원화 잔고가 모자란 상태(주의 안내),
   *  바이낸스 테스트넷 · 데모는 비어 있어 한 줄로 접힘. */
  function seedSettings() {
    m.keys.upbit = {
      access: "upAccessDemo0000",
      secret: "upSecretDemo0000",
      remembered: true,
      status: "connected",
      problem: "funds",
    };
  }

  /** ?demo=applog — 설정 › 앱 기록을 열고, 여러 출처·수준의 줄을 채운 뒤 [자세히]가 달린 알림 토스트를 띄움. */
  function seedAppLog() {
    const now = Date.now();
    // 켤 때 남긴 줄을 견본보다 앞선 시각으로 — 번호 순서와 시각 순서를 맞춤.
    for (const e of appLog) e.at = now - 45 * 60_000;
    const rows: [AppLogLevel, AppLogSource, string, string, number][] = [
      ["info", "web", "껄무새 웹과 연결했어요", "", 41],
      ["info", "keys", "회원 키를 웹 연결에서 받았어요", "", 40],
      [
        "warn",
        "server",
        "서버 응답이 늦어요 — 다시 시도했어요",
        "GET /api/runner/session · 8.2초\n두 번째 시도에서 응답을 받았어요.",
        33,
      ],
      ["info", "wizard", "연결 도우미를 열었어요 — 바이낸스 테스트넷", "", 27],
      ["info", "keys", "바이낸스 테스트넷 키를 확인했어요", "읽기 ✓ · 거래 ✓ · 출금 꺼짐 ✓", 26],
      [
        "error",
        "run",
        "시작하지 못했어요 — 업비트 KRW-BTC",
        "업비트가 이 IP(211.234.101.57)를 허용하지 않아요.\n업비트 Open API 관리에서 허용 IP에 추가하세요.",
        18,
      ],
      ["warn", "storage", "기억한 키를 저장하지 못했어요", "키체인이 잠겨 있어요. 이번 실행 동안만 기억해요.", 12],
      ["info", "web", "껄무새 웹과 연결이 끊겼어요 — 다시 연결하는 중", "", 6],
      ["info", "web", "껄무새 웹과 다시 연결했어요", "", 5],
    ];
    for (const [level, source, title, detail, minutes] of rows)
      logApp(level, source, title, detail, now - minutes * 60_000 + Math.floor(Math.random() * 50_000));
    later(1500, () =>
      notice(
        {
          level: "info",
          title: "매크로 폴더",
          message: "매크로 파일 2개를 매크로 폴더로 가져왔어요.",
        },
        "library",
      ),
    );
  }

  /** 긴 목록 사이사이에 실행 중 매크로를 둠 — 스크롤한 뒤에도 실행 중 카드가 눈에 띄는지 보려고. */
  /** ?demo=many — 24개 중 몇 개를 실행 중으로. 앞쪽은 카드 검토용: 긴 종목(1000SHIBUSDT 실행 중·테스트넷·보유 중),
   *  BTCDOMUSDT(대기·테스트넷), KRW-SHIB(대기·실전), 서버 재시도 중인 카드. */
  function seedMany() {
    const now = Date.now();
    const [shib, dom, krwShib] = m.runs.slice(2, 5);
    if (shib) shib.mode = "testnet";
    if (dom) dom.mode = "testnet";
    if (krwShib) krwShib.mode = "live";
    for (const i of [0, 2, 5, 11, 17, 22]) {
      const r = m.runs[i];
      if (!r || isActive(r)) continue;
      const t0 = now - (i + 3) * 60_000;
      start(r, t0);
      goLive(r);
      for (let k = 1; k <= 6; k += 1) tickRun(say, r, t0 + k * 20_000);
      if (r.run) r.run.online = true;
    }
    if (shib?.run) holdAt(shib, 100, 101.37);
    const retrying = m.runs[5];
    if (retrying?.run) {
      retrying.offline = true;
      Object.assign(retrying.run, { online: false, offlineSince: now - 90_000 });
    }
    // 잔고 부족으로 진입 주문이 거절된 실전 실행('실행 중 · 잔고 부족 · 주문 못 함').
    const short = m.runs[11];
    if (short?.run) {
      short.mode = "live";
      short.run.fundsShort = true;
    }
  }

  return {
    async invoke(cmd, args = {}) {
      const fn = commands[cmd];
      if (!fn) throw `모르는 명령이에요: ${cmd}`;
      const result = fn(args);
      if (!cmd.startsWith("get_") && !cmd.startsWith("reveal_")) queueMicrotask(push);
      return result;
    },
    async listen(event, handler): Promise<Unlisten> {
      if (!handlers.has(event)) handlers.set(event, new Set());
      handlers.get(event)!.add(handler);
      return () => handlers.get(event)?.delete(handler);
    },
  };
}
