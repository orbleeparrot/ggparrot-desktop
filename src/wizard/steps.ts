// 단계 점과 바닥 버튼 — 도우미 단계별 점 목록과 바닥 버튼의 표.
// '처음이에요'의 단계 순서는 Rust(`wizard::flow`)·웹 가이드와 같음.
import type { BinanceEnv, WizardState } from "../types";

export type StepState = "done" | "on" | "todo";
type Stage = WizardState["stage"];

interface Flow {
  stages: readonly Stage[];
  labels: readonly string[];
}

// 국내는 허용 IP를 먼저 등록하고 발급함. 바이낸스는 키를 만든 뒤 그 키에 IP·권한을 걺
// (테스트넷·데모에는 IP 제한이 없어 두 단계).
const DOMESTIC: Flow = { stages: ["ip", "perm", "keys"], labels: ["허용 IP", "권한·발급", "키 복사", "연결 확인"] };
const PRACTICE: Flow = { stages: ["perm", "keys"], labels: ["키 만들기", "키 복사", "연결 확인"] };
const LIVE: Flow = { stages: ["perm", "keys", "ip"], labels: ["키 만들기", "키 복사", "허용 IP·권한", "연결 확인"] };

export function flowOf(env: BinanceEnv): Flow {
  if (env === "live") return LIVE;
  if (env === "testnet" || env === "demo") return PRACTICE;
  return DOMESTIC;
}

const EXISTING_STEPS = ["기존 키 넣기", "연결 확인"] as const;

const EXISTING_INDEX: Partial<Record<Stage, number>> = {
  existing: 0,
  manual: 0,
  checking: 1,
  result_fail: 1,
  result_ok: 2,
};

function newIndex(flow: Flow, stage: Stage): number | undefined {
  const i = flow.stages.indexOf(stage);
  if (i >= 0) return i;
  // 직접 입력은 키 복사 칸, 확인 중·확인 필요는 '연결 확인' 칸, 연결 완료는 모두 끝남.
  if (stage === "manual") return flow.stages.indexOf("keys");
  if (stage === "checking" || stage === "result_fail") return flow.stages.length;
  if (stage === "result_ok") return flow.stages.length + 1;
  return undefined;
}

/** 지금 단계의 점들. 질문 화면(ask)이나 길이 정해지기 전에는 점이 없음. */
export function stepStates(w: Pick<WizardState, "stage" | "path" | "env">): { label: string; state: StepState }[] {
  const existing = w.path === "existing";
  const flow = flowOf(w.env);
  const labels: readonly string[] = existing ? EXISTING_STEPS : flow.labels;
  const index = existing ? EXISTING_INDEX[w.stage] : newIndex(flow, w.stage);
  if (index === undefined || w.stage === "ask") return [];
  return labels.map((label, i) => ({ label, state: i < index ? "done" : i === index ? "on" : "todo" }));
}

export interface FootButton {
  label: string;
  action: "back" | "close" | "next" | "manual" | "check" | "done";
  primary?: boolean;
  enabled: boolean;
}

/** '처음이에요' 길에서 다음 단계. 마지막 단계면 없음. */
export function nextStage(w: Pick<WizardState, "stage" | "env">): Stage | undefined {
  const stages = flowOf(w.env).stages;
  const i = stages.indexOf(w.stage);
  return i >= 0 && i + 1 < stages.length ? stages[i + 1] : undefined;
}

export function footFor(w: WizardState): { left?: FootButton; right?: FootButton } {
  const back: FootButton = { label: "이전", action: "back", enabled: true };
  switch (w.stage) {
    case "ask":
      return { left: { label: "닫기", action: "close", enabled: true } };
    case "ip":
      // 바이낸스 실거래는 허용 IP·주문 권한이 마지막 단계 — 걸고 나면 바로 연결을 확인함.
      if (!nextStage(w))
        return { left: back, right: { label: "연결 확인", action: "check", primary: true, enabled: w.keysReady } };
      return { left: back, right: { label: "다음", action: "next", primary: true, enabled: true } };
    case "perm":
      // [다음]은 오른쪽 주 단추 — [이전](왼쪽 보조)과 무게가 같으면 어느 쪽이 앞으로인지 헷갈림.
      return { left: back, right: { label: "다음", action: "next", primary: true, enabled: true } };
    case "keys":
    case "existing":
      return { left: back, right: { label: "직접 입력", action: "manual", enabled: true } };
    case "manual":
      return {
        left: back,
        // 직접 넣은 키는 '복사로 받음' 표시(slots)가 아니라 칸이 찼는지(keysReady)로 봄.
        right: { label: "연결 확인", action: "check", primary: true, enabled: w.keysReady },
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

const TITLES: Record<Stage, string> = {
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
const BINANCE_TITLES: Partial<Record<Stage, string>> = { perm: "키 만들기", ip: "허용 IP · 주문 권한" };

/** 단계 제목 — 바이낸스는 '키 만들기'·'허용 IP · 주문 권한'(웹 가이드와 같은 말). */
export function stageTitle(w: Pick<WizardState, "stage" | "env">): string {
  return (w.env && BINANCE_TITLES[w.stage]) || TITLES[w.stage];
}
