// 실행 상태 카드 — 맨 윗줄(상태·경과 시간·서버 연결)·큰 평가손익 %·금액·현재가 흐름·청산 기준 막대·2열 표·확인 필요 띠.
// 상태 라벨은 목록 카드와 같은 slot.status를 씀(서버 끊김·시세 끊김을 두 곳이 다르게 말하지 않게).
// 손익은 수수료 반영 값(지금 정리하면) — Rust 봇이 계산해 보냄. 서버·웹 기록은 수수료 전 금액임.
import { TriangleAlert } from "lucide-react";
import {
  formatAgo,
  formatAmount,
  formatElapsed,
  formatFee,
  formatMoney,
  formatPercent,
  formatPrice,
  formatQty,
  moneyDigits,
  toneOf,
} from "../../state/format";
import { alertOf, labelOf, serverCell, STALE_MS } from "../../state/runStatus";
import type { MacroInfo, RunMode, RunSlot, RunStatus, Tone } from "../../types";
import { ExitBar } from "./ExitBar";
import { PriceChart } from "./PriceChart";
import { useRunClock } from "./useElapsed";

/** 웹(껄무새 내 에이전트) 기록과 이 카드의 손익이 다른 이유. */
export const WEB_GROSS_NOTE = "웹 기록은 수수료 전 금액이에요";

