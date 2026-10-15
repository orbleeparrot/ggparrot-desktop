// Rust(src-tauri/src/state/render.rs의 DTO)와 1:1인 웹뷰 계약 v2 — 동시 실행·설정 분리·활동 기록.
// 상태는 Rust가 소유함. 웹뷰는 `runner:state` 스냅샷을 그리고 명령만 보냄.
// 거래소 키·시크릿·회원 키 원문은 이 타입 어디에도 없음(`reveal_*`의 1회 반환만 예외).

export type Exchange = "binance" | "upbit" | "bithumb";
/** 설정의 키 칸. 테스트넷으로 도는 바이낸스 매크로는 현물이면 binance_testnet, 선물이면 binance_demo 칸을 씀. */
export type KeySlot = "binance" | "binance_testnet" | "binance_demo" | "upbit" | "bithumb";
export type RunMode = "mock" | "testnet" | "live";
export type Market = "spot" | "futures";
export type Check = "ok" | "bad" | "wait" | "na";
export type KeyField = "access" | "secret";
export type StopMode = "stop_only" | "close_and_stop";
export type Tone = "muted" | "ok" | "warn" | "danger";

export interface MacroInfo {
  /** 웹 매크로 이름 또는 파일 이름(경로 제외). */
  title: string;
  source: "file" | "web";
  /** 매크로의 거래소 원문(소문자). 모르는 거래소면 exchange는 null. */
  exchangeName: string;
  exchange: Exchange | null;
  exchangeLabel: string;
  symbol: string;
  /** 기초 자산 이름(BTCUSDT → BTC, KRW-BTC → BTC). */
  baseAsset: string;
  quote: "USDT" | "KRW";
  market: Market;
  side: "long" | "short";
  leverage: number;
  summary: string;
  signature: "web" | "signed" | "unsigned" | "modified";
  /** 웹과 같은 전략 이름(예: "RSI 조건 매매"). 모르면 "". */
  strategy: string;
  /** 투자 금액(params.initial_capital, 견적 통화). 없으면 null. */
  capital: number | null;
  /** 사용자가 바꾸기 전 금액. 바꾸지 않았으면 null. */
  capitalOriginal: number | null;
  /** 투자 금액을 바꿀 수 없는 이유(웹에서 연결한 매크로). 바꿀 수 있으면 "". */
  capitalLocked: string;
  /** 1회 주문 상한(견적 통화) — 투자 금액이 커도 한 번에 이보다 많이 주문하지 않음. */
  orderCap: number;
  /** 이 앱이 로컬에서 지키는 안전 설정(예: "손절 평단 대비 가격 -3%"). 서버 연결이 끊겨도 지킴. */
  risk: string[];
  /** 청산 기준(익절·손절·트레일링, 평단 대비 가격 %) — 웹 positionExits.js `exitRules`와 같은 값·문구. */
  exits: ExitRules;
}

/** 청산 규칙 하나 — pct는 평단 대비 가격 %(양수), label은 "손절 -3%"·"익절 +5%". */
export interface ExitRule {
  pct: number;
  label: string;
}

export interface ExitRules {
  sl: ExitRule | null;
  tp: ExitRule | null;
  /** 트레일링(E) — 발동 수익 %(0이면 처음부터)·고점 대비 되돌림 %. label "발동 +5% · 고점 -2%". */
  trailing: { activation: number; trail: number; label: string } | null;
  kind: "two" | "sl-only" | "tp-only" | "none";
  /** "익절 +5% · 손절 -3%"·"손절 -4% · 익절은 신호"·"전략 신호". */
  summary: string;
}

