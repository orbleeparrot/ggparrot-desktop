// 상태 카드의 현재가 그래프 — 선(Sparkline) 위에 숫자 꼬리표를 HTML로 얹음.
// - 오른쪽 칸: 지금 가격(선 끝 높이)·평단(점선 높이). 너무 가까우면 하나를 비켜 놓음(spreadLabels).
// - 그래프 안 왼쪽 위·아래: 이 구간의 최고·최저. 아래 줄: 그래프가 덮는 시간("최근 12분").
// - SVG는 preserveAspectRatio="none"이라 글자를 SVG 안에 두면 찌그러짐 — 그래서 HTML로 둠.
// - 같은 숫자가 아래 표(현재가·평단)에 있으므로 화면 읽기 프로그램에는 숨김.
import { chartDirection, chartRange, chartY, spreadLabels } from "../../state/chart";
import { formatPrice, formatSpan } from "../../state/format";
import type { MacroInfo } from "../../types";
import { Sparkline } from "../Sparkline";

/** 그래프 높이(px) — SVG 눈금(viewBox 높이)과 꼬리표 위치를 같은 단위로 맞추려고 한 값을 씀. */
const PLOT_PX = 64;
/** 꼬리표 두 개 사이 최소 간격·꼬리표 높이의 반(px). run.css .chart-tag 높이(18px)와 맞춤. */
const TAG_GAP = 20;
const TAG_HALF = 9;

interface Props {
  points: number[];
  entry?: number;
  quote: MacroInfo["quote"];
  /** 그래프가 덮는 시간(ms). 0이면 표시하지 않음. */
  spanMs: number;
}

export function PriceChart({ points, entry, quote, spanMs }: Props) {
  if (points.length < 2) {
    return (
      <div className="price-chart is-empty" aria-hidden="true">
        <div className="chart-plot" style={{ height: PLOT_PX }}>
          <Sparkline points={points} height={PLOT_PX} />
        </div>
      </div>
    );
  }
  const entryPrice = entry && entry > 0 ? entry : 0;
  const hasEntry = entryPrice > 0;
  const range = chartRange(points, entryPrice);
  const last = points[points.length - 1];
  const high = Math.max(...points);
  const low = Math.min(...points);
  const lastY = chartY(last, range, PLOT_PX);
  const [priceTop, entryTop] = spreadLabels(
    hasEntry ? [lastY, chartY(entryPrice, range, PLOT_PX)] : [lastY],
    PLOT_PX,
    TAG_GAP,
    TAG_HALF,
  );
  const span = formatSpan(spanMs);

  return (
    <div className={`price-chart dir-${chartDirection(points)}`} aria-hidden="true">
      <div className="chart-plot" style={{ height: PLOT_PX }}>
        <Sparkline points={points} entry={entryPrice} height={PLOT_PX} head={false} />
        <span className="chart-dot" style={{ top: lastY }} />
        {high !== low && (
          <>
            <span className="chart-extreme is-high num">최고 {formatPrice(high, quote)}</span>
            <span className="chart-extreme is-low num">최저 {formatPrice(low, quote)}</span>
          </>
        )}
      </div>
      <div className="chart-gutter" style={{ height: PLOT_PX }}>
        {hasEntry && (
          <span className="chart-tag is-entry num" style={{ top: entryTop }}>
            평단 {formatPrice(entryPrice, quote)}
          </span>
        )}
        <span className="chart-tag is-price num" style={{ top: priceTop }}>
          {formatPrice(last, quote)}
        </span>
      </div>
      {span && <span className="chart-span">{span}</span>}
    </div>
  );
}
