// 거래소 화면 그림의 조각 — 업비트(남색·파란 네모 체크)·빗썸(주황 원형 체크·검정 버튼).
import type { ReactNode } from "react";
import type { SceneState } from "./types";

export const INK = "#111827";
export const MUTED = "#6B7280";
export const PLACEHOLDER = "#A6ABB5";
export const UP_NAVY = "#093687";
export const UP_BLUE = "#0062DF";
export const UP_CHECK = "#1261C4";
export const UP_LINE = "#D5D6DC";
export const BT_ORANGE = "#FE6420";
export const BT_DARK = "#1A1D24";
export const BT_LINE = "#DCDDE3";
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

export function UpBox(p: {
  s: SceneState;
  name?: string;
  x: number;
  y: number;
  label?: string;
  size?: number;
  line?: string;
  width?: number;
}) {
  const { s, name, x, y, label, size = 16, line = "#C4C8CF", width = 1 } = p;
  const on = !!name && s.on.has(name);
  return (
    <g>
      <rect x={x} y={y} width={size} height={size} fill="#FFFFFF" stroke={on ? UP_CHECK : line} strokeWidth={width} />
      {on && <CheckMark x={x + size * 0.08} y={y + size * 0.08} size={size * 0.84} stroke={UP_CHECK} />}
      {label && (
        <T x={x + size + 6} y={y + size / 2} fill="#333333">
          {label}
        </T>
      )}
    </g>
  );
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
        fill="#FFFFFF"
        stroke={focused ? p.focus : p.line}
        strokeWidth={focused ? 1.5 : 1}
      />
      {typed ? (
        <T x={p.x0 + 10} y={cy} bold>
          {typed}
          {focused && <tspan className="ex-caret">|</tspan>}
        </T>
      ) : (
        <T x={p.x0 + 10} y={cy} fill={PLACEHOLDER}>
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

export function BtCircle(p: {
  s: SceneState;
  name?: string;
  x: number;
  y: number;
  label: string;
  bold?: boolean;
  ink?: string;
}) {
  const on = !!p.name && p.s.on.has(p.name);
  const ink = p.ink ?? "#444444";
  return (
    <g>
      <circle
        cx={p.x + 10}
        cy={p.y + 10}
        r={10}
        fill={on ? BT_ORANGE : "#FFFFFF"}
        stroke={on ? BT_ORANGE : "#DCDCE0"}
        strokeWidth={1.5}
        className="ex-fade"
      />
      <CheckMark x={p.x + 4} y={p.y + 4} size={12} stroke={on ? "#FFFFFF" : "#CFD0D5"} />
      <T x={p.x + 26} y={p.y + 10} bold={p.bold} fill={on ? "#111111" : ink}>
        {p.label}
      </T>
    </g>
  );
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

/** 업비트 머리줄 — 남색 띠·'UPbit'·제목. */
export function UpChrome({ title = "Open API 관리" }: { title?: string }) {
  return (
    <g>
      <rect x={0} y={0} width={520} height={32} fill={UP_NAVY} />
      <T x={12} y={16} bold italic fill="#FFFFFF" size={13}>
        UPbit
      </T>
      <T x={66} y={16} bold fill="#FFFFFF">
        {title}
      </T>
    </g>
  );
}

/** 빗썸 머리줄 — 로고·'API 관리'·(탭 API 2.0 / 1.0). 다음 내용이 시작할 y를 돌려줌. */
export function BtChrome({ tabs = true }: { tabs?: boolean }) {
  return (
    <g>
      <line x1={0} y1={32} x2={520} y2={32} stroke="#EEEEEE" />
      <image href="/exchanges/bithumb.png" x={12} y={7} width={18} height={18} />
      <T x={38} y={16} bold>
        API 관리
      </T>
      {tabs && (
        <g>
          <T x={14} y={46} bold fill="#111111">
            API 2.0
          </T>
          <line x1={14} y1={59} x2={62} y2={59} stroke="#111111" strokeWidth={2} />
          <T x={80} y={46} fill="#B5B8BF">
            API 1.0
          </T>
          <line x1={0} y1={60} x2={520} y2={60} stroke="#EEEEEE" />
        </g>
      )}
    </g>
  );
}
