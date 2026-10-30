// 상태 카드의 청산 기준 막대 — 손절가·평단·현재가(점)·익절가를 실제 가격 위치로 한 줄에 놓음.
// - 계산은 state/exitBar.ts(순수 함수). 왼쪽 끝은 손절, 오른쪽 끝은 익절·트레일링 발동(숏도 같음).
// - 한 줄 배치: [손절 끝] [막대] [익절 끝]. 막대 위는 '지금' 꼬리표, 아래는 평단 눈금 이름.
// - '지금' 꼬리표에는 가격·%를 다시 적지 않음 — 현재가는 그래프에, 평가손익은 큰 숫자에 이미 있음.
// - 막대는 평단을 경계로 왼쪽(손실 쪽) 옅은 빨강·오른쪽(이익 쪽) 옅은 초록.
// - 꼬리표 겹침 방지: 위·아래 꼬리표는 다른 줄이고, 꼬리표마다 left X%·translateX(-X%)로 놓아
//   끝에 가까울수록 꼬리표가 안쪽으로 붙음 — 막대 밖으로 나가지 않고 늘 제 점 위에 걸침.
// - 머리줄 오른쪽에 현재가에서 각 기준까지 거리("손절까지 -3.96% · 익절까지 +2.97%").
import type { CSSProperties } from "react";
import { exitBar, exitSummary } from "../../state/exitBar";
import { formatPercent, formatPrice } from "../../state/format";
import type { ExitRules, MacroInfo } from "../../types";

interface Props {
  rules: ExitRules;
  side: MacroInfo["side"];
  quote: MacroInfo["quote"];
  /** 보유가 없으면 0 — 막대 대신 요약 한 줄. */
  entry: number;
  price: number;
}

/** X%(0~100)에 놓되 꼬리표 자체를 X%만큼 왼쪽으로 당김 — 양 끝에서도 막대 안에 머묾. */
function anchor(x: number) {
  return { left: `${x}%`, transform: `translateX(-${x}%)` };
}

export function ExitBar({ rules, side, quote, entry, price }: Props) {
  const bar = exitBar(rules, side, entry, price);
  if (!bar.show) {
    // 보유 전(진입 대기 등)에는 막대 대신 기준을 칩으로 — 줄글 한 줄로 늘어놓지 않음.
    const parts = rules.summary.split(" · ").filter(Boolean);
    return (
      <div className="exit-chips" role="group" aria-label={`청산 기준 · ${exitSummary(rules)}`}>
        <span className="exit-line-head">청산 기준</span>
        {parts.map((part) => (
          <span
            key={part}
            className={`exit-chip ${/손절/.test(part) ? "tone-down" : /익절|발동/.test(part) ? "tone-up" : ""}`}
          >
            {part}
          </span>
        ))}
        {rules.kind !== "none" && <span className="exit-chip-note">평단 대비</span>}
      </div>
    );
  }
  const gain = bar.priceX >= bar.entryX;
  const entryText = `평단 ${formatPrice(entry, quote)} ${quote}`;
  const distance = [bar.toLeft, bar.toRight].filter(Boolean).join(" · ");
  const end = (e: typeof bar.left, at: "left" | "right") => (
    <span className={`exit-end is-${at} tone-${e.tone}`}>
      <span className="exit-end-label">{e.label}</span>
      {e.price !== null && <span className="exit-end-price num">{formatPrice(e.price, quote)}</span>}
    </span>
  );
  return (
    <div className="exit-bar" role="group" aria-label={`청산 기준 · ${exitSummary(rules)}`}>
      <div className="exit-top">
        <span className="exit-head">청산 기준 · 평단 대비 가격</span>
        {distance && (
          <span className="exit-dist num" title="현재가에서 각 기준 가격까지(손실 쪽 -, 이익 쪽 +)">
            {distance}
          </span>
        )}
      </div>
      <div className="exit-row">
        {end(bar.left, "left")}
        <div className="exit-scale" aria-hidden="true">
          <span
            className={`exit-tag is-price tone-${gain ? "up" : "down"}`}
            style={anchor(bar.priceX)}
            title={`현재 ${formatPrice(price, quote)} · 평단 대비 ${formatPercent(bar.move)}`}
          >
            {bar.beyond === "left" && "◀ "}지금
            {bar.beyond === "right" && " ▶"}
          </span>
          <div className="exit-track" style={{ "--entry": `${bar.entryX}%` } as CSSProperties}>
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
          <span className="exit-entry" style={anchor(bar.entryX)} title={entryText}>
            평단
          </span>
        </div>
        {end(bar.right, "right")}
      </div>
    </div>
  );
}
