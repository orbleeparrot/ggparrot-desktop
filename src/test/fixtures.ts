// 테스트용 상태 스냅샷 만들기(계약 v2).
import type { Activity, AppState, KeySlot, MacroInfo, RunSlot, RunStatus, SettingsState, SlotCard } from "../types";

export const binanceMacro: MacroInfo = {
  title: "BTC 추세.ggm.json",
  source: "file",
  exchangeName: "binance",
  exchange: "binance",
  exchangeLabel: "바이낸스",
  symbol: "BTCUSDT",
  baseAsset: "BTC",
  quote: "USDT",
  market: "futures",
  side: "long",
  leverage: 3,
  summary: "",
  signature: "signed",
  strategy: "RSI 조건 매매",
  capital: 500,
  capitalOriginal: null,
  capitalLocked: "",
  orderCap: 100,
  risk: ["손절 3%"],
};

export const upbitMacro: MacroInfo = {
  ...binanceMacro,
  title: "업비트 비트코인 눌림목",
  source: "web",
  exchangeName: "upbit",
  exchange: "upbit",
  exchangeLabel: "업비트",
  symbol: "KRW-BTC",
  quote: "KRW",
  market: "spot",
  leverage: 1,
  signature: "web",
  capital: 500_000,
  capitalLocked: "웹에서 연결한 매크로는 웹에서 고친 뒤 다시 연결하세요.",
  orderCap: 150_000,
};

export function makeSlot(over: Partial<RunSlot> = {}): RunSlot {
  return {
    id: "run-1",
    macro: binanceMacro,
    mode: "mock",
    modeChoices: ["mock", "testnet", "live"],
    phase: "idle",
    keySlot: "binance",
    readiness: { canStart: true, blocker: "", needs: null },
    run: null,
    webLinked: false,
    marketPage: "바이낸스에서 보기",
    webRunPage: false,
    status: { text: "시작 전", tone: "muted" },
    ...over,
  };
}

const LABELS: Record<KeySlot, string> = {
  binance: "바이낸스",
  binance_testnet: "바이낸스 테스트넷",
  binance_demo: "바이낸스 데모",
  upbit: "업비트",
  bithumb: "빗썸",
};

export function makeRun(over: Partial<RunStatus> = {}): RunStatus {
  return {
    startedAt: Date.now() - 60_000,
    sessionId: 1,
    online: true,
    authFailed: false,
    rejected: 0,
    offlineSince: 0,
    priceStaleSince: 0,
    closeFailures: 0,
    updatedAt: Date.now(),
    price: 100,
    inPosition: false,
    uncertain: false,
    entryPrice: 0,
    qty: 0,
    unrealizedPct: 0,
    unrealized: 0,
    realized: 0,
    fees: null,
    dayChangePct: null,
    macroOrigin: "",
    note: "",
    finished: null,
    ...over,
  };
}

export function slotCard(slot: KeySlot, over: Partial<SlotCard> = {}): SlotCard {
  const exchange = slot === "binance_testnet" || slot === "binance_demo" ? "binance" : slot;
  return {
    slot,
    exchange,
    label: LABELS[slot],
    fieldLabels: exchange === "upbit" ? ["Access Key", "Secret Key"] : ["API Key", "Secret Key"],
    accessFilled: false,
    secretFilled: false,
    remembered: false,
    rememberSupported: true,
    status: "empty",
    headline: "● 연결 안 됨",
    note: "",
    checks: { auth: "wait", ip: "wait", order: "wait" },
    wizardAvailable: exchange !== "binance",
    inUseBy: 0,
    ...over,
  };
}

export function makeSettings(over: Partial<SettingsState> = {}): SettingsState {
  return {
    slots: (["binance", "binance_testnet", "binance_demo", "upbit", "bithumb"] as const).map((s) => slotCard(s)),
    memberKeyFilled: true,
    memberKeyPending: "",
    memberInUseBy: 0,
    storage: { status: "loaded", note: "" },
    showDevLog: false,
    closeToTray: true,
    serverBase: "https://api.example.com",
    serverCustom: true,
    os: "macOS 15.6.1 (arm64)",
    publicIp: { status: "ok", address: "203.0.113.7" },
    serverLinkLevel: "19",
    ...over,
  };
}

export function makeState(over: Partial<AppState> = {}): AppState {
  return {
    version: "0.18.3",
    runs: [makeSlot()],
    settings: makeSettings(),
    library: {
      dir: "/Users/me/Library/Application Support/ggparrot-desktop/macros",
      files: [],
      skipped: 0,
      truncated: 0,
    },
    wizard: null,
    update: {
      status: "none",
      current: "0.18.3",
      version: "",
      notes: "",
      progress: -1,
      downloaded: 0,
      total: 0,
      error: "",
      checkedAt: 0,
      blocked: "",
      updatedFrom: "",
    },
    status: { text: "대기 중", tone: "muted" },
    claiming: false,
    ...over,
  };
}

let nextId = 1;
export function act(over: Partial<Activity> = {}): Activity {
  return {
    id: nextId++,
    runId: "run-1",
    at: Date.now(),
    kind: "info",
    title: "안내",
    detail: "",
    raw: "",
    dev: false,
    tone: "muted",
    ...over,
  };
}
