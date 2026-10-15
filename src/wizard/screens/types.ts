import type { ReactElement } from "react";

/** 장면의 움직임 한 칸 — 커서 이동·클릭 등 예시 화면 재생의 한 동작. */
export type Step =
  | { t: "move"; to: string }
  | { t: "click" }
  | { t: "on"; name: string }
  | { t: "show"; layer: string }
  | { t: "hide"; layer: string }
  | { t: "focus"; input: string }
  | { t: "type"; input: string }
  | { t: "wait"; ms: number };

export interface SceneState {
  on: ReadonlySet<string>;
  layers: ReadonlySet<string>;
  focus: ReadonlySet<string>;
  typed: Readonly<Record<string, string>>;
}

export interface Scene {
  /** 브라우저 주소창에 보일 주소(그림일 뿐). */
  url: string;
  width: number;
  height: number;
  targets: Record<string, readonly [number, number]>;
  steps: Step[];
  draw(s: SceneState): ReactElement;
}

export const EMPTY_STATE: SceneState = { on: new Set(), layers: new Set(), focus: new Set(), typed: {} };

/** 입력칸의 커서 목표 — 칸의 오른쪽을 누름(왼쪽부터 찍히는 주소 글자를 가리지 않게). */
export function inputTarget(x0: number, y0: number, x1: number, y1: number): readonly [number, number] {
  return [Math.min(x1 - 24, x0 + Math.max(150, (x1 - x0) * 0.6)), (y0 + y1) / 2];
}

export const W = 520;