export interface RunStatus {
  /** epoch ms */
  startedAt: number;
  sessionId: number | null;
  online: boolean;
  /** 서버가 회원 키를 받지 않음(401·403). 저절로 풀리지 않음. */
  authFailed: boolean;
  /** 서버가 요청을 거절한 HTTP 상태(일시적 끊김이 아님). 0이면 아님. */
  rejected: number;
  /** 서버와 끊긴 시각(epoch ms). 0이면 연결됨. */
  offlineSince: number;
  /** 현재가를 못 받기 시작한 시각(epoch ms) — 그동안 손절·안전 규칙을 확인하지 못함. 0이면 받고 있음. */
  priceStaleSince: number;
  /** 손절·강제 청산 주문이 연달아 실패한 횟수. */
  closeFailures: number;
  /** 진입 주문이 잔고(증거금) 부족으로 거절된 뒤 아직 진입 주문이 체결되지 않음. 다음 진입이 체결되면 풀림. */
  fundsShort: boolean;
  /** 마지막 상태를 받은 시각(epoch ms). 0이면 아직(시작 직후). */
  updatedAt: number;
  price: number;
  inPosition: boolean;
  uncertain: boolean;
  entryPrice: number;
  qty: number;
  /** 평가손익 %(지금 정리하면) — 목록 카드·상태 카드가 같은 값을 씀. 수수료를 알면 투입금 대비 수수료 반영 %,
   *  모르면 평단 대비 가격 %(수수료 전). */
  unrealizedPct: number;
  /** 평가손익 금액(견적 통화) — unrealizedPct와 같은 기준. */
  unrealized: number;
  /** 평가손익이 수수료 반영 값인지. 거짓이면 '수수료 제외 전'을 붙임(실전 선물·BNB 수수료). */
  unrealizedFeeAware: boolean;
  /** 투입금 — 지금 포지션에 들어간 돈(수량×평단 + 남은 진입 수수료). 보유가 없으면 0. */
  invested: number;
  /** 서버에 보고하는 실현손익 그대로(수수료를 빼지 않음) — 웹 기록과 같은 값. */
  realized: number;
  /** 이번 실행 실현손익(수수료 반영). 수수료를 모르면 null. */
  realizedNet: number | null;
  /** 이번 실행에서 낸 수수료(견적 통화). 모르면 null — 선물·BNB로 낸 수수료가 끼면 모름. */
  fees: number | null;
  /** 전일 대비 등락률(%) — 업비트·빗썸 시세에 함께 옴. 모르면 null. */
  dayChangePct: number | null;
  /** 서버가 알려 준 매크로 출처(예: "웹 저장본"). 없으면 "". */
  macroOrigin: string;
  note: string;
  /** 끝났으면 종료 결과. 실행 중이면 null. */
  finished: null | { status: "stopped" | "error"; note: string; at: number };
}

/** 실행 슬롯 하나 = 매크로 하나 + 모드 하나 + 서버 세션 하나. */
export interface RunSlot {
  id: string;
  macro: MacroInfo;
  mode: RunMode;
  modeChoices: RunMode[];
  phase: "idle" | "checking" | "starting" | "running" | "stopping" | "finished";
  /** 이 슬롯이 쓸 키 칸(모드에 따라 바이낸스는 실거래/테스트넷). 모의는 거래소 키 없이도 돎(회원 키는 필요). */
  keySlot: KeySlot | null;
  /** 시작할 수 있는가와 막힌 이유. needs가 있으면 [설정에서 입력]으로 그 칸에 데려감. */
  readiness: { canStart: boolean; blocker: string; needs: null | { kind: "key"; slot: KeySlot } | { kind: "member" } };
  run: RunStatus | null;
  /** 웹 [실행기 열기]로 들어온 슬롯. */
  webLinked: boolean;
  /** 거래소 종목 화면 단추 글자("업비트에서 보기" 등). 열 화면이 없으면 "". 주소는 `open_market_page`가 만듦. */
  marketPage: string;
  /** 껄무새 웹에서 이 실행 기록을 열 수 있는지 여부. 주소는 `open_run_web_page`가 만듦. */
  webRunPage: boolean;
  /** 카드 머리 한 줄 상태. */
  status: { text: string; tone: Tone };
}

export interface SlotCard {
  slot: KeySlot;
  exchange: Exchange;
  /** "바이낸스"·"바이낸스 테스트넷"·"바이낸스 데모"·"업비트"·"빗썸" */
  label: string;
  fieldLabels: [string, string];
  accessFilled: boolean;
  secretFilled: boolean;
  remembered: boolean;
  rememberSupported: boolean;
  status: "empty" | "unchecked" | "checking" | "connected" | "blocked";
  /** "✓ 연결됨"·"● 연결 안 됨"·"● 허용 IP 확인 필요"·"● 확인 중"·"● 확인 전" */
  headline: string;
  /** 칸 아래 한두 줄(막힌 이유·저장 문제). 없으면 "". */
  note: string;
  /** note의 색 — "warn"(잔고 부족·권한·허용 IP·키 저장소처럼 고쳐야 할 문제)·"muted"(참고). 없으면 muted. */
  noteTone?: "muted" | "warn";
  checks: { auth: Check; ip: Check; order: Check };
  /** 연결 도우미(업비트·빗썸·바이낸스). 바이낸스는 칸(실거래·테스트넷·데모)이 곧 사이트임. */
  wizardAvailable: boolean;
  /** 이 칸의 키로 실행 중인 매크로 수. 키는 언제든 바꿀 수 있고, 실행 중인 매크로는 시작할 때의 키를 계속 씀. */
  inUseBy: number;
}

