// 브라우저 미리보기용 mock 백엔드의 데이터 — 견본 매크로·매크로 폴더·모델. Rust의 규칙을 흉내만 냄(실제 규칙은 Rust가 소유).
import type {
  Exchange,
  KeySlot,
  LibraryFile,
  MacroInfo,
  RunMode,
  RunSlot,
  RunStatus,
  SettingsState,
  SlotCard,
  UpdateState,
  WizardState,
} from "./types";
import cargoToml from "../src-tauri/Cargo.toml?raw";

/** 앱 버전 — Cargo 워크스페이스 한 곳에서 읽음(견본에 버전을 따로 적지 않게). */
export const MOCK_VERSION = /^\[workspace\.package\][^[]*?^version\s*=\s*"([^"]+)"/ms.exec(cargoToml)?.[1] ?? "0.0.0";
/** 견본 업데이트가 내놓는 '새 버전' — 지금 버전의 다음 패치. */
export const MOCK_NEXT_VERSION = MOCK_VERSION.replace(/(\d+)$/, (n) => String(Number(n) + 1));
export const EXCHANGE_LABELS: Record<Exchange, string> = { binance: "바이낸스", upbit: "업비트", bithumb: "빗썸" };
export const SLOT_LABELS: Record<KeySlot, string> = {
  binance: "바이낸스",
  binance_testnet: "바이낸스 테스트넷",
  binance_demo: "바이낸스 데모",
  upbit: "업비트",
  bithumb: "빗썸",
};
export const SLOT_EXCHANGE: Record<KeySlot, Exchange> = {
  binance: "binance",
  binance_testnet: "binance",
  binance_demo: "binance",
  upbit: "upbit",
  bithumb: "bithumb",
};
export const FIELD_LABELS: Record<Exchange, [string, string]> = {
  binance: ["API Key", "Secret Key"],
  upbit: ["Access Key", "Secret Key"],
  bithumb: ["API Key", "Secret Key"],
};
export const SLOT_ORDER: KeySlot[] = ["binance", "binance_testnet", "binance_demo", "upbit", "bithumb"];
export const LIBRARY_DIR = "/Users/me/Library/Application Support/ggparrot-desktop/macros";

function macro(over: Partial<MacroInfo> & Pick<MacroInfo, "title" | "exchange" | "symbol">): MacroInfo {
  const ex = over.exchange;
  const quote = ex === "binance" ? "USDT" : "KRW";
  return {
    source: "file",
    exchangeName: ex ?? "kucoin",
    exchangeLabel: ex ? EXCHANGE_LABELS[ex] : "",
    baseAsset: over.symbol.replace(/USDT$|^KRW-|_KRW$/g, ""),
    quote,
    market: "spot",
    side: "long",
    leverage: 1,
    summary: "",
    signature: "signed",
    strategy: "RSI 조건 매매",
    capital: quote === "USDT" ? 500 : 500_000,
    capitalOriginal: null,
    capitalLocked: over.source === "web" ? "웹에서 연결한 매크로는 웹에서 고친 뒤 다시 연결하세요." : "",
    orderCap: quote === "USDT" ? 100 : 150_000,
    risk: ["손절 3%", "하루 최대 손실 5%"],
    ...over,
  };
}

const WEB_MACRO = macro({
  title: "업비트 비트코인 눌림목",
  source: "web",
  exchange: "upbit",
  symbol: "KRW-BTC",
  summary: "1시간 봉 20일선 아래로 3% 눌리면 나눠 사고, 5% 오르면 정리해요.",
  signature: "web",
});

/** 웹 [실행기 열기]로 받은 견본 매크로의 파일 — 앱은 받은 매크로를 웹 내려받기와 같은 이름으로 매크로 폴더에 저장하고 그 파일과 이음. */
export const WEB_FILE_NAME = "upbit_KRW-BTC_rsi_long_20261015.ggm.json";

