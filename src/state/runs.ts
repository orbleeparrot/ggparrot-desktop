// 실행 슬롯 표시 규칙 — 모드·방향 이름, 버튼 상태, 막힌 이유에서 설정의 어느 칸으로 갈지.
import type { KeySlot, RunMode, RunSlot, StopMode } from "../types";

export const MODE_LABELS: Record<RunMode, string> = { mock: "모의", testnet: "테스트넷", live: "실전" };

/** 모드 세그먼트 단추의 설명(풍선 도움말·접근성 설명). */
export const MODE_NOTES: Record<RunMode, string> = {
  mock: "주문을 보내지 않고 현재가로 연습해요.",
  testnet: "바이낸스 테스트넷 키로 연습 주문을 보내요.",
  live: "실제 자금으로 주문해요. 시작할 때 한 번 더 확인해요.",
};

/** "모의로"·"테스트넷으로"·"실전으로" — 받침에 맞춘 조사. */
const MODE_WITH: Record<RunMode, string> = { mock: "모의로", testnet: "테스트넷으로", live: "실전으로" };

/** 상세 조작 줄의 모드 배지 — 모드를 바꿀 수 없는 단계(확인·시작·실행·종료 중)에 세그먼트 대신 보임. */
export function runningModeText(mode: RunMode, phase: Phase): string {
  if (phase === "checking") return `${MODE_WITH[mode]} 시작 전 확인 중`;
  if (phase === "starting") return `${MODE_WITH[mode]} 시작하는 중`;
  return `${MODE_WITH[mode]} 실행 중`;
}

export const SIDE_LABELS = { long: "롱", short: "숏" } as const;
export const MARKET_LABELS = { spot: "현물", futures: "선물" } as const;

export type Phase = RunSlot["phase"];

/** 상태 문구 → [앞부분, 뒷부분]. "실행 중 · 서버 재시도 중" → ["실행 중", "서버 재시도 중"]. 카드가 좁으면 뒷부분을 숨김. */
export function statusParts(text: string): [string, string] {
  const i = text.indexOf(" · ");
  return i < 0 ? [text, ""] : [text.slice(0, i), text.slice(i + 3)];
}

/** 대기 중이거나 끝난 슬롯만 모드를 바꾸고, 시작하고, 목록에서 뺄 수 있음. */
export function isIdle(phase: Phase): boolean {
  return phase === "idle" || phase === "finished";
}

/** 종료할 수 있는 단계(시작하는 중·실행 중). */
export function canStop(phase: Phase): boolean {
  return phase === "starting" || phase === "running";
}

/** 실행 중으로 셀 단계 — 카드의 큰 손익·경과 시간을 보임. */
export function isLive(slot: RunSlot): boolean {
  return (
    !!slot.run &&
    !slot.run.finished &&
    (slot.phase === "running" || slot.phase === "starting" || slot.phase === "stopping")
  );
}

export type PrimaryAction = { kind: "start" } | { kind: "stop" } | { kind: "busy"; label: string };

export function primaryAction(phase: Phase): PrimaryAction {
  if (isIdle(phase)) return { kind: "start" };
  if (canStop(phase)) return { kind: "stop" };
  return { kind: "busy", label: phase === "checking" ? "연결 확인 중…" : "종료 처리 중…" };
}

const STOP_LABELS = { stop_only: "매크로만 종료", close_and_stop: "청산 후 종료" } as const;

/** 고를 수 있는 종료 방법 하나. */
export interface StopChoice {
  mode: StopMode;
  label: string;
  hint: string;
  /** 실제(테스트넷 포함) 주문을 내는 선택인지 여부 — 빨간 단추·확인. */
  danger: boolean;
}

/**
 * 지금 고를 수 있는 종료 방법. 두 방법의 차이가 없으면 [종료] 하나만 보임.
 * - 모의: 실제 포지션이 없음 — 모의 보유분은 지금 가격으로 청산한 것으로 계산하고 끝냄(주문 없음).
 *   보유 중이면 이름이 '청산', 보유가 없으면 팔 게 없으니 '종료'(10/18 — '종료'보다 '청산'이 낫다는 요청).
 * - 보유 없음: '포지션 유지'와 '청산'이 같음.
 * - 보유 중(실전·테스트넷): 두 가지. 테스트넷의 청산은 테스트넷 주문이라고 밝힘.
 */
export function stopChoices(slot: Pick<RunSlot, "mode" | "run">): StopChoice[] {
  const holding = !!slot.run?.inPosition;
  if (slot.mode === "mock")
    return [
      {
        mode: "close_and_stop",
        label: holding ? "청산" : "종료",
        hint: holding
          ? "모의 보유분을 지금 가격으로 청산한 것으로 계산하고 끝내요. 실제 주문은 보내지 않아요."
          : "모의 실행을 끝내요. 실제 주문은 보내지 않아요.",
        danger: false,
      },
    ];
  if (!holding)
    return [{ mode: "stop_only", label: "종료", hint: "보유 포지션이 없어요. 매크로를 끝내요.", danger: false }];
  const where = slot.mode === "testnet" ? "테스트넷에서 " : "";
  return [
    {
      mode: "stop_only",
      label: STOP_LABELS.stop_only,
      hint: "매크로만 멈추고 열린 포지션은 그대로 둬요. 포지션은 거래소에서 직접 관리하세요.",
      danger: false,
    },
    {
      mode: "close_and_stop",
      label: STOP_LABELS.close_and_stop,
      hint: `보유 포지션을 ${where}시장가로 정리한 뒤 종료해요.`,
      danger: true,
    },
  ];
}

/** 설정 화면에서 데려갈 곳. log: 앱 기록의 그 줄(알림 토스트의 [자세히]). */
export type SettingsTarget =
  | { kind: "slot"; slot: KeySlot }
  | { kind: "member" }
  | { kind: "section"; section: SettingsSection }
  | { kind: "log"; id: number };

export type SettingsSection = "exchanges" | "account" | "library" | "display" | "about" | "log";

export function settingsTargetOf(needs: RunSlot["readiness"]["needs"]): SettingsTarget | null {
  if (!needs) return null;
  return needs.kind === "member" ? { kind: "member" } : { kind: "slot", slot: needs.slot };
}

/** 데려간 곳의 DOM id — 굴려서 보이고 그 안의 첫 입력칸에 포커스를 둠. */
export function settingsElementId(target: SettingsTarget): string {
  if (target.kind === "slot") return `settings-slot-${target.slot}`;
  if (target.kind === "member") return "settings-member";
  if (target.kind === "log") return `app-log-${target.id}`;
  return `settings-${target.section}`;
}
