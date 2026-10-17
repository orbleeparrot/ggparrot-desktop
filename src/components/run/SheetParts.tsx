// 시작 · 종료 시트의 칸 — 제목 바로 아래 테두리 칸(회색 줄글을 두지 않음).
// - WaitBox: '이 매크로는 바로 사지 않아요' — 모래시계 · 큰 제목 · 조건 문장 · 숫자 세 칸(실전 시작 확인 시트 안)
// - WaitFacts: 숫자 세 칸만(모의 · 테스트넷의 '바로 사지 않아요' 시트는 제목이 이미 그 말)
// - StopFacts: 평가손익(지금 정리하면) · 정리할 수량 · 할 일 · 되돌릴 수 없어요
import { Hourglass, TriangleAlert } from "lucide-react";
import { formatMoney, formatPercent, formatQty, moneyDigits, toneOf } from "../../state/format";
import type { StopChoice } from "../../state/runs";
import type { EntryPreview, RunSlot } from "../../types";
import { rough } from "./EntryWait";

export function WaitFacts({ typical }: { typical: EntryPreview["typical"] | null | undefined }) {
  if (!typical) return null;
  return (
    <dl className="ew-facts">
      <div>
        <dt>평소 기다린 시간</dt>
        <dd className="num">{typical.avg_ms ? rough(typical.avg_ms) : "드묾"}</dd>
      </div>
      <div>
        <dt>최근 {rough(typical.span_ms).replace("약 ", "")}</dt>
        <dd className="num">{typical.episodes}번 진입</dd>
      </div>
      <div>
        <dt>가장 길게 기다린 때</dt>
        <dd className="num">{typical.longest_ms ? rough(typical.longest_ms) : "—"}</dd>
      </div>
    </dl>
  );
}

export function WaitBox({ preview }: { preview: EntryPreview }) {
  return (
    <section className="wait-box" aria-label="이 매크로는 바로 사지 않아요">
      <h3 className="wait-box-title">
        <span className="wait-box-icon" aria-hidden="true">
          <Hourglass size={17} />
        </span>
        이 매크로는 바로 사지 않아요
      </h3>
      <p className="wait-box-sentence">{preview.sentence} 조건이 맞을 때만 사요.</p>
      <WaitFacts typical={preview.typical} />
    </section>
  );
}

export function StopFacts({ slot, choice }: { slot: RunSlot | undefined; choice: StopChoice | undefined }) {
  const run = slot?.run;
  const holding = !!run?.inPosition && run.qty > 0;
  const q = slot?.macro.quote ?? "USDT";
  const closing = choice?.mode === "close_and_stop" && slot?.mode !== "mock";
  return (
    <div className="sheet-box">
      {holding && run && (
        <div className="sheet-box-row is-figure">
          <span>{slot?.mode === "mock" ? "평가손익 · 지금 정리하면 · 모의" : "평가손익 · 지금 정리하면"}</span>
          <b className={`num tone-${toneOf(run.unrealized, moneyDigits(q))}`}>
            {formatMoney(run.unrealized, q)}
            <small className={`tone-${toneOf(run.unrealizedPct)}`}>{formatPercent(run.unrealizedPct)}</small>
          </b>
        </div>
      )}
      {holding && run && closing && (
        <div className="sheet-box-row">
          <span>정리할 수량</span>
          <b className="num">
            {formatQty(run.qty)} {slot?.macro.baseAsset} · 시장가
          </b>
        </div>
      )}
      <p className="sheet-box-row is-text">{choice?.hint ?? "매크로를 끝내요."}</p>
      {choice?.danger && (
        <p className="sheet-box-row is-warn">
          <TriangleAlert size={14} aria-hidden="true" />
          되돌릴 수 없어요
        </p>
      )}
    </div>
  );
}