export function StatusCard({
  run,
  macro,
  mode = "live",
  phase,
  status,
  prices,
  pricesSince = 0,
}: {
  run: RunStatus;
  macro: MacroInfo;
  mode?: RunMode;
  phase: RunSlot["phase"];
  status: RunSlot["status"];
  prices: number[];
  /** 그래프 첫 점을 받은 시각(epoch ms). 0이면 덮는 시간을 보이지 않음. */
  pricesSince?: number;
}) {
  // 실행 중에는 1초마다 다시 그림 — 서버 소식이 끊긴 지 얼마인지도 이 시각으로 셈.
  const { now, elapsed } = useRunClock(run);
  const label = labelOf(run, phase, status);
  const live = !run.finished && phase === "running" && status.tone === "ok";
  const toneClass = run.finished ? "" : `tone-${status.tone as Tone}`;
  const server = serverCell(run, now);
  const alert = alertOf(run, mode);
  const age = run.updatedAt > 0 ? now - run.updatedAt : 0;
  const stale = !run.finished && phase === "running" && age > STALE_MS;
  const q = macro.quote;
  const mock = mode === "mock";
  const stateClass = run.finished ? (run.finished.status === "error" ? "is-error" : "is-finished") : "is-running";
  const holding = run.inPosition && run.entryPrice > 0;
  // 실현손익 — 수수료를 알면 수수료 반영 값(Rust realized_net), 모르면 서버 값(수수료 전)에 그렇다고 밝힘.
  const realized = run.realizedNet ?? run.realized;
  const realizedNote = run.realizedNet === null && run.realized !== 0 ? "수수료 제외 전" : "";
  const futures = macro.market === "futures";
  const feeText = run.fees === null ? (futures ? "알 수 없음(선물)" : "알 수 없음") : formatFee(run.fees, q);
  // 큰 %의 기준 — 수수료 반영이면 투입금(수량×평단 + 진입 수수료) 대비, 모르면 평단 대비 가격.
  const basis = run.unrealizedFeeAware ? "투입금 대비" : "평단 대비 · 수수료 제외 전";
  const leveraged = futures && macro.leverage > 1;

  return (
    <section className={`status-card ${stateClass}`} aria-label="실행 상태">
      {alert && (
        <div className="uncertain-banner" role="alert">
          <TriangleAlert size={16} aria-hidden="true" />
          {alert}
        </div>
      )}

      <div className="status-head">
        <span className={`run-label ${stateClass} ${toneClass}`} title={label}>
          {!run.finished && <span className={`status-dot ${live ? "is-live" : ""}`} aria-hidden="true" />}
          <span className="run-label-text">{label}</span>
        </span>
        <span className="status-meta num" title="경과 시간">
          경과 {formatElapsed(elapsed)}
        </span>
        {!run.finished && (
          <span className={`status-meta ${server.tone}`} title="서버 연결">
            <span aria-hidden="true">● </span>서버 {server.text}
          </span>
        )}
      </div>

      <div className="pnl-row">
        <div className="pnl" aria-live="polite">
          <span className="eyebrow">{mock ? "평가손익 · 지금 정리하면 · 모의" : "평가손익 · 지금 정리하면"}</span>
          {run.inPosition ? (
            <>
              <div className="pnl-figures">
                <span className={`pnl-pct num tone-${toneOf(run.unrealizedPct)}`}>
                  {formatPercent(run.unrealizedPct)}
                </span>
                <span className={`pnl-amount num tone-${toneOf(run.unrealized, moneyDigits(q))}`}>
                  {formatMoney(run.unrealized, q)}
                </span>
              </div>
              <span className="pnl-basis">{basis}</span>
              {leveraged && (
                <span className={`pnl-margin num tone-${toneOf(run.unrealizedPct * macro.leverage)}`}>
                  증거금 대비 {formatPercent(run.unrealizedPct * macro.leverage)} ({macro.leverage}배)
                </span>
              )}
            </>
          ) : (
            <div className="pnl-figures">
              <span className="pnl-flat">{run.finished ? "무포지션" : "무포지션 · 신호 대기 중"}</span>
            </div>
          )}
        </div>
        <div className="pnl-chart">
          <PriceChart
            points={prices}
            entry={run.inPosition ? run.entryPrice : undefined}
            quote={q}
            spanMs={pricesSince > 0 && run.updatedAt > pricesSince ? run.updatedAt - pricesSince : 0}
          />
        </div>
      </div>

      {!run.finished && (
        <ExitBar
          rules={macro.exits}
          side={macro.side}
          quote={q}
          entry={holding ? run.entryPrice : 0}
          price={run.price}
        />
      )}

      <dl className="run-table">
        <div>
          <dt>현재가</dt>
          <dd className="num">
            {run.updatedAt === 0 && !run.finished ? (
              "—"
            ) : (
              <>
                {formatPrice(run.price, q)} <span className="unit">{q}</span>
                {!run.finished && run.dayChangePct !== null && (
                  <span
                    className={`day-change tone-${toneOf(run.dayChangePct)}`}
                    title="전일 종가 대비 등락률(거래소 기준)"
                  >
                    {" "}
                    전일 {formatPercent(run.dayChangePct)}
                  </span>
                )}
                {!run.finished && (run.priceStaleSince > 0 || stale) && (
                  <span className={`price-age ${stale || run.priceStaleSince > 0 ? "tone-warn" : ""}`}>
                    {" "}
                    · {run.priceStaleSince > 0 ? "마지막 확인 가격" : `${formatAgo(age)} 갱신`}
                  </span>
                )}
              </>
            )}
          </dd>
        </div>
        <div>
          <dt>평단</dt>
          <dd className="num">{holding ? `${formatPrice(run.entryPrice, q)} ${q}` : "—"}</dd>
        </div>
        <div>
          <dt title="지금 포지션에 들어간 돈 — 수량×평단 + 진입 수수료">투입금</dt>
          <dd className="num">{run.inPosition && run.invested > 0 ? formatAmount(run.invested, q) : "없음"}</dd>
        </div>
        <div>
          <dt>보유 수량</dt>
          <dd className="num">{run.inPosition ? `${formatQty(run.qty)} ${macro.baseAsset}` : "없음"}</dd>
        </div>
        <div>
          <dt>낸 수수료</dt>
          <dd className={`num ${run.fees === null ? "is-unknown" : ""}`}>{feeText}</dd>
        </div>
        <div>
          <dt title={WEB_GROSS_NOTE}>{mock ? "이번 실행 실현손익 · 모의" : "이번 실행 실현손익"}</dt>
          <dd className={`num tone-${toneOf(realized, moneyDigits(q))}`} title={WEB_GROSS_NOTE}>
            {formatMoney(realized, q)}
            {realizedNote && <span className="realized-note">{realizedNote}</span>}
          </dd>
        </div>
      </dl>

      <p className="run-foot">
        {run.realizedNet !== null || run.unrealizedFeeAware ? "손익은 수수료를 반영했어요 · " : ""}
        {WEB_GROSS_NOTE}
      </p>
      {run.note && <p className="run-note">{run.note}</p>}
    </section>
  );
}
