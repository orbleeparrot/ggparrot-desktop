// invoke/listen 얇은 래퍼. Tauri 안이면 진짜 명령을, 브라우저(`npm run dev`)면 mock 백엔드를 씀.
import type {
  Activity,
  AppLogEntry,
  AppState,
  EntryPreview,
  Exchange,
  KeyField,
  KeySlot,
  Notice,
  RunMode,
  StartReply,
  StateEvent,
  StopMode,
  WizardState,
} from "./types";
import { EV_ACTIVITY, EV_APP_LOG, EV_CONFIRM, EV_NAVIGATE, EV_NOTICE, EV_STATE } from "./types";

type NoArgs = undefined;
type RunArg = { runId: string };

/**
 * Rust 명령(`src-tauri/src/lib.rs`의 `generate_handler!`) 표. 이름·인자·반환 타입을 컴파일러가 확인함.
 * - 실패하면 한국어 오류 문자열로 reject함.
 * - 이름 목록은 `scripts/commands.test.mjs`가 Rust 쪽과 같은지 확인함.
 */
/** 구동 중 설정 변경 요청 — 바꾼 칸만 담는다(부분 수정). 값이 null 이면 그 규칙을 끈다. */
export interface ChangeSettingsArgs extends RunArg {
  changes: Record<string, Record<string, number | null>>;
  applyMode: "immediate" | "next_entry";
}

/** 서버 응답. `applied`가 false면 "이 포지션 정리된 뒤부터"로 예약된 것이다. */
export interface ChangeSettingsReply {
  ok: boolean;
  applied: boolean;
  apply_mode: string;
  summary: string;
  note: string;
}

export interface CommandMap {
  // 실행
  get_state: { args: NoArgs; ret: AppState };
  get_activity: { args: RunArg; ret: Activity[] };
  /** 파일을 골라 매크로 폴더로 가져온 뒤 슬롯 추가. 추가한 runId, 취소면 null. */
  add_macro_file: { args: NoArgs; ret: string | null };
  /** 추가한 runId를 돌려줌. */
  add_run_from_library: { args: { fileName: string }; ret: string };
  set_run_capital: { args: { runId: string; amount: number | null }; ret: void };
  move_run: { args: RunArg & { index: number }; ret: void };
  /** 실행 중이면 거절. */
  remove_run: { args: RunArg; ret: void };
  set_run_mode: { args: RunArg & { mode: RunMode }; ret: void };
  start_run: { args: RunArg; ret: StartReply };
  start_run_confirmed: { args: RunArg; ret: StartReply };
  entry_preview: { args: RunArg; ret: EntryPreview };
  /** 구동 중 설정 변경 — 서버가 받아 전략을 다시 세우고 기록을 남긴다. 실패하면 이유가 담긴 Error.
   *  한 줄로 적는다 — scripts/commands.test.mjs 가 줄머리의 `이름:` 으로 명령을 세어, 여러 줄로
   *  쓰면 args·ret 까지 명령으로 읽힌다. */
  change_run_settings: { args: ChangeSettingsArgs; ret: ChangeSettingsReply };
  stop_run: { args: RunArg & { mode: StopMode }; ret: void };
  copy_activity: { args: RunArg; ret: void };
  open_market_page: { args: RunArg; ret: void };
  /** 껄무새 웹의 이 실행 기록 화면을 기본 브라우저로 엶. */
  open_run_web_page: { args: RunArg; ret: void };
  refresh_public_ip: { args: { force: boolean }; ret: void };
  /** 문의용 정보 — 끝에 최근 앱 기록 20줄. */
  copy_support_info: { args: NoArgs; ret: void };
  // 앱 기록
  /** 앱 기록 전체(오래된 것 → 최근). 새 줄은 runner:app-log로 옴. */
  get_app_log: { args: NoArgs; ret: AppLogEntry[] };
  /** 머리말 + 줄마다 날짜·시각. 길면 최근 줄만. */
  copy_app_log: { args: NoArgs; ret: void };
  // 설정
  set_slot_key: { args: { slot: KeySlot; field: KeyField; value: string }; ret: void };
  reveal_slot_key: { args: { slot: KeySlot; field: KeyField }; ret: string };
  set_slot_remember: { args: { slot: KeySlot; remember: boolean }; ret: void };
  check_slot: { args: { slot: KeySlot }; ret: void };
  /** 설정 › 거래소 연결 칸의 거래소 사이트(주소는 Rust). */
  open_slot_site: { args: { slot: KeySlot }; ret: void };
  set_member_key: { args: { value: string }; ret: void };
  reveal_member_key: { args: NoArgs; ret: string };
  set_show_dev_log: { args: { show: boolean }; ret: void };
  set_close_to_tray: { args: { value: boolean }; ret: void };
  set_notify: { args: { value: boolean }; ret: void };
  recover_storage: { args: NoArgs; ret: void };
  reload_storage: { args: NoArgs; ret: void };
  resolve_member_key: { args: { accept: boolean }; ret: void };
  update_check: { args: NoArgs; ret: void };
  /** 내려받아 서명 확인. 끝나면 status=ready(설치는 아직). */
  update_install: { args: NoArgs; ret: void };
  /** 설치하고 다시 시작. 실행 중인 매크로가 있으면 거절. */
  update_restart: { args: NoArgs; ret: void };
  // 매크로 폴더 — 창에 파일을 끌어다 놓으면 Rust가 폴더로 가져오고 runner:notice(info)로 알림.
  library_refresh: { args: NoArgs; ret: void };
  /** Finder·탐색기로 엶. */
  library_open_folder: { args: NoArgs; ret: void };
  /** 파일을 골라 폴더로 복사. 가져온 수를 돌려줌. 같은 이름은 "이름 (2).ggm.json". */
  library_import: { args: NoArgs; ret: number };
  // 껄무새 웹(기본 브라우저)
  open_web: { args: NoArgs; ret: void };
  /** 껄무새 웹의 내 에이전트(실행 기록 목록). */
  open_web_agents: { args: NoArgs; ret: void };
  // 연결 도우미
  /** slot: 바이낸스 실거래·테스트넷·데모 칸. */
  wizard_open: { args: { exchange: Exchange; slot?: KeySlot; stage?: WizardState["stage"] }; ret: void };
  wizard_choose: { args: { path: "new" | "existing" }; ret: void };
  wizard_go: { args: { stage: WizardState["stage"] }; ret: void };
  wizard_back: { args: NoArgs; ret: void };
  wizard_close: { args: NoArgs; ret: void };
  wizard_refresh_ip: { args: NoArgs; ret: void };
  wizard_copy_ip: { args: NoArgs; ret: void };
  wizard_copy_and_open: { args: NoArgs; ret: void };
  wizard_open_api_page: { args: NoArgs; ret: void };
  wizard_open_home: { args: NoArgs; ret: void };
  wizard_recheck: { args: NoArgs; ret: void };
  wizard_recopy: { args: NoArgs; ret: void };
  wizard_set_key: { args: { field: KeyField; value: string }; ret: void };
  wizard_set_remember: { args: { remember: boolean }; ret: void };
}

