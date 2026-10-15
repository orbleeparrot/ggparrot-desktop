// 단계 점과 바닥 버튼 — 도우미 단계별 점 목록과 바닥 버튼의 표.
import type { WizardState } from "../types";

export type StepState = "done" | "on" | "todo";

export const NEW_STEPS = ["허용 IP", "권한·발급", "키 복사", "연결 확인"] as const;
export const EXISTING_STEPS = ["기존 키 넣기", "연결 확인"] as const;

const NEW_INDEX: Partial<Record<WizardState["stage"], number>> = {
  ip: 0,
  perm: 1,
  keys: 2,
  manual: 2,
  checking: 3,
  result_fail: 3,
  result_ok: 4,
};
const EXISTING_INDEX: Partial<Record<WizardState["stage"], number>> = {
  existing: 0,
  manual: 0,
  checking: 1,
  result_fail: 1,
  result_ok: 2,
};

/** 지금 단계의 점들. 질문 화면(ask)이나 길이 정해지기 전에는 점이 없음. */
export function stepStates(w: Pick<WizardState, "stage" | "path">): { label: string; state: StepState }[] {
  const existing = w.path === "existing";
  const labels: readonly string[] = existing ? EXISTING_STEPS : NEW_STEPS;
  const index = (existing ? EXISTING_INDEX : NEW_INDEX)[w.stage];
  if (index === undefined || w.stage === "ask") return [];
  return labels.map((label, i) => ({ label, state: i < index ? "done" : i === index ? "on" : "todo" }));
}

export interface FootButton {
  label: string;
  action: "back" | "close" | "next" | "manual" | "check" | "done";
  primary?: boolean;
  enabled: boolean;
}

export function footFor(w: WizardState): { left?: FootButton; right?: FootButton } {
  const back: FootButton = { label: "이전", action: "back", enabled: true };
  switch (w.stage) {
    case "ask":
      return { left: { label: "닫기", action: "close", enabled: true } };
    case "ip":
    case "perm":
      return { left: back, right: { label: "다음", action: "next", enabled: true } };
    case "keys":
    case "existing":
      return { left: back, right: { label: "직접 입력", action: "manual", enabled: true } };
    case "manual":
      return {
        left: back,
        right: { label: "연결 확인", action: "check", primary: true, enabled: w.slots.access && w.slots.secret },
      };
    case "checking":
      return { left: back, right: { label: "완료", action: "done", primary: true, enabled: false } };
    case "result_fail":
      // 실패 화면의 다음 할 일은 본문에 있음(다시 확인·키 다시 입력). 아래 단추는 닫기만.
      return { left: back, right: { label: "닫기", action: "close", enabled: true } };
    case "result_ok":
      return { left: back, right: { label: "완료", action: "done", primary: true, enabled: true } };
  }
}

export const NEXT_STAGE: Partial<Record<WizardState["stage"], WizardState["stage"]>> = { ip: "perm", perm: "keys" };

export const STAGE_TITLES: Record<WizardState["stage"], string> = {
  ask: "",
  ip: "허용 IP 등록",
  perm: "권한 선택 · 키 발급",
  keys: "키 복사",
  existing: "기존 키 넣기",
  manual: "직접 입력",
  checking: "연결 확인 중",
  result_ok: "연결 완료",
  result_fail: "연결 확인 필요",
};
