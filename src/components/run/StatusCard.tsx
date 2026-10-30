// 실행 상태 카드 — 큰 평가손익 숫자 하나·금액·현재가 흐름·청산 기준 막대·접힌 2열 표·확인 필요 띠.
// - 상태(실행 중·경과·서버 연결)는 상세 머리 제목 옆(RunState)이 말함 — 이 카드에는 다시 적지 않음.
// - 1040×720 창에서 아래 활동 기록이 세 줄 넘게 보이도록 낮게 짬(그래프 46px·막대 한 줄·표 줄 간격 좁힘).
// 손익은 수수료 반영 값(지금 정리하면) — Rust 봇이 계산해 보냄. 서버·웹 기록은 수수료 전 금액임.
// - 큰 숫자 밑에 '평단 대비 가격'을 따로 두지 않음 — 수수료만큼만 다른 같은 값이고 막대가 이미 위치로 보임.
import { ChevronDown, ChevronUp, TriangleAlert } from "lucide-react";
import { useState } from "react";
import {
  formatAgo,
  formatAmount,
  formatFee,
  formatMoney,
  formatPercent,
  formatPrice,
  formatQty,
  moneyDigits,
  toneOf,
} from "../../state/format";
import { trailSpanLabel } from "../../state/priceTrail";
import { STALE_MS, statusAlert } from "../../state/runStatus";
import type { MacroInfo, RunMode, RunSlot, RunStatus } from "../../types";
import { CloseChip, EntryWaitDetail, EntryWaitHead } from "./EntryWait";
import { ExitBar } from "./ExitBar";
import { CandlePlot } from "./CandlePlot";
import { PriceChart } from "./PriceChart";
import { useRunClock } from "./useElapsed";

// 현재가·평단·투입금·수량·수수료·실현손익 표는 처음엔 접어 둠(웹 내 에이전트와 같게). 펼친 상태는 이 PC에만 기억.
export const DETAILS_KEY = "ggp.runner.status.details";

function useDetailsOpen(): [boolean, () => void] {
  const [open, setOpen] = useState(() => {
    try {
      return window.localStorage.getItem(DETAILS_KEY) === "open";
    } catch {
      return false;
    }
  });
  const toggle = () =>
    setOpen((current) => {
      const next = !current;
      try {
        window.localStorage.setItem(DETAILS_KEY, next ? "open" : "closed");
      } catch {
        // 저장소를 못 쓰면 이 화면에서만 펼침.
      }
      return next;
    });
  return [open, toggle];
}

/** 웹(껄무새 내 에이전트) 기록과 이 카드의 손익이 다른 이유. */
export const WEB_GROSS_NOTE = "웹 기록은 수수료 전 금액이에요";