/** 매크로 폴더에 처음부터 있는 파일. macro가 null이면 읽히지 않는 파일. */
const LIBRARY_SAMPLES: {
  fileName: string;
  macro: MacroInfo | null;
  error?: string;
  unsupported?: string;
  ageMin: number;
}[] = [
  {
    fileName: "BTC 추세 따라가기.ggm.json",
    ageMin: 60 * 26,
    macro: macro({
      title: "BTC 추세 따라가기",
      exchange: "binance",
      symbol: "BTCUSDT",
      market: "futures",
      leverage: 3,
      summary: "4시간 봉 RSI가 30 아래에서 올라오면 사고, 70 위에서 꺾이면 팔아요.",
    }),
  },
  {
    fileName: "빗썸 이더 단타.ggm.json",
    ageMin: 42,
    macro: macro({
      title: "빗썸 이더 단타",
      exchange: "bithumb",
      symbol: "ETH_KRW",
      summary: "15분 봉 볼린저 하단에서 사고 중앙선에서 팔아요.",
    }),
  },
  {
    fileName: "ETH 숏 연습.ggm.json",
    ageMin: 60 * 24 * 6,
    macro: macro({
      title: "ETH 숏 연습",
      exchange: "binance",
      symbol: "ETHUSDT",
      market: "futures",
      side: "short",
      leverage: 2,
      signature: "unsigned",
    }),
  },
  {
    fileName: "BTC 단타 (내 수정).ggm.json",
    ageMin: 8,
    macro: macro({
      title: "BTC 단타 (내 수정)",
      exchange: "binance",
      symbol: "BTCUSDT",
      signature: "modified",
      summary: "5분 봉 MACD 교차로 짧게 사고팔아요.",
    }),
  },
  {
    fileName: "binance_BTCUSDT_dca_long_20261012.ggm.json",
    ageMin: 60 * 30,
    macro: macro({
      title: "binance_BTCUSDT_dca_long_20261012",
      exchange: "binance",
      symbol: "BTCUSDT",
      strategy: "정기 분할매수(DCA)",
    }),
    unsupported: "이 앱은 아직 이 매크로 유형(정기 분할매수 · 하락 방어 전환)을 지원하지 않아요.",
  },
  { fileName: "깨진 파일.ggm.json", macro: null, error: "올바른 껄무새 매크로 파일이 아니에요.", ageMin: 60 * 3 },
  {
    // 웹 연결로 받아 저장한 파일 — 서명은 없음(웹 연결 응답에는 서명이 실려 오지 않음).
    fileName: WEB_FILE_NAME,
    ageMin: 5,
    macro: {
      ...WEB_MACRO,
      title: WEB_FILE_NAME.replace(/\.ggm\.json$/, ""),
      source: "file",
      signature: "unsigned",
      capitalLocked: "",
    },
  },
];

/** 매크로 형식 이름(.ggm.json)이 아니라 목록에서 뺀 .json 수(견본: 메모.json). */
export const LIBRARY_SKIPPED = 1;

/** [골라서 실행 목록에 추가…]·[폴더로 가져오기]·끌어다 놓기로 들어오는 견본. */
export const IMPORT_SAMPLES: MacroInfo[] = [
  macro({
    title: "빗썸 리플 눌림목",
    exchange: "bithumb",
    symbol: "XRP_KRW",
    summary: "일봉 20일선 근처에서 나눠 사요.",
  }),
  macro({ title: "SOL 돌파", exchange: "binance", symbol: "SOLUSDT", market: "futures", leverage: 2 }),
];

export interface MockKey {
  access: string;
  secret: string;
  remembered: boolean;
  status: SlotCard["status"];
  problem: "" | "ip" | "auth";
}

export interface MockRun {
  id: string;
  macro: MacroInfo;
  fileName: string | null;
  mode: RunMode;
  phase: RunSlot["phase"];
  run: RunStatus | null;
  webLinked: boolean;
  /** 시작 전 연결 확인이 끝나면 이어서 시작함. */
  startAfterCheck: boolean;
  tick: number;
}

export interface MockFile {
  fileName: string;
  macro: MacroInfo | null;
  error: string;
  unsupported: string;
  modifiedAt: number;
}

export interface Model {
  runs: MockRun[];
  keys: Record<KeySlot, MockKey>;
  member: string;
  /** 웹 연결이 가져온 다른 계정의 회원 키(바꿀지 정하기 전). */
  memberPending: string;
  showDevLog: boolean;
  closeToTray: boolean;
  storage: SettingsState["storage"];
  wizard: WizardState | null;
  /** 정보 화면의 공인 IP(미리보기는 정해진 주소). */
  publicIp: SettingsState["publicIp"];
  update: UpdateState;
  files: MockFile[];
  nextRun: number;
  imported: number;
}

export const emptyKey = (): MockKey => ({ access: "", secret: "", remembered: false, status: "empty", problem: "" });