export interface SettingsState {
  slots: SlotCard[];
  memberKeyFilled: boolean;
  /** 웹 연결이 가져온, 지금과 다른 계정의 회원 키 끝 네 글자("…abcd"). 없으면 "". 키 자체는 오지 않음. */
  memberKeyPending: string;
  /** 실행 중인 매크로 수 — 회원 키를 바꿔도 실행 중인 매크로는 시작할 때의 회원 키를 계속 씀. */
  memberInUseBy: number;
  storage: {
    status: "missing" | "loaded" | "read_error" | "decrypt_error" | "invalid_data" | "unsupported";
    note: string;
  };
  showDevLog: boolean;
  /** 창을 닫으면 트레이(메뉴 막대·알림 영역)로 숨기고 매크로는 계속 실행함. */
  closeToTray: boolean;
  serverBase: string;
  /** 기본 운영 서버가 아닌 곳에 연결됨 — 정보 화면에 서버 주소를 보여 줌. */
  serverCustom: boolean;
  /** "macOS 15.6.1 (arm64)" 등 — 정보 화면·문의용 정보. */
  os: string;
  /** 공인 IP — 연결 도우미와 같은 조회 결과(120초가 지나면 expired). */
  publicIp: { status: "idle" | "loading" | "ok" | "expired" | "error"; address: string };
  /** 서버 연동 레벨(정수 문자열, "19") — 서버가 기능별 최소 레벨과 비교하는 값. */
  serverLinkLevel: string;
}

/** 바이낸스 연결 도우미가 안내하는 사이트 — 현물 테스트넷·선물 데모·실거래. 국내는 "". */
export type BinanceEnv = "" | "testnet" | "demo" | "live";

export interface WizardState {
  exchange: Exchange;
  /** 키를 넣는 칸(바이낸스는 실거래·테스트넷·데모). */
  slot: KeySlot;
  env: BinanceEnv;
  market: Market;
  /** 사이트 이름 — "바이낸스 테스트넷"·"바이낸스 데모"·"바이낸스"·"업비트"·"빗썸". */
  label: string;
  /** 머리줄 '… 열기' 단추의 이름("테스트넷"·"데모"·거래소 이름). */
  homeLabel: string;
  stage: "ask" | "ip" | "perm" | "keys" | "existing" | "manual" | "checking" | "result_ok" | "result_fail";
  path: "" | "new" | "existing";
  ip: { status: "idle" | "loading" | "ok" | "error" | "expired"; address: string; note: string; changed: boolean };
  slots: { access: boolean; secret: boolean };
  fieldLabels: [string, string];
  checks: { auth: Check; ip: Check; order: Check };
  headline: string;
  /** 세 줄 오른쪽 글자(바이낸스, 예 "필요 없음"). 빈 값은 기본 글자. */
  notes: { auth: string; ip: string; order: string };
  /** 키 칸 두 개가 다 찼는지(복사로 받았든 직접 넣었든). */
  keysReady: boolean;
  reason: string;
  problem: "" | "ip" | "auth" | "order" | "order_size" | "network" | "other";
  watchingClipboard: boolean;
  remembered: boolean;
  rememberSupported: boolean;
}

/** 매크로 폴더의 파일 하나(.ggm.json·.json). */
export interface LibraryFile {
  fileName: string;
  /** 매크로가 읽히면 제목·거래소·종목. 읽히지 않으면 valid=false, error에 이유. */
  valid: boolean;
  error: string;
  title: string;
  exchange: Exchange | null;
  exchangeLabel: string;
  symbol: string;
  side: "long" | "short";
  summary: string;
  /** 웹과 같은 전략 이름. */
  strategy: string;
  /** 서버가 시작을 거절할 이유(적립식·SAR·여러 종목·모르는 거래소). 돌릴 수 있으면 "". */
  unsupported: string;
  signed: boolean;
  /** epoch ms */
  modifiedAt: number;
  /** 이 파일로 만든 실행 슬롯 수. */
  inUse: number;
}

export interface Library {
  /** 매크로 폴더 절대 경로(화면에 보여 줌). */
  dir: string;
  files: LibraryFile[];
  /** 매크로 형식 이름(.ggm.json)이 아니라 목록에서 뺀 .json 수. */
  skipped: number;
  /** 너무 많아 목록에 올리지 않은 수(최근 300개만). */
  truncated: number;
}

