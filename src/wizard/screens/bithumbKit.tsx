// 빗썸 화면 그림의 조각 — 로고 머리줄·주황 원형 체크.
import { CheckMark, T } from "./kit";
import type { SceneState } from "./types";

const BT_ORANGE = "#FE6420";
export const BT_DARK = "#1A1D24";
export const BT_LINE = "#DCDDE3";

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
