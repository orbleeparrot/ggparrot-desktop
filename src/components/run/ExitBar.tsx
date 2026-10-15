// 상태 카드의 청산 기준 막대 — 손절가·평단·현재가(점)·익절가를 실제 가격 위치로 한 줄에 놓음.
// - 계산은 state/exitBar.ts(순수 함수). 왼쪽 끝은 손절, 오른쪽 끝은 익절·트레일링 발동(숏도 같음).
// - 위 줄 꼬리표(평단·현재가)는 겹치면 비켜 놓음(차트 꼬리표와 같은 spreadLabels).
// - 손절·익절 %는 평단 대비 가격 기준임을 머리말에 밝힘(수수료 반영 손익과 다른 기준).
import { spreadLabels } from "../../state/chart";
import { exitBar, exitSummary } from "../../state/exitBar";
import { formatPrice } from "../../state/format";
import type { ExitRules, MacroInfo } from "../../types";

/** 위 줄 꼬리표 사이 최소 간격·꼬리표 폭의 반(막대 너비 대비 %). 꼬리표 하나가 대개 막대의 1/4 안쪽임. */
const TAG_GAP = 26;
const TAG_HALF = 11;

interface Props {
  rules: ExitRules;
  side: MacroInfo["side"];
  quote: MacroInfo["quote"];
  /** 보유가 없으면 0 — 막대 대신 요약 한 줄. */
  entry: number;
  price: number;
}

export function ExitBar({ rules, side, quote, entry, price }: Props) {
  const bar = exitBar(rules, side, entry, price);
  if (!bar.show) {
    return (
      <p className="exit-line">
        <span className="exit-line-head">청산 기준</span> {exitSummary(rules)}
      </p>
    );
  }
  const [entryLeft, priceLeft] = spreadLabels([bar.entryX, bar.priceX], 100, TAG_GAP, TAG_HALF);
  const gain = bar.priceX >= bar.entryX;
  const end = (e: typeof bar.left, at: "left" | "right") => (
    <span className={`exit-end is-${at} tone-${e.tone}`}>
      <span className="exit-end-label">{e.label}</span>
      {e.price !== null && <span className="exit-end-price num">{formatPrice(e.price, quote)}</span>}
    </span>
  );
  return (
    <div className="exit-bar" role="group" aria-label={`청산 기준 · ${exitSummary(rules)}`}>
      <span className="exit-head">청산 기준 · 평단 대비 가격</span>
      <div className="exit-tags" aria-hidden="true">
        <span className="exit-tag is-entry num" style={{ left: `${entryLeft}%` }}>
          평단 {formatPrice(entry, quote)}
        </span>
        <span className={`exit-tag is-price num tone-${gain ? "up" : "down"}`} style={{ left: `${priceLeft}%` }}>
          {bar.beyond === "left" && "◀ "}
          {formatPrice(price, quote)}
          {bar.beyond === "right" && " ▶"}
        </span>
      </div>
      <div className="exit-track" aria-hidden="true">
        <span
          className={`exit-fill is-${gain ? "up" : "down"}`}
          style={{ left: `${Math.min(bar.entryX, bar.priceX)}%`, width: `${Math.abs(bar.priceX - bar.entryX)}%` }}
        />
        <span className="exit-tick" style={{ left: `${bar.entryX}%` }} />
        <span
          className={`exit-dot is-${gain ? "up" : "down"} ${bar.beyond ? "is-beyond" : ""}`}
          style={{ left: `${bar.priceX}%` }}
        />
      </div>
      <div className="exit-ends">
        {end(bar.left, "left")}
        {end(bar.right, "right")}
      </div>
    </div>
  );
}
