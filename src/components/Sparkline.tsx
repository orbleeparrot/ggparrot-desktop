// 이번 실행에서 받은 현재가 흐름. 보유 중이면 평단을 점선으로 겹침. 숫자 꼬리표는 PriceChart가 HTML로 얹음.
import { useId } from "react";
import { chartDirection, chartRange, chartY } from "../state/chart";

interface Props {
  points: number[];
  /** 점마다의 시각 — 있으면 가로 위치를 시간에 비례하게 둠(길이가 points와 같을 때만). */
  times?: number[];
  entry?: number;
  width?: number;
  height?: number;
  /** 끝점 동그라미 — 늘어나는 SVG에서는 찌그러지므로, 겉에서 HTML로 그릴 때는 끔. */
  head?: boolean;
}

export function Sparkline({ points, times, entry, width = 240, height = 64, head = true }: Props) {
  const gradientId = useId();
  if (points.length < 2) {
    return (
      <svg
        className="sparkline is-empty"
        viewBox={`0 0 ${width} ${height}`}
        preserveAspectRatio="none"
        aria-hidden="true"
      >
        <line x1="0" x2={width} y1={height / 2} y2={height / 2} />
      </svg>
    );
  }
  const range = chartRange(points, entry);
  const ts = times && times.length === points.length && times[times.length - 1] > times[0] ? times : null;
  const x = (i: number) =>
    ts ? ((ts[i] - ts[0]) / (ts[ts.length - 1] - ts[0])) * width : (i / (points.length - 1)) * width;
  const y = (v: number) => chartY(v, range, height);
  const line = points.map((v, i) => `${x(i).toFixed(1)},${y(v).toFixed(1)}`).join(" ");
  const area = `0,${height} ${line} ${width},${height}`;
  const dir = chartDirection(points);
  const last = points[points.length - 1];
  return (
    <svg
      className={`sparkline dir-${dir}`}
      viewBox={`0 0 ${width} ${height}`}
      preserveAspectRatio="none"
      aria-hidden="true"
    >
      <defs>
        <linearGradient id={gradientId} x1="0" x2="0" y1="0" y2="1">
          <stop offset="0" stopColor="currentColor" stopOpacity="0.22" />
          <stop offset="1" stopColor="currentColor" stopOpacity="0" />
        </linearGradient>
      </defs>
      <polygon points={area} fill={`url(#${gradientId})`} />
      {entry && entry > 0 ? <line className="sparkline-entry" x1="0" x2={width} y1={y(entry)} y2={y(entry)} /> : null}
      <polyline points={line} fill="none" stroke="currentColor" strokeWidth="1.75" vectorEffect="non-scaling-stroke" />
      {head && <circle className="sparkline-head" cx={width} cy={y(last)} r="3" />}
    </svg>
  );
}
