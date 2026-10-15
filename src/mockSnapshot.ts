// mock 모델 → AppState 스냅샷. 시작 준비 판정·같은 거래소·종목 막기·키 칸 상태를 Rust처럼 계산함.
import {
  EXCHANGE_LABELS,
  FIELD_LABELS,
  LIBRARY_DIR,
  MOCK_VERSION,
  SLOT_EXCHANGE,
  SLOT_LABELS,
  SLOT_ORDER,
  LIBRARY_SKIPPED,
  toLibraryFile,
  type MockRun,
  type Model,
} from "./mockData";
import type { AppState, Check, KeySlot, RunSlot, SlotCard, WizardState } from "./types";

const DOMESTIC = ["upbit", "bithumb"];
const ACTIVE_PHASES: RunSlot["phase"][] = ["checking", "starting", "running", "stopping"];

export function isActive(r: MockRun): boolean {
  return ACTIVE_PHASES.includes(r.phase);
}

export function keySlotOf(r: Pick<MockRun, "macro" | "mode">): KeySlot | null {
  const ex = r.macro.exchange;
  if (!ex) return null;
  if (ex === "binance") {
    if (r.mode !== "testnet") return "binance";
    return r.macro.market === "futures" ? "binance_demo" : "binance_testnet";
  }
  return ex;
}

/** 거래소 종목 화면 — runner-core `market_page`와 같은 규칙(미리보기용). */
export function marketPageOf(r: Pick<MockRun, "macro" | "mode">): { label: string; url: string } | null {
  const { exchange, symbol, baseAsset, market } = r.macro;
  const s = symbol.toUpperCase();
  const base = baseAsset.toUpperCase();
  if (!/^[A-Z0-9]{1,20}$/.test(base)) return null;
  if (exchange === "upbit" || exchange === "bithumb") {
    if (s !== `KRW-${base}`) return null;
    return exchange === "upbit"
      ? { label: "업비트에서 보기", url: `https://upbit.com/exchange?code=CRIX.UPBIT.KRW-${base}` }
      : { label: "빗썸에서 보기", url: `https://www.bithumb.com/react/trade/order/${base}-KRW` };
  }
  if (exchange !== "binance") return null;
  const quote = s.startsWith(base) ? s.slice(base.length) : "";
  if (!/^[A-Z0-9]{1,20}$/.test(quote)) return null;
  if (market === "futures" && r.mode === "testnet")
    return { label: "바이낸스 데모에서 보기", url: `https://demo.binance.com/futures/${base}${quote}` };
  if (market === "futures")
    return { label: "바이낸스에서 보기", url: `https://www.binance.com/futures/${base}${quote}` };
  return { label: "바이낸스에서 보기", url: `https://www.binance.com/trade/${base}_${quote}?type=spot` };
}

export function filled(m: Model, slot: KeySlot): boolean {
  return m.keys[slot].access !== "" && m.keys[slot].secret !== "";
}

/** 같은 거래소·같은 종목을 다른 슬롯이 실행 중이면 그 막힘 문구. */
export function conflictMessage(m: Model, r: MockRun): string {
  const other = m.runs.find(
    (o) => o.id !== r.id && isActive(o) && o.macro.exchange === r.macro.exchange && o.macro.symbol === r.macro.symbol,
  );
  if (!other || !r.macro.exchange) return "";
  return `같은 거래소 · 종목(${EXCHANGE_LABELS[r.macro.exchange]} ${r.macro.symbol})은 한 번에 하나만 실행해요. 지금 다른 매크로가 실행 중이에요.`;
}

function readinessOf(m: Model, r: MockRun): RunSlot["readiness"] {
  const idle = r.phase === "idle" || r.phase === "finished";
  const no = (blocker: string, needs: RunSlot["readiness"]["needs"] = null) => ({ canStart: false, blocker, needs });
  if (!r.macro.exchange)
    return no(
      `이 앱이 지원하지 않는 거래소(${r.macro.exchangeName})예요. 바이낸스 · 업비트 · 빗썸 매크로만 실행할 수 있어요.`,
    );
  if (!idle) return no("");
  const slot = keySlotOf(r)!;
  if (r.mode !== "mock" && !filled(m, slot))
    return no(`${SLOT_LABELS[slot]} API 키가 필요해요. 설정에서 입력하세요.`, { kind: "key", slot });
  if (!m.member) return no("회원 키가 필요해요. 설정 › 껄무새 계정에서 입력하세요.", { kind: "member" });
  const conflict = conflictMessage(m, r);
  if (conflict) return no(conflict);
  return { canStart: true, blocker: "", needs: null };
}

function statusOf(m: Model, r: MockRun): RunSlot["status"] {
  switch (r.phase) {
    case "checking":
      return { text: "연결 확인 중", tone: "warn" };
    case "starting":
      return { text: "시작하는 중", tone: "warn" };
    case "stopping":
      return { text: "종료 처리 중", tone: "warn" };
    case "running":
      // Rust degraded()와 같은 차례 — 결과 모름 → 잔고 부족 → 서버 재시도.
      if (r.run?.uncertain) return { text: "포지션 확인 필요", tone: "danger" };
      if (r.run?.fundsShort) return { text: "실행 중 · 잔고 부족 · 주문 못 함", tone: "warn" };
      if (r.run && !r.run.online) return { text: "실행 중 · 서버 재시도 중", tone: "warn" };
      return { text: "실행 중", tone: "ok" };
    default: {
      if (r.run?.finished)
        return r.run.finished.status === "error"
          ? { text: `오류로 끝남`, tone: "danger" }
          : { text: `종료됨 · ${r.run.finished.note}`, tone: "muted" };
      const ready = readinessOf(m, r);
      if (ready.canStart) return { text: "시작 전", tone: "muted" };
      if (ready.needs?.kind === "key") return { text: "키가 필요해요", tone: "warn" };
      if (ready.needs?.kind === "member") return { text: "회원 키가 필요해요", tone: "warn" };
      return { text: "지금은 시작할 수 없어요", tone: "warn" };
    }
  }
}

