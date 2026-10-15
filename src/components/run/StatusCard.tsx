// 실행 상태 카드 — 큰 평가손익 %·금액·현재가 흐름·2열 표·확인 필요 띠·종료 라벨.
// 상태 라벨은 목록 카드와 같은 slot.status를 씀(서버 끊김·시세 끊김을 두 곳이 다르게 말하지 않게).
import { TriangleAlert } from "lucide-react";
import {
  formatAgo,
  formatElapsed,
  formatMoney,
  formatPercent,
  formatPrice,
  formatQty,
  moneyDigits,
  toneOf,
} from "../../state/format";
import { SIDE_LABELS } from "../../state/runs";
import { alertOf, labelOf, serverCell, STALE_MS } from "../../state/runStatus";
import type { MacroInfo, RunMode, RunSlot, RunStatus, Tone } from "../../types";
import { Sparkline } from "../Sparkline";
import { useElapsed } from "./useElapsed";

export function StatusCard({
  run,
  macro,
  mode = "live",
  phase,
  status,
  prices,
}: {
  run: RunStatus;
  macro: MacroInfo;
  mode?: RunMode;
  phase: RunSlot["phase"];
  status: RunSlot["status"];
  prices: number[];
}) {
  const elapsed = useElapsed(run);
  // useElapsed가 실행 중에는 1초마다 다시 그리므로 여기서 읽는 지금 시각도 그만큼 새로움.
  const now = Date.now();
  const label = labelOf(run, phase, status);
  const live = !run.finished && phase === "running" && status.tone === "ok";
  const toneClass = run.finished ? "" : `tone-${status.tone as Tone}`;
  const server = serverCell(run, now);
  const alert = alertOf(run, mode);
  const age = run.updatedAt > 0 ? now - run.updatedAt : 0;
  const stale = !run.finished && phase === "running" && age > STALE_MS;
  const q = macro.quote;
  const side = SIDE_LABELS[macro.side];
  const stateClass = run.finished ? (run.finished.status === "error" ? "is-error" : "is-finished") : "is-running";
  // 화면의 실현손익은 수수료를 뺀 값 — 서버에 보고하는 값(run.realized)은 그대로 둠. 모르면 수수료 전이라고 밝힘.
  const net = run.fees === null ? run.realized : run.realized - run.fees;
  const feeNote =
    run.fees === null
      ? run.realized !== 0
        ? "수수료 제외 전"
        : ""
      : run.fees > 0
        ? `수수료 ${formatPrice(run.fees, q)} ${q} 반영`
        : "";

  return (
    <section className={`status-card ${stateClass}`} aria-label="실행 상태">
      {alert && (
        <div className="uncertain-banner" role="alert">
          <TriangleAlert size={16} aria-hidden="true" />
          {alert}
        </div>
      )}

      <div className="pnl-row">
        <div className="pnl" aria-live="polite">
          <span className="pnl-top">
            <span className="eyebrow">{mode === "mock" ? "평가손익 · 모의" : "평가손익"}</span>
            <span className={`run-label ${stateClass} ${toneClass}`} title={label}>
              {!run.finished && <span className={`status-dot ${live ? "is-live" : ""}`} aria-hidden="true" />}
              <span className="run-label-text">{label}</span>
            </span>
          </span>
          {run.inPosition ? (
            <div className="pnl-figures">
              <span className={`pnl-pct num tone-${toneOf(run.unrealizedPct)}`}>
                {formatPercent(run.unrealizedPct)}
              </span>
              <span className={`pnl-amount num tone-${toneOf(run.unrealized, moneyDigits(q))}`}>
                {formatMoney(run.unrealized, q)}
              </span>
            </div>
          ) : (
            <div className="pnl-figures">
              <span className="pnl-flat">{run.finished ? "무포지션" : "무포지션 · 신호 대기 중"}</span>
            </div>
          )}
        </div>
        <div className="pnl-chart">
          <Sparkline points={prices} entry={run.inPosition ? run.entryPrice : undefined} />
        </div>
      </div>

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
          <dt>포지션</dt>
          <dd className="num">{run.inPosition ? `${side} · ${formatQty(run.qty)} ${macro.baseAsset}` : "없음"}</dd>
        </div>
        <div>
          <dt>평단</dt>
          <dd className="num">
            {run.inPosition && run.entryPrice > 0 ? `${formatPrice(run.entryPrice, q)} ${q}` : "—"}
          </dd>
        </div>
        <div>
          <dt>{mode === "mock" ? "이번 실행 실현손익 · 모의" : "이번 실행 실현손익"}</dt>
          <dd className={`num tone-${toneOf(net, moneyDigits(q))}`}>
            {formatMoney(net, q)}
            {feeNote && <span className="realized-note">{feeNote}</span>}
          </dd>
        </div>
        <div>
          <dt>경과 시간</dt>
          <dd className="num">{formatElapsed(elapsed)}</dd>
        </div>
        <div>
          <dt>서버 연결</dt>
          <dd className={server.tone}>
            {!run.finished && <span aria-hidden="true">● </span>}
            {server.text}
          </dd>
        </div>
      </dl>

      {run.note && <p className="run-note">{run.note}</p>}
    </section>
  );
}
