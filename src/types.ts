// Rust(src-tauri/src/state/render.rs의 DTO)와 1:1인 웹뷰 계약 v2 — 동시 실행·설정 분리·활동 기록.
// 상태는 Rust가 소유함. 웹뷰는 `runner:state` 스냅샷을 그리고 명령만 보냄.
// 거래소 키·시크릿·회원 키 원문은 이 타입 어디에도 없음(`reveal_*`의 1회 반환만 예외).

export type Exchange = "binance" | "upbit" | "bithumb";
/** 설정의 키 칸. 테스트넷으로 도는 바이낸스 매크로는 binance_testnet 칸을 씀. */
export type KeySlot = "binance" | "binance_testnet" | "upbit" | "bithumb";
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
  /** 이 앱이 로컬에서 지키는 안전 설정(예: "손절 3%"). 서버 연결이 끊겨도 지킴. */
  risk: string[];
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
  /** 마지막 상태를 받은 시각(epoch ms). 0이면 아직(시작 직후). */
  updatedAt: number;
  price: number;
  inPosition: boolean;
  uncertain: boolean;
  entryPrice: number;
  qty: number;
  unrealizedPct: number;
  unrealized: number;
  realized: number;
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
  /** 카드 머리 한 줄 상태. */
  status: { text: string; tone: Tone };
}

export interface SlotCard {
  slot: KeySlot;
  exchange: Exchange;
  /** "바이낸스"·"바이낸스 테스트넷"·"업비트"·"빗썸" */
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
  checks: { auth: Check; ip: Check; order: Check };
  /** 연결 도우미(업비트·빗썸). */
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
  /** 서버 계약 버전(정수 문자열, "18"). */
  protocolVersion: string;
}

export interface WizardState {
  exchange: "upbit" | "bithumb";
  label: string;
  stage: "ask" | "ip" | "perm" | "keys" | "existing" | "manual" | "checking" | "result_ok" | "result_fail";
  path: "" | "new" | "existing";
  ip: { status: "idle" | "loading" | "ok" | "error" | "expired"; address: string; note: string; changed: boolean };
  slots: { access: boolean; secret: boolean };
  fieldLabels: [string, string];
  checks: { auth: Check; ip: Check; order: Check };
  headline: string;
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
/** payload: { runId, title, message } — 시작 전 검사가 끝난 뒤 실전 확인이 필요할 때. */
export const EV_CONFIRM = "runner:confirm";
/** payload: { to: "settings" | "settings:about" | "run" | "run:{id}" | "library" } — 앱 메뉴·트레이 등에서 화면을 바꿈. */
export const EV_NAVIGATE = "runner:navigate";

/**
 * 명령(invoke 이름·인자). 실패하면 한국어 오류 문자열로 reject.
 * 실행
 *  get_state() -> AppState
 *  get_activity({ runId }) -> Activity[]
 *  add_macro_file() -> string | null          (파일을 골라 매크로 폴더로 가져온 뒤 슬롯 추가, 추가한 runId·취소면 null)
 *  add_run_from_library({ fileName }) -> string (추가한 runId)
 *  remove_run({ runId }) -> void              (실행 중이면 거절)
 *  set_run_mode({ runId, mode }) -> void
 *  start_run({ runId }) -> StartReply
 *  start_run_confirmed({ runId }) -> StartReply
 *  stop_run({ runId, mode: StopMode }) -> void
 *  copy_activity({ runId }) -> void
 * 설정
 *  set_slot_key({ slot, field, value }) -> void
 *  reveal_slot_key({ slot, field }) -> string
 *  set_slot_remember({ slot, remember }) -> void
 *  check_slot({ slot }) -> void
 *  set_member_key({ value }) -> void
 *  reveal_member_key() -> string
 *  set_show_dev_log({ show }) -> void
 *  set_close_to_tray({ value }) -> void
 *  recover_storage() -> void
 *  update_check() -> void
 *  update_install() -> void                   (내려받아 서명 확인, 끝나면 status=ready. 설치는 아직)
 *  update_restart() -> void                   (설치하고 다시 시작. 실행 중인 매크로가 있으면 거절)
 * 매크로 폴더
 *  library_refresh() -> void
 *  library_open_folder() -> void              (Finder·탐색기로 엶)
 *  library_import() -> number                 (파일을 골라 폴더로 복사, 가져온 수. 같은 이름은 "이름 (2).ggm.json")
 *  창에 파일을 끌어다 놓으면 Rust가 폴더로 가져오고 runner:notice(info)로 알림.
 * 연결 도우미(업비트·빗썸)
 *  wizard_open({ exchange, stage? }) -> void
 *  wizard_choose({ path: "new" | "existing" }) -> void
 *  wizard_go({ stage }) -> void
 *  wizard_back() -> void
 *  wizard_close() -> void
 *  wizard_refresh_ip() -> void
 *  wizard_copy_ip() -> void
 *  wizard_copy_and_open() -> void
 *  wizard_open_api_page() -> void
 *  wizard_open_home() -> void
 *  wizard_recheck() -> void
 *  wizard_recopy() -> void
 *  wizard_set_key({ field: "access" | "secret", value }) -> void
 *  wizard_set_remember({ remember }) -> void
 */
