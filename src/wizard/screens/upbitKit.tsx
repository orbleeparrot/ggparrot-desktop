// 업비트 화면 그림의 조각 — 남색 머리줄·파란 네모 체크.
import { CheckMark, T } from "./kit";
import type { SceneState } from "./types";

export const UP_NAVY = "#093687";
export const UP_BLUE = "#0062DF";
const UP_CHECK = "#1261C4";
export const UP_LINE = "#D5D6DC";

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