export type Command = keyof CommandMap;
export type ArgsOf<K extends Command> = CommandMap[K]["args"] extends NoArgs ? [] : [CommandMap[K]["args"]];

/** runner:confirm — 시작 전 검사가 끝난 뒤 실전 확인이 필요할 때. */
export interface ConfirmPayload {
  runId: string;
  title: string;
  message: string;
}
/** runner:navigate — 앱 메뉴(설정… ⌘,)·웹 [실행기 열기] 등에서 화면을 바꿈.
 * "settings"·"settings:about"(정보/업데이트로)·"library"(매크로 추가 시트)·"run"·"run:{runId}"(그 슬롯을 고름). */
export interface NavigatePayload {
  to: "settings" | "settings:about" | "library" | "run" | `run:${string}`;
}

export interface EventMap {
  [EV_STATE]: StateEvent;
  [EV_ACTIVITY]: Activity;
  [EV_NOTICE]: Notice;
  [EV_APP_LOG]: AppLogEntry;
  [EV_CONFIRM]: ConfirmPayload;
  [EV_NAVIGATE]: NavigatePayload;
}
export type EventName = keyof EventMap;
export type Unlisten = () => void;

export interface Backend {
  invoke(cmd: string, args?: Record<string, unknown>): Promise<unknown>;
  listen(event: string, handler: (payload: unknown) => void): Promise<Unlisten>;
}

export function isTauri(): boolean {
  return typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;
}

/** 브라우저 미리보기(`?demo=…&open=1`)에서만 쓰는 디자인 검토용 바로가기. Tauri 안에서는 늘 "". */
export function demoParam(name = "demo"): string {
  if (isTauri() || typeof location === "undefined") return "";
  return new URLSearchParams(location.search).get(name) ?? "";
}

let backendPromise: Promise<Backend> | null = null;

async function tauriBackend(): Promise<Backend> {
  const [core, event] = await Promise.all([import("@tauri-apps/api/core"), import("@tauri-apps/api/event")]);
  return {
    invoke: (cmd, args) => core.invoke(cmd, args),
    listen: async (name, handler) => event.listen(name, (e) => handler(e.payload)),
  };
}

async function mockBackend(): Promise<Backend> {
  const mock = await import("./mock");
  return mock.createMockBackend(undefined, { dom: true });
}

function backend(): Promise<Backend> {
  backendPromise ??= isTauri() ? tauriBackend() : mockBackend();
  return backendPromise;
}

/** 명령 하나. 실패하면 Rust가 준 한국어 오류 문자열(Error로 감싼)로 reject함. */
export async function call<K extends Command>(cmd: K, ...args: ArgsOf<K>): Promise<CommandMap[K]["ret"]> {
  const b = await backend();
  try {
    return (await b.invoke(cmd, args[0] as Record<string, unknown> | undefined)) as CommandMap[K]["ret"];
  } catch (err) {
    throw new Error(errorText(err), { cause: err });
  }
}

export async function on<E extends EventName>(name: E, handler: (payload: EventMap[E]) => void): Promise<Unlisten> {
  const b = await backend();
  return b.listen(name, (payload) => handler(payload as EventMap[E]));
}

export function errorText(err: unknown): string {
  if (typeof err === "string") return err;
  if (err instanceof Error) return err.message;
  return "알 수 없는 오류가 났어요.";
}
