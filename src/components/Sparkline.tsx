// 이번 실행에서 받은 현재가 흐름. 보유 중이면 평단을 점선으로 겹침.
import { useId } from "react";

interface Props {
  points: number[];
  entry?: number;
  width?: number;
  height?: number;
}

export function Sparkline({ points, entry, width = 240, height = 64 }: Props) {
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
  const values = entry && entry > 0 ? [...points, entry] : points;
  let min = Math.min(...values);
  let max = Math.max(...values);
  if (max - min < 1e-9) {
    min -= 1;
    max += 1;
  }
  const pad = 4;
  const x = (i: number) => (i / (points.length - 1)) * width;
  const y = (v: number) => pad + (1 - (v - min) / (max - min)) * (height - pad * 2);
  const line = points.map((v, i) => `${x(i).toFixed(1)},${y(v).toFixed(1)}`).join(" ");
  const area = `0,${height} ${line} ${width},${height}`;
  const dir = points[points.length - 1] >= points[0] ? "up" : "down";
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
      <circle className="sparkline-head" cx={width} cy={y(last)} r="3" />
    </svg>
  );
}
