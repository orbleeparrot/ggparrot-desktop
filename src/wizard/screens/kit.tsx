// 거래소 화면 그림의 공통 조각 — 글자·체크 표시·버튼·입력칸·복사 단추·모달. 거래소별 조각은 upbitKit·bithumbKit·binanceKit.
import type { ReactNode } from "react";
import type { SceneState } from "./types";

export const INK = "#111827";
export const MUTED = "#6B7280";
const PLACEHOLDER = "#A6ABB5";
export const DIM = "rgba(17, 24, 39, 0.42)";

interface TextProps {
  x: number;
  y: number;
  children: ReactNode;
  bold?: boolean;
  italic?: boolean;
  fill?: string;
  anchor?: "start" | "middle" | "end";
  size?: number;
}

export function T({ x, y, children, bold, italic, fill = INK, anchor = "start", size = 12 }: TextProps) {
  return (
    <text
      x={x}
      y={y}
      fill={fill}
      textAnchor={anchor}
      dominantBaseline="central"
      fontSize={size}
      fontWeight={bold ? 700 : 500}
      fontStyle={italic ? "italic" : undefined}
    >
      {children}
    </text>
  );
}

/** 24칸 체크 표시(M5 12.5 l4.5 4.5 L19 7.5)를 size 칸으로. */
export function CheckMark({
  x,
  y,
  size,
  stroke,
  width = 2.2,
}: {
  x: number;
  y: number;
  size: number;
  stroke: string;
  width?: number;
}) {
  const k = size / 24;
  const d = `M${x + 5 * k} ${y + 12.5 * k} L${x + 9.5 * k} ${y + 17 * k} L${x + 19 * k} ${y + 7.5 * k}`;
  return <path d={d} fill="none" stroke={stroke} strokeWidth={width} strokeLinecap="round" strokeLinejoin="round" />;
}

export function FlatButton(p: {
  s: SceneState;
  name?: string;
  x0: number;
  y0: number;
  x1: number;
  y1: number;
  label: string;
  off: string;
  on: string;
  radius?: number;
}) {
  const lit = !!p.name && p.s.on.has(p.name);
  return (
    <g>
      <rect
        x={p.x0}
        y={p.y0}
        width={p.x1 - p.x0}
        height={p.y1 - p.y0}
        rx={p.radius ?? 0}
        fill={lit ? p.on : p.off}
        className="ex-fade"
      />
      <T x={(p.x0 + p.x1) / 2} y={(p.y0 + p.y1) / 2} bold fill="#FFFFFF" anchor="middle">
        {p.label}
      </T>
    </g>
  );
}

export function InputBox(p: {
  s: SceneState;
  name: string;
  x0: number;
  y0: number;
  x1: number;
  y1: number;
  placeholder: string;
  line: string;
  focus: string;
  radius?: number;
  /** 어두운 화면(바이낸스)용 — 칸 바탕·입력 글자·안내 글 색, 안내 글 크기. */
  fill?: string;
  ink?: string;
  hint?: string;
  hintSize?: number;
}) {
  const typed = p.s.typed[p.name] ?? "";
  const focused = p.s.focus.has(p.name);
  const cy = (p.y0 + p.y1) / 2;
  return (
    <g>
      <rect
        x={p.x0}
        y={p.y0}
        width={p.x1 - p.x0}
        height={p.y1 - p.y0}
        rx={p.radius ?? 0}
        fill={p.fill ?? "#FFFFFF"}
        stroke={focused ? p.focus : p.line}
        strokeWidth={focused ? 1.5 : 1}
      />
      {typed ? (
        <T x={p.x0 + 10} y={cy} bold fill={p.ink}>
          {typed}
          {focused && <tspan className="ex-caret">|</tspan>}
        </T>
      ) : (
        <T x={p.x0 + 10} y={cy} fill={p.hint ?? PLACEHOLDER} size={p.hintSize}>
          {p.placeholder}
        </T>
      )}
    </g>
  );
}

export function CopyButton(p: {
  s: SceneState;
  name: string;
  x1: number;
  y: number;
  line: string;
  ink: string;
  onFill: string;
  radius?: number;
  height?: number;
}) {
  const h = p.height ?? 26;
  const w = 52;
  const lit = p.s.on.has(p.name);
  return (
    <g>
      <rect
        x={p.x1 - w}
        y={p.y}
        width={w}
        height={h}
        rx={p.radius ?? 0}
        fill={lit ? p.onFill : "#FFFFFF"}
        stroke={lit ? p.onFill : p.line}
        className="ex-fade"
      />
      <T x={p.x1 - w / 2} y={p.y + h / 2} fill={lit ? "#FFFFFF" : p.ink} anchor="middle">
        {lit ? "복사됨" : "복사"}
      </T>
    </g>
  );
}

export function copyTarget(x1: number, y: number, h = 26): readonly [number, number] {
  return [x1 - 26, y + h / 2];
}

export function ModalBox({
  x0,
  y0,
  x1,
  y1,
  radius = 0,
}: {
  x0: number;
  y0: number;
  x1: number;
  y1: number;
  radius?: number;
}) {
  return (
    <g>
      <rect x={x0 + 2} y={y0 + 3} width={x1 - x0} height={y1 - y0} rx={radius} fill="#E9EBEF" />
      <rect x={x0} y={y0} width={x1 - x0} height={y1 - y0} rx={radius} fill="#FFFFFF" stroke="#E5E7EB" />
    </g>
  );
}
