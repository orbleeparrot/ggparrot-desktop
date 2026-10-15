// 상태 카드의 청산 기준 막대 — 손절가·평단·현재가·익절가(트레일링 발동가)를 한 축에 놓는 순수 계산.
// - 축은 '평단 대비 가격 %'(롱은 가격 그대로, 숏은 뒤집음): 왼쪽 끝 = 손절, 오른쪽 끝 = 익절·발동.
//   그래서 숏은 왼쪽이 높은 가격임 — 어느 방향이든 왼쪽은 손실, 오른쪽은 이익 쪽.
// - 한쪽 규칙만 있으면 그쪽 폭을 반대편에도 써서 점이 움직일 자리를 둠(웹 positionExits.js gaugeModel과 같음).
// - 위치는 실제 가격에 비례함(평단 대비 % 축은 가격과 1차 관계).
// - 막대 아래 한 줄은 현재가에서 각 기준까지 거리 — 같은 축이라 숏도 왼쪽(손절)은 -, 오른쪽(익절)은 +.
import type { ExitRules } from "../types";
import { formatPercent } from "./format";

export interface ExitBarEnd {
  /** "손절 -3%"·"익절 +5%"·"익절은 신호"·"손절 규칙 없음"·"고점 -2% 추적 중" */
  label: string;
  /** 그 기준의 가격. 규칙이 없으면 null. */
  price: number | null;
  tone: "up" | "down" | "muted";
}

export interface ExitBar {
  show: boolean;
  /** 축 위 위치(0~100, %). */
  entryX: number;
  priceX: number;
  /** 현재가가 축 밖이면 그쪽 끝에 붙이고 표시함. */
  beyond: "left" | "right" | null;
  /** 평단 대비 현재가 %(이익 쪽 +). */
  move: number;
  left: ExitBarEnd;
  right: ExitBarEnd;
  /** 현재가에서 손절·익절(발동)까지 — "손절까지 -3.96%"·"익절까지 +2.97%"·"발동까지 +1.20%"·"익절은 신호". */
  toLeft: string;
  toRight: string;
}

const HIDDEN: ExitBar = {
  show: false,
  entryX: 50,
  priceX: 50,
  beyond: null,
  move: 0,
  left: { label: "", price: null, tone: "muted" },
  right: { label: "", price: null, tone: "muted" },
  toLeft: "",
  toRight: "",
};

/** 평단 대비 가격 % — 롱은 오르면 +, 숏은 내리면 +. */
export function moveFromEntry(price: number, entry: number, side: "long" | "short"): number {
  if (!(entry > 0) || !(price > 0)) return 0;
  const move = (price / entry - 1) * 100;
  return side === "short" ? -move : move;
}

/** 평단에서 이익 쪽으로 pct%(음수면 손실 쪽) 움직인 가격. */
export function priceAt(entry: number, pct: number, side: "long" | "short"): number {
  return entry * (1 + ((side === "short" ? -1 : 1) * pct) / 100);
}

export function exitBar(rules: ExitRules, side: "long" | "short", entry: number, price: number): ExitBar {
  if (!(entry > 0)) return HIDDEN;
  const sl = rules.sl?.pct ?? 0;
  const tp = rules.tp?.pct ?? 0;
  const trailing = rules.trailing;
  const up = tp || trailing?.activation || trailing?.trail || 0;
  if (!sl && !up) return HIDDEN;
  const lo = -(sl || up);
  const hi = up || sl;
  const at = (p: number) => Math.min(100, Math.max(0, ((p - lo) / (hi - lo)) * 100));
  const move = moveFromEntry(price, entry, side);
  const armed = !!trailing && trailing.activation > 0 && move >= trailing.activation;
  const left: ExitBarEnd = rules.sl
    ? { label: rules.sl.label, price: priceAt(entry, -sl, side), tone: "down" }
    : { label: "손절 규칙 없음", price: null, tone: "muted" };
  let right: ExitBarEnd;
  if (rules.tp) right = { label: rules.tp.label, price: priceAt(entry, tp, side), tone: "up" };
  else if (trailing)
    right = {
      label: armed ? `고점 -${trimPct(trailing.trail)}% 추적 중` : trailing.label,
      price: trailing.activation > 0 ? priceAt(entry, trailing.activation, side) : null,
      tone: "up",
    };
  else right = { label: "익절은 신호", price: null, tone: "muted" };
  const beyond = price > 0 && move < lo ? "left" : price > 0 && move > hi ? "right" : null;
  // 현재가에서 그 가격까지 이익 쪽 % — 손절은 늘 -, 익절·발동은 늘 +(지나쳤으면 "…지남").
  const dist = (target: number) => formatPercent(moveFromEntry(target, price, side));
  let toLeft = "손절 규칙 없음";
  if (left.price !== null)
    toLeft = !(price > 0) ? "" : beyond === "left" ? "손절선 지남" : `손절까지 ${dist(left.price)}`;
  let toRight = "익절은 신호";
  // 트레일링 — 발동 전엔 발동가까지 거리, 추적 중이면 오른쪽 끝 이름("고점 -2% 추적 중")이 이미 말하므로 비움.
  if (trailing && !rules.tp)
    toRight =
      armed || !(price > 0) || right.price === null
        ? trailing.activation > 0
          ? ""
          : `고점 -${trimPct(trailing.trail)}% 추적 중`
        : `발동까지 ${dist(right.price)}`;
  else if (rules.tp && right.price !== null)
    toRight = !(price > 0) ? "" : beyond === "right" ? "익절선 지남" : `익절까지 ${dist(right.price)}`;
  return {
    show: true,
    entryX: at(0),
    priceX: price > 0 ? at(move) : at(0),
    beyond,
    move,
    left,
    right,
    toLeft,
    toRight,
  };
}

/** 2.5·5 — 끝의 0 없이 소수 둘째 자리까지(웹 fmtPct). */
function trimPct(v: number): string {
  return String(Math.round(v * 100) / 100);
}

/** 보유가 없을 때의 한 줄 — "익절 +5% · 손절 -3% (평단 대비 가격)". 규칙이 없으면 "전략 신호". */
export function exitSummary(rules: ExitRules): string {
  return rules.kind === "none" ? rules.summary : `${rules.summary} (평단 대비 가격)`;
}
