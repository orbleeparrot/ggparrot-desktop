// 끝난 실행 결과 — 종료 이유 · 이번 실행 실현손익(수수료 반영) 큰 숫자 · 투자 금액 대비 % · 거래 · 낸 수수료 · 실행 시간 · 마지막 가격.
// - 모두 실행기가 이미 가진 값(Rust 봇이 보낸 실현손익 · 수수료, 활동 기록의 체결 줄).
// - 다시 시작은 위 조작 줄의 [시작]이 같은 설정으로 함 — 여기 단추를 또 두지 않음.
import {
  formatElapsed,
  formatFee,
  formatMoney,
  formatPercent,
  formatPrice,
  moneyDigits,
  toneOf,
} from "../../state/format";
import type { Activity, MacroInfo, RunMode, RunStatus } from "../../types";
import { WEB_GROSS_NOTE } from "./StatusCard";

/** 이번 실행의 진입 · 정리 체결 수 — 이번 시작 줄부터 셈. 시작 줄이 기록에서 밀려났으면 모름(null). */
export function tradeCounts(items: Activity[], startedAt: number): { buys: number; sells: number } | null {
  // 시작 줄은 실행 시작 시각 무렵에 남음 — 몇 초 앞서 남을 수 있어 여유를 둠.
  let from = -1;
  for (let i = items.length - 1; i >= 0; i--) {
    if (items[i].kind === "start" && items[i].at >= startedAt - 10_000) {
      from = i;
      break;
    }
  }
  if (from < 0) return null;
  let buys = 0;
  let sells = 0;
  for (const a of items.slice(from)) {
    if (a.dev) continue;
    if (a.kind === "buy") buys += 1;
    else if (a.kind === "sell") sells += 1;
  }
  return { buys, sells };
}

export function FinishedCard({
  run,
  macro,
  mode,
  activity,
}: {
  run: RunStatus;
  macro: MacroInfo;
  mode: RunMode;
  activity: Activity[];
}) {
  const finished = run.finished;
  if (!finished) return null;
  const q = macro.quote;
  const mock = mode === "mock";
  const error = finished.status === "error";
  // 실현손익 — 수수료를 알면 수수료 반영 값(Rust realized_net), 모르면 서버 값(수수료 전)에 그렇다고 밝힘.
  const realized = run.realizedNet ?? run.realized;
  const gross = run.realizedNet === null && run.realized !== 0;
  const pct = macro.capital ? (realized / macro.capital) * 100 : null;
  const trades = tradeCounts(activity, run.startedAt);
  const futures = macro.market === "futures";
  const fees = run.fees === null ? (futures ? "알 수 없음(선물)" : "알 수 없음") : formatFee(run.fees, q);

  return (
    <section className={`finished-card ${error ? "is-error" : ""}`} aria-label="이번 실행 결과">
      <div className="finished-head">
        <span className={`finished-chip ${error ? "tone-danger" : ""}`}>{error ? "오류로 멈춤" : "종료됨"}</span>
        {finished.note && <span className="finished-note">{finished.note}</span>}
      </div>
      <div className="finished-pnl">
        <span className="eyebrow" title={WEB_GROSS_NOTE}>
          {mock ? "이번 실행 실현손익 · 모의" : "이번 실행 실현손익"}
        </span>
        <div className="pnl-figures">
          <span className={`pnl-pct num tone-${toneOf(realized, moneyDigits(q))}`}>{formatMoney(realized, q)}</span>
          {pct !== null && (
            <span className={`pnl-amount num tone-${toneOf(pct)}`}>투자 금액 대비 {formatPercent(pct)}</span>
          )}
          {gross && <span className="pnl-tag">수수료 제외 전</span>}
        </div>
      </div>
      <dl className="finished-stats">
        <div>
          <dt>거래</dt>
          <dd className="num">
            {trades ? `${trades.buys + trades.sells}번` : "—"}
            {trades && trades.buys + trades.sells > 0 && (
              <small>
                진입 {trades.buys} · 정리 {trades.sells}
              </small>
            )}
          </dd>
        </div>
        <div>
          <dt>낸 수수료</dt>
          <dd className={`num ${run.fees === null ? "is-unknown" : ""}`}>{fees}</dd>
        </div>
        <div>
          <dt>실행 시간</dt>
          <dd className="num">{formatElapsed(Math.max(0, finished.at - run.startedAt))}</dd>
        </div>
        <div>
          <dt>마지막 가격</dt>
          <dd className="num">{run.price > 0 ? formatPrice(run.price, q) : "—"}</dd>
        </div>
      </dl>
    </section>
  );
}