function checksOf(m: Model, slot: KeySlot): SlotCard["checks"] {
  const k = m.keys[slot];
  const order: Check = slot === "upbit" ? "ok" : "na";
  if (k.status === "connected") return { auth: "ok", ip: "ok", order };
  if (k.status === "blocked" && k.problem === "ip") return { auth: "ok", ip: "bad", order: "wait" };
  if (k.status === "blocked") return { auth: "bad", ip: "wait", order: "wait" };
  return { auth: "wait", ip: "wait", order: "wait" };
}

function headlineOf(m: Model, slot: KeySlot): string {
  const k = m.keys[slot];
  switch (k.status) {
    case "empty":
      return "● 연결 안 됨";
    case "unchecked":
      return "● 확인 전";
    case "checking":
      return "● 확인 중";
    case "connected":
      return "✓ 연결됨";
    case "blocked":
      return k.problem === "ip" ? "● 허용 IP 확인 필요" : "● 연결 안 됨";
  }
}

function noteOf(m: Model, slot: KeySlot): string {
  const k = m.keys[slot];
  if (k.status === "connected" && k.problem === "funds")
    return "주문할 원화 잔고가 모자라요. 잔고를 채우거나 매크로 투자 금액을 줄이세요.";
  if (k.status !== "blocked") return "";
  if (k.problem === "ip")
    return "이 기기 주소가 거래소에 등록된 허용 IP와 달라요. 거래소 API 관리에서 허용 IP를 고치세요.";
  return DOMESTIC.includes(slot)
    ? "키가 맞지 않아요. 다시 복사해 넣어 주세요."
    : "API 키가 거절됐어요. 키 · 시크릿과 IP 제한을 확인하세요.";
}

export function slotCard(m: Model, slot: KeySlot): SlotCard {
  const k = m.keys[slot];
  const ex = SLOT_EXCHANGE[slot];
  return {
    slot,
    exchange: ex,
    label: SLOT_LABELS[slot],
    fieldLabels: FIELD_LABELS[ex],
    accessFilled: k.access !== "",
    secretFilled: k.secret !== "",
    remembered: k.remembered,
    rememberSupported: m.storage.status !== "unsupported",
    status: k.status,
    headline: headlineOf(m, slot),
    note: noteOf(m, slot),
    noteTone: k.status === "blocked" || k.problem === "funds" ? "warn" : "muted",
    checks: checksOf(m, slot),
    wizardAvailable: true,
    inUseBy: m.runs.filter((r) => isActive(r) && r.mode !== "mock" && keySlotOf(r) === slot).length,
  };
}

function syncWizard(m: Model): WizardState | null {
  const w = m.wizard;
  if (!w) return null;
  const k = m.keys[w.slot];
  return {
    ...w,
    slots: { access: k.access !== "", secret: k.secret !== "" },
    keysReady: k.access !== "" && k.secret !== "",
    remembered: k.remembered,
    rememberSupported: m.storage.status !== "unsupported",
  };
}

export function snapshot(m: Model): AppState {
  const active = m.runs.filter(isActive);
  const offline = active.some((r) => r.run && !r.run.online);
  // 잔고 부족은 기다려도 풀리지 않음 — Rust처럼 '확인 필요'로 셈(모의는 빼고).
  const attention = active.filter((r) => r.run?.fundsShort && r.mode !== "mock").length;
  const status: AppState["status"] = active.length
    ? attention
      ? { text: `실행 중 ${active.length}개 · 확인 필요 ${attention}개`, tone: "danger" }
      : {
          text: offline ? `실행 중 ${active.length}개 · 연결 다시 시도하는 중` : `실행 중 ${active.length}개`,
          tone: offline ? "warn" : "ok",
        }
    : m.runs.length
      ? { text: "대기 중", tone: "muted" }
      : { text: "매크로를 추가하세요", tone: "muted" };
  return {
    version: MOCK_VERSION,
    runs: m.runs.map((r) => ({
      id: r.id,
      macro: r.macro,
      mode: r.mode,
      modeChoices: r.macro.exchange === "binance" ? ["mock", "testnet", "live"] : ["mock", "live"],
      phase: r.phase,
      keySlot: keySlotOf(r),
      readiness: readinessOf(m, r),
      run: r.run ? { ...r.run, finished: r.run.finished ? { ...r.run.finished } : null } : null,
      webLinked: r.webLinked,
      marketPage: marketPageOf(r)?.label ?? "",
      webRunPage: Boolean(r.run?.sessionId),
      status: statusOf(m, r),
    })),
    settings: {
      slots: SLOT_ORDER.map((s) => slotCard(m, s)),
      memberKeyFilled: m.member !== "",
      memberKeyPending: m.memberPending ? `…${m.memberPending.slice(-4)}` : "",
      memberInUseBy: active.length,
      storage: m.storage,
      showDevLog: m.showDevLog,
      closeToTray: m.closeToTray,
      serverBase: "https://gg-parrot.onrender.com",
      serverCustom: false,
      os: "macOS 15.6.1 (arm64)",
      publicIp: m.publicIp,
      serverLinkLevel: "19",
    },
    library: {
      dir: LIBRARY_DIR,
      files: m.files.map((f) => toLibraryFile(m, f)),
      skipped: LIBRARY_SKIPPED,
      truncated: 0,
    },
    wizard: syncWizard(m),
    update: { ...m.update },
    status,
    claiming: false,
  };
}