export function newRun(m: Model, macro: MacroInfo, fileName: string | null, webLinked = false): MockRun {
  const r: MockRun = {
    id: `run-${++m.nextRun}`,
    macro,
    fileName,
    mode: "mock",
    phase: "idle",
    run: null,
    webLinked,
    startAfterCheck: false,
    tick: 0,
  };
  m.runs.push(r);
  return r;
}

export function initialModel(params: URLSearchParams, now = Date.now()): Model {
  const storage = params.get("storage");
  const broken = (["read_error", "decrypt_error", "invalid_data", "unsupported"] as const).find((s) => s === storage);
  const m: Model = {
    runs: [],
    keys: {
      binance: emptyKey(),
      binance_testnet: emptyKey(),
      binance_demo: emptyKey(),
      upbit: emptyKey(),
      bithumb: emptyKey(),
    },
    member: "demo-member-key",
    memberPending: "",
    showDevLog: params.get("dev") === "1",
    closeToTray: true,
    storage: broken
      ? {
          status: broken,
          note:
            broken === "unsupported"
              ? "키는 이번 실행 동안만 기억해요."
              : broken === "read_error"
                ? "키체인 잠금을 풀거나 접근을 허용한 뒤 [다시 읽기]를 누르세요. 저장 항목은 지우지 않아요."
                : "다른 사용자 계정이나 기기에서 저장한 키일 수 있어요. 키를 다시 입력하거나 [지우고 새로 만들기]로 저장 항목을 새로 만드세요.",
        }
      : { status: "loaded", note: "" },
    wizard: null,
    publicIp: { status: "idle", address: "" },
    update: {
      status: "idle",
      current: MOCK_VERSION,
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
    files: LIBRARY_SAMPLES.map((s) => ({
      fileName: s.fileName,
      macro: s.macro,
      error: s.error ?? "",
      unsupported: s.unsupported ?? "",
      modifiedAt: now - s.ageMin * 60_000,
    })),
    nextRun: 0,
    imported: 0,
  };
  m.keys.binance = {
    access: "bnAccess00demo00000000",
    secret: "bnSecret00demo00000000",
    remembered: true,
    status: "connected",
    problem: "",
  };
  m.keys.bithumb = {
    access: "bad-bithumb-access",
    secret: "bithumbSecretDemo",
    remembered: false,
    status: "blocked",
    problem: "ip",
  };
  if (params.get("demo") !== "empty") {
    newRun(m, m.files[0].macro!, m.files[0].fileName);
    newRun(m, WEB_MACRO, WEB_FILE_NAME, true);
  }
  // ?demo=many — 목록에 스크롤이 생기게 매크로를 스무 개 넘게 채움(읽히는 파일·가져오기 견본을 돌려 씀).
  if (params.get("demo") === "many") {
    const pool = [
      ...m.files.filter((f) => f.macro && !f.unsupported).map((f) => [f.macro!, f.fileName] as const),
      ...IMPORT_SAMPLES.map((mc) => [mc, `${mc.title}.ggm.json`] as const),
    ];
    for (let i = 0; i < 22; i += 1) {
      const [mc, file] = pool[i % pool.length];
      newRun(m, mc, file);
    }
  }
  return m;
}

export function toLibraryFile(m: Model, f: MockFile): LibraryFile {
  const mc = f.macro;
  return {
    fileName: f.fileName,
    valid: !!mc,
    error: f.error,
    title: mc?.title ?? "",
    exchange: mc?.exchange ?? null,
    exchangeLabel: mc?.exchangeLabel ?? "",
    symbol: mc?.symbol ?? "",
    side: mc?.side ?? "long",
    summary: mc?.summary ?? "",
    strategy: mc?.strategy ?? "",
    unsupported: f.unsupported,
    signed: mc?.signature === "signed",
    modifiedAt: f.modifiedAt,
    inUse: m.runs.filter((r) => r.fileName === f.fileName).length,
  };
}

/** 같은 이름이 있으면 "이름 (2).ggm.json". */
export function uniqueName(m: Model, name: string): string {
  if (!m.files.some((f) => f.fileName === name)) return name;
  const base = name.replace(/(\.ggm)?\.json$/, "");
  const ext = name.slice(base.length);
  for (let i = 2; ; i += 1) {
    const next = `${base} (${i})${ext}`;
    if (!m.files.some((f) => f.fileName === next)) return next;
  }
}