export function StatusCard({
  run,
  macro,
  mode = "live",
  phase,
  prices,
  priceTimes = [],
}: {
  run: RunStatus;
  macro: MacroInfo;
  mode?: RunMode;
  phase: RunSlot["phase"];
  prices: number[];
  /** 그래프 점마다 받은 시각(epoch ms). 비면 덮는 시간을 보이지 않음. */
  priceTimes?: number[];
}) {
  // 실행 중에는 1초마다 다시 그림 — 현재가가 얼마나 오래됐는지도 이 시각으로 셈.
  const { now } = useRunClock(run);
  const [detailsOpen, toggleDetails] = useDetailsOpen();
  const alert = statusAlert(run, mode, macro);
  const age = run.updatedAt > 0 ? now - run.updatedAt : 0;
  const stale = !run.finished && phase === "running" && age > STALE_MS;
  const q = macro.quote;
  const mock = mode === "mock";
  const stateClass = run.finished ? (run.finished.status === "error" ? "is-error" : "is-finished") : "is-running";
  const holding = run.inPosition && run.entryPrice > 0;
  // 진입 대기 — 조건을 기다리는 매크로가 아직 안 샀고 서버가 대기 값을 보내는 동안.
  const waiting = !run.finished && !run.inPosition && run.entryWait !== null;
  // 실현손익 — 수수료를 알면 수수료 반영 값(Rust realized_net), 모르면 서버 값(수수료 전)에 그렇다고 밝힘.
  const realized = run.realizedNet ?? run.realized;
  const realizedNote = run.realizedNet === null && run.realized !== 0 ? "수수료 제외 전" : "";
  const futures = macro.market === "futures";
  const feeText = run.fees === null ? (futures ? "알 수 없음(선물)" : "알 수 없음") : formatFee(run.fees, q);
  const leveraged = futures && macro.leverage > 1;

  return (
    <section className={`status-card ${stateClass}`} aria-label="실행 상태">
      {alert && (
        <div className={`uncertain-banner ${alert.tone === "warn" ? "is-warn" : ""}`} role="alert">
          <TriangleAlert size={16} aria-hidden="true" />
          {alert.text}
        </div>
      )}

      <div className={`pnl-row${run.chart ? " has-candles" : ""}`}>
        <div className="pnl" aria-live="polite">
          <span className="pnl-head">
            <span className="eyebrow">
              {waiting
                ? `${run.entryWait?.interval_label ?? ""} 봉${mock ? " · 모의" : ""}`
                : mock
                  ? "평가손익 · 지금 정리하면 · 모의"
                  : "평가손익 · 지금 정리하면"}
            </span>
            {waiting && run.entryWait && <CloseChip wait={run.entryWait} />}
            {/* 수수료를 모르면 큰 %는 평단 대비 가격(수수료 전) — 이름 옆에 그렇다고 밝힘 */}
            {!waiting && run.inPosition && !run.unrealizedFeeAware && (
              <span className="pnl-tag">평단 대비 · 수수료 제외 전</span>
            )}
          </span>
          {waiting && run.entryWait ? (
            <EntryWaitHead wait={run.entryWait} />
          ) : run.inPosition ? (
            <>
              <div className="pnl-figures">
                <span className={`pnl-pct num tone-${toneOf(run.unrealizedPct)}`}>
                  {formatPercent(run.unrealizedPct)}
                </span>
                <span className={`pnl-amount num tone-${toneOf(run.unrealized, moneyDigits(q))}`}>
                  {formatMoney(run.unrealized, q)}
                </span>
              </div>
              {leveraged && (
                <span className="pnl-mini">
                  <span className="pnl-mini-label">증거금 대비 · {macro.leverage}배</span>
                  <span className={`pnl-mini-value num tone-${toneOf(run.unrealizedPct * macro.leverage)}`}>
                    {formatPercent(run.unrealizedPct * macro.leverage)}
                  </span>
                </span>
              )}
            </>
          ) : (
            <div className="pnl-figures">
              <span className="pnl-flat">{run.finished ? "무포지션" : "무포지션 · 신호 대기 중"}</span>
            </div>
          )}
        </div>
        {/* 서버가 판단하면 앱은 봉을 들고 있지 않음 — 현재가 흐름만 작게.
            앱이 직접 판단하면 봉이 있으므로 아래에 **크게** 따로 그림. */}
        {!run.chart && (
          <div className="pnl-chart">
            <PriceChart
              points={prices}
              times={priceTimes}
              entry={run.inPosition ? run.entryPrice : undefined}
              buy={waiting ? run.entryWait?.buy_line?.price : undefined}
              quote={q}
              span={trailSpanLabel(run.startedAt, priceTimes[0] ?? 0, run.updatedAt)}
            />
          </div>
        )}
      </div>

      {/* 조건을 보는 게 이 화면의 목적이라 차트가 손익 바로 아래, 카드 폭 전체를 씀. */}
      {run.chart && (
        <CandlePlot
          chart={run.chart}
          price={run.price}
          entry={run.inPosition ? run.entryPrice : 0}
          quote={q}
          live={run.chartLive}
          intervalLabel={macro.interval || run.chart.interval}
        />
      )}

      {waiting && run.entryWait && <EntryWaitDetail wait={run.entryWait} />}

      {!run.finished && (
        <ExitBar
          rules={macro.exits}
          side={macro.side}
          quote={q}
          entry={holding ? run.entryPrice : 0}
          price={run.price}
        />
      )}

      <button type="button" className="run-details-toggle" aria-expanded={detailsOpen} onClick={toggleDetails}>
        <span>{detailsOpen ? "자세히 접기" : "현재가 · 평단 · 수량 자세히"}</span>
        {detailsOpen ? <ChevronUp size={14} aria-hidden="true" /> : <ChevronDown size={14} aria-hidden="true" />}
      </button>

      {detailsOpen && (
        <>
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
              <dd className="num" title={run.inPosition ? `${formatQty(run.qty)} ${macro.baseAsset}` : undefined}>
                {run.inPosition ? `${formatQty(run.qty)} ${macro.baseAsset}` : "없음"}
              </dd>
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
        </>
      )}
      {run.note && <p className="run-note">{run.note}</p>}
    </section>
  );
}
