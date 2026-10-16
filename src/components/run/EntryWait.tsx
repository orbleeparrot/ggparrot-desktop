// 진입 대기 — 조건을 기다리는 매크로가 아직 안 샀을 때 상태 카드의 평가손익 자리에 놓임.
// - 위계는 글자 크기 · 굵기로만: 조건 한 문장(크게) → 지금과의 거리 → 막대 → 세 칸(라벨 위 값).
// - 값은 서버가 엔진과 같은 규칙으로 계산해 하트비트로 보낸 것(entry_wait) — 웹 내 에이전트와 같은 숫자.
import type { EntryWait } from "../../types";

/** epoch ms → "23:00"(이 PC 시각). */
export function clock(ms: number): string {
  const d = new Date(ms);
  return `${String(d.getHours()).padStart(2, "0")}:${String(d.getMinutes()).padStart(2, "0")}`;
}

/** "18분" · "2시간 14분" · "1일 3시간". */
export function span(ms: number): string {
  const m = Math.max(0, Math.round(ms / 60_000));
  if (m < 60) return `${m}분`;
  const h = Math.floor(m / 60);
  if (h < 24) return m % 60 ? `${h}시간 ${m % 60}분` : `${h}시간`;
  const d = Math.floor(h / 24);
  return h % 24 ? `${d}일 ${h % 24}시간` : `${d}일`;
}

/** 대략값 — "약 9시간" · "약 2일". */
export function rough(ms: number): string {
  const h = ms / 3_600_000;
  if (h < 1) return `약 ${Math.max(1, Math.round(ms / 60_000))}분`;
  if (h < 48) return `약 ${Math.round(h)}시간`;
  return `약 ${Math.round(h / 24)}일`;
}

/** 평가손익 자리 — 조건 문장 · 거리 · 막대. */
export function EntryWaitHead({ wait }: { wait: EntryWait }) {
  const at = `${Math.round(Math.min(1, Math.max(0, wait.meter?.progress ?? 0)) * 1000) / 10}%`;
  return (
    <div className={`entry-wait ${wait.reached ? "is-reached" : ""}`}>
      <p className="ew-sentence">{wait.sentence}</p>
      {wait.distance && <p className="ew-distance num">{wait.distance}</p>}
      {wait.meter && (
        <div
          className="ew-meter"
          role="img"
          aria-label={`${wait.meter.from.label} ${wait.meter.from.value}에서 ${wait.meter.to.label} ${wait.meter.to.value}까지 중 지금 ${wait.meter.now}`}
        >
          <div className="ew-bar" style={{ ["--at" as string]: at }}>
            <i className="ew-fill" />
            <i className="ew-pin" />
          </div>
          <div className="ew-ends">
            <span>
              {wait.meter.from.label} <b className="num">{wait.meter.from.value}</b>
            </span>
            <span>
              {wait.meter.to.label} <b className="num">{wait.meter.to.value}</b>
            </span>
          </div>
        </div>
      )}
    </div>
  );
}

/** 세 칸 — 다음 확인 · 평소 기다린 시간 · 지금까지 확인. */
export function EntryWaitFacts({ wait, now }: { wait: EntryWait; now: number }) {
  const next = wait.next_check_ms;
  const t = wait.typical;
  return (
    <dl className="ew-facts">
      <div>
        <dt>다음 확인</dt>
        <dd className="num">{wait.realtime ? "실시간" : next ? clock(next) : "—"}</dd>
        <dd className="ew-sub">
          {wait.realtime
            ? "가격이 닿는 순간"
            : next
              ? `${next - now >= 60_000 ? `${span(next - now)} 뒤` : "곧"} · ${wait.interval_label} 봉 마감`
              : ""}
        </dd>
      </div>
      <div>
        <dt>평소 기다린 시간</dt>
        <dd className="num">{t ? (t.avg_ms ? rough(t.avg_ms) : "드묾") : "—"}</dd>
        {t && (
          <dd className="ew-sub">
            최근 {rough(t.span_ms).replace("약 ", "")} 동안 {t.episodes}번
          </dd>
        )}
      </div>
      <div>
        <dt>지금까지 확인</dt>
        <dd className="num">{wait.checks === null ? "계속" : `${wait.checks}번`}</dd>
        <dd className="ew-sub">
          {wait.realtime
            ? "3초마다"
            : wait.last_check_ms
              ? `마지막 ${clock(wait.last_check_ms)} · ${wait.log[0]?.hit ? "조건 맞음" : "아직"}`
              : `${wait.interval_label} 봉 마감마다`}
        </dd>
      </div>
    </dl>
  );
}
