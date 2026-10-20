// 매크로 조건 차트 — 받은 마감봉 위에 **엔진이 낸** 가격선을 겹쳐 그림.
//
// 봉도 선도 다시 계산하지 않는다. 봉은 거래소에서 받은 그대로, 선은 엔진이 낸 그대로다 —
// 화면이 다시 계산하면 규칙 산수가 두 벌이 되고, 실제로 그 일이 있었다(E 눌림목 선이
// 현재가를 따라 영원히 내려왔다).
//
// SVG 는 preserveAspectRatio="none" 이라 글자를 안에 두면 찌그러짐 — 꼬리표는 HTML 로 얹음.
import { useId } from "react";
import {
  candleRange,
  candleWidth,
  candleX,
  candleY,
  chartSpanMs,
  priceLines,
  type ChartData,
} from "../../state/candles";
import { formatPrice } from "../../state/format";
import type { MacroInfo } from "../../types";

/** 그래프 높이(px) — SVG 눈금(viewBox 높이)과 꼬리표 위치를 같은 단위로 맞춤. */
const PLOT_PX = 120;
const VIEW_W = 320;
/** 꼬리표 두 개 사이 최소 간격 · 꼬리표 높이의 반(px). */
const TAG_GAP = 18;
const TAG_HALF = 8;

interface Props {
  chart: ChartData | null;
  price: number;
  /** 보유 중이면 평단 — 가로 점선으로 겹침. */
  entry?: number;
  quote: MacroInfo["quote"];
  intervalLabel: string;
}

/** 꼬리표가 겹치지 않게 아래로 밀어 놓음(위에서부터 차례로). */
function spread(tags: { y: number }[], height: number): number[] {
  const sorted = tags.map((t, i) => ({ i, y: t.y })).sort((a, b) => a.y - b.y);
  let last = -Infinity;
  const out = new Array<number>(tags.length);
  for (const { i, y } of sorted) {
    const placed = Math.max(y, last + TAG_GAP);
    out[i] = Math.min(placed, height - TAG_HALF);
    last = out[i];
  }
  return out;
}

function spanLabel(ms: number): string {
  if (ms <= 0) return "";
  const hours = ms / 3_600_000;
  if (hours < 48) return `${Math.round(hours)}시간`;
  return `${Math.round(hours / 24)}일`;
}

export function CandleChart({ chart, price, entry = 0, quote, intervalLabel }: Props) {
  const clipId = useId();
  const bars = chart?.bars ?? [];
  if (bars.length < 2) {
    return (
      <div className="candle-chart is-empty">
        <div className="candle-plot" style={{ height: PLOT_PX }} />
        <p className="candle-note">
          {chart?.levels?.pending ? "첫 봉이 마감되면 조건선이 나와요." : "봉을 받는 중이에요."}
        </p>
      </div>
    );
  }

  const lines = priceLines(chart?.levels, price);
  const range = candleRange(bars, lines, entry);
  const width = candleWidth(bars.length, VIEW_W);
  const tags = [
    { key: "now", label: "지금", value: price, tone: "now" as const },
    ...(entry > 0 ? [{ key: "entry", label: "평단", value: entry, tone: "info" as const }] : []),
    ...lines.map((l) => ({ key: l.key, label: l.label, value: l.value, tone: l.tone })),
  ].filter((t) => Number.isFinite(t.value) && t.value > 0);
  const ys = spread(
    tags.map((t) => ({ y: candleY(t.value, range, PLOT_PX) })),
    PLOT_PX,
  );

  return (
    <div className="candle-chart">
      <div className="candle-plot" style={{ height: PLOT_PX }}>
        <svg viewBox={`0 0 ${VIEW_W} ${PLOT_PX}`} preserveAspectRatio="none" aria-hidden="true" className="candle-svg">
          <defs>
            <clipPath id={clipId}>
              <rect x="0" y="0" width={VIEW_W} height={PLOT_PX} />
            </clipPath>
          </defs>
          <g clipPath={`url(#${clipId})`}>
            {bars.map(([t, open, high, low, close], i) => {
              const x = candleX(i, bars.length, VIEW_W);
              const up = close >= open;
              const top = candleY(Math.max(open, close), range, PLOT_PX);
              const bottom = candleY(Math.min(open, close), range, PLOT_PX);
              return (
                <g key={t} className={up ? "candle is-up" : "candle is-down"}>
                  <line x1={x} x2={x} y1={candleY(high, range, PLOT_PX)} y2={candleY(low, range, PLOT_PX)} />
                  <rect
                    x={x - width / 2}
                    y={top}
                    width={width}
                    // 시가 == 종가면 높이가 0 이라 안 보임 — 최소 1px.
                    height={Math.max(1, bottom - top)}
                  />
                </g>
              );
            })}
            {entry > 0 && (
              <line
                className="candle-line is-entry"
                x1="0"
                x2={VIEW_W}
                y1={candleY(entry, range, PLOT_PX)}
                y2={candleY(entry, range, PLOT_PX)}
              />
            )}
            {lines.map((line) => (
              <line
                key={line.key}
                className={`candle-line is-${line.tone}`}
                x1="0"
                x2={VIEW_W}
                y1={candleY(line.value, range, PLOT_PX)}
                y2={candleY(line.value, range, PLOT_PX)}
              />
            ))}
          </g>
        </svg>
        <div className="candle-tags">
          {tags.map((tag, i) => (
            <span key={tag.key} className={`candle-tag is-${tag.tone}`} style={{ top: ys[i] - TAG_HALF }}>
              {tag.label} {formatPrice(tag.value, quote)}
            </span>
          ))}
        </div>
      </div>
      <p className="candle-note">
        {intervalLabel} 봉 {bars.length}개 · {spanLabel(chartSpanMs(bars))}
        {chart?.levels?.indicator ? (
          <span className="candle-indicator">
            {" · "}
            {chart.levels.indicator.name} {chart.levels.indicator.value.toFixed(1)}
            {chart.levels.indicator.entry_threshold !== null
              ? ` (사는 선 ${chart.levels.indicator.entry_threshold})`
              : ""}
          </span>
        ) : null}
      </p>
    </div>
  );
}