export interface UpdateState {
  status: "idle" | "checking" | "none" | "available" | "downloading" | "ready" | "installing" | "error" | "unsupported";
  current: string;
  /** available·downloading·ready일 때 새 버전. */
  version: string;
  notes: string;
  /** 0~1, downloading일 때만 의미가 있음. 크기를 모르면 -1. */
  progress: number;
  /** 내려받은 바이트·전체 바이트(모르면 0). */
  downloaded: number;
  total: number;
  error: string;
  /** epoch ms, 마지막 확인 시각. 0이면 확인 전. */
  checkedAt: number;
  /** 이 위치에서는 설치할 수 없는 이유(macOS 디스크 이미지에서 실행 등). 비면 설치 가능. */
  blocked: string;
  /** 이번 실행에서 막 업데이트됐다면 그 전 버전. */
  updatedFrom: string;
}

export interface AppState {
  /** 앱 버전(semver, 예 "0.18.3"). */
  version: string;
  runs: RunSlot[];
  settings: SettingsState;
  library: Library;
  wizard: WizardState | null;
  update: UpdateState;
  /** 머리줄 요약 — "실행 중 2" 등. */
  status: { text: string; tone: Tone };
  /** 웹 연결 중(티켓 교환 중)인지 여부. */
  claiming: boolean;
}

export type ActivityKind =
  "start" | "signal" | "buy" | "sell" | "stop_loss" | "blocked" | "connection" | "error" | "stop" | "info";

export interface Activity {
  id: number;
  runId: string;
  /** epoch ms */
  at: number;
  kind: ActivityKind;
  /** 사람이 읽는 한 줄. */
  title: string;
  /** 둘째 줄(이유·금액·손익). 없으면 "". */
  detail: string;
  /** 원문 로그 줄(자세히 보기). 없으면 "". */
  raw: string;
  /** 원문 로그 줄 자체(개발자 로그 보기를 켰을 때만 보임). */
  dev: boolean;
  tone: Tone;
}

export interface Notice {
  level: "info" | "warn" | "error";
  title: string;
  message: string;
  /** 이 알림을 남긴 앱 기록 번호(AppLogEntry.id) — [자세히]로 앱 기록을 열 때 씀. 없을 수 있음. */
  logId?: number;
}

export type AppLogLevel = "info" | "warn" | "error";
/** 앱 기록의 출처 — 앱 전체·웹 연결·업데이트·키(키체인·회원 키)·로컬 저장·매크로 폴더·서버·실행 시작·연결 도우미. */
export type AppLogSource = "app" | "web" | "update" | "keys" | "storage" | "library" | "server" | "run" | "wizard";

/** 앱 기록 한 줄 — 매크로 하나에 묶이지 않는 앱 전체의 일. 알림(토스트)도 모두 여기에 남음. 메모리에 최근 500줄. */
export interface AppLogEntry {
  /** 오르는 번호. get_app_log와 runner:app-log 이벤트가 같은 줄을 함께 줄 수 있으니 id로 합칠 것. */
  id: number;
  /** epoch ms */
  at: number;
  level: AppLogLevel;
  source: AppLogSource;
  /** 첫 줄(알림이면 알림 글의 첫 줄). */
  title: string;
  /** 나머지 줄. 없으면 "". 줄바꿈이 들어 있을 수 있음. */
  detail: string;
}

export type StartReply =
  | { kind: "started" }
  | { kind: "confirm_live"; title: string; message: string }
  | { kind: "checking" }
  | { kind: "blocked"; message: string }
  | { kind: "wizard" };

/** 이벤트 이름 */
export const EV_STATE = "runner:state";

/** runner:state 이벤트 — 매크로 폴더 목록은 바뀌었을 때만 실림(없으면 지난 목록을 씀). get_state는 늘 실음. */
export type StateEvent = Omit<AppState, "library"> & { library?: AppState["library"] };
/** payload: Activity */
export const EV_ACTIVITY = "runner:activity";
export const EV_NOTICE = "runner:notice";
/** payload: AppLogEntry — 앱 기록에 새 줄이 생길 때마다(알림이면 runner:notice보다 먼저). */
export const EV_APP_LOG = "runner:app-log";
/** payload: { runId, title, message } — 시작 전 검사가 끝난 뒤 실전 확인이 필요할 때. */
export const EV_CONFIRM = "runner:confirm";
/** payload: { to: "settings" | "settings:about" | "run" | "run:{id}" | "library" } — 앱 메뉴·트레이 등에서 화면을 바꿈. */
export const EV_NAVIGATE = "runner:navigate";
