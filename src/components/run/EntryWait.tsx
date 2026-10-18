// 진입 대기 — 조건을 기다리는 매크로가 아직 안 샀을 때 상태 카드의 평가손익 자리에 놓임.
// - 지금 상태 한 줄(크게) → 짧은 설명 → 막대 → 조건 체크. 긴 조건 문장은 늘어놓지 않음.
// - 조건이 둘(사는 선 + 20봉 평균)이면 가격 막대 하나에 두 선을 그리고 지금 점을 찍음 — 점이 두 선 사이에 있으면
//   '사는 선은 넘었는데 평균선 아래라 안 산다'가 한눈에 보임(2026-10-17 KRW-FORT).
// - 값은 서버가 엔진과 같은 규칙으로 계산해 하트비트로 보낸 것(entry_wait) — 웹 내 에이전트와 같은 숫자.
import { useEffect, useState, type CSSProperties } from "react";
import { Check } from "lucide-react";
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

/** 봉 간격 → ms("15m" · "1h" · "4h" · "1d" · "1w"). 모르는 꼴이면 null(막대 없이 글자만). */
export function intervalMs(interval: string): number | null {
  const m = /^(\d+)([mhdw])$/.exec(interval.trim());
  if (!m) return null;
  const unit = { m: 60_000, h: 3_600_000, d: 86_400_000, w: 604_800_000 }[m[2] as "m" | "h" | "d" | "w"];
  return Number(m[1]) * unit;
}

/** 봉 마감 칩 글자 — "03:15 판정 · 12분 남음" · 1분 안이면 "03:15 판정 · 곧". 지정가는 "실시간 확인". */
export function closeChipText(wait: EntryWait, now: number): { at: string; left: string } | null {
  if (wait.realtime) return { at: "실시간 확인", left: "" };
  const at = wait.next_check_ms;
  if (!at) return null;
  return { at: `${clock(at)} 판정`, left: at - now >= 60_000 ? `${span(at - now)} 남음` : "곧" };
}

/** '15분 봉' 옆 칩 — 이번 봉이 얼마나 지났는지 작은 막대 + 판정 시각 · 남은 시간. */
export function CloseChip({ wait }: { wait: EntryWait }) {
  const [now, setNow] = useState(() => Date.now());
  useEffect(() => {
    const t = setInterval(() => setNow(Date.now()), 1000);
    return () => clearInterval(t);
  }, []);
  const text = closeChipText(wait, now);
  if (!text) return null;
  const len = wait.realtime ? null : intervalMs(wait.interval);
  const done = len && wait.next_check_ms ? Math.min(1, Math.max(0, 1 - (wait.next_check_ms - now) / len)) : null;
  return (
    <span
      className="ew-close num"
      title={wait.realtime ? "3초마다 조건을 봐요" : `${wait.interval_label} 봉이 끝날 때 조건을 다시 봐요`}
    >
      {done !== null && (
        <span className="ew-close-bar" aria-hidden="true">
          <i style={{ width: pct(done) }} />
        </span>
      )}
      {text.at}
      {text.left && <span className="ew-close-left">· {text.left}</span>}
    </span>
  );
}

const pct = (x: number) => `${Math.round(Math.min(1, Math.max(0, x)) * 1000) / 10}%`;
/** 위치는 --at 하나 — 꼬리표는 끝에 가까울수록 안쪽으로 당김(left X% · translateX(-X%)). */
const place = (x: number) => ({ "--at": pct(x) }) as CSSProperties;

/** 값 막대 — 선들(사는 선 · 평균선 · 청산 기준)을 같은 눈금(왼쪽이 작은 값)에 놓고 지금 점을 찍음.
 *  사는 구역은 초록 — 올라가야 사면 선 오른쪽, 내려가야 사면(RSI · 지정가 · 눌림) 선 왼쪽. */
function Ladder({ ladder }: { ladder: NonNullable<EntryWait["ladder"]> }) {
  const marks = [...ladder.marks].sort((a, b) => a.pos - b.pos);
  const close = marks.length > 1 && marks[1].pos - marks[0].pos < 0.3; // 이름표가 겹치면 둘째 줄로
  return (
    <div
      className="ew-ladder"
      role="img"
      aria-label={`${marks.map((m) => `${m.label} ${m.value}`).join(" · ")} 중 지금 ${ladder.now.value}`}
    >
      <span className="ew-ladder-now num" style={place(ladder.now.pos)}>
        지금 {ladder.now.value}
      </span>
      <div className="ew-ladder-track">
        <i className={`ew-ladder-zone ${ladder.zone_side === "below" ? "is-below" : ""}`} style={place(ladder.zone)} />
        {marks.map((m) => (
          <i key={m.key} className={`ew-ladder-mark ${m.main ? "is-main" : ""}`} style={place(m.pos)} />
        ))}
        <i className="ew-ladder-dot" style={place(ladder.now.pos)} />
      </div>
      <div className={`ew-ladder-labels ${close ? "is-stacked" : ""}`}>
        {marks.map((m, i) => (
          <span
            key={m.key}
            className={`${m.main ? "is-main" : ""} ${close && i === 1 ? "is-low" : ""}`}
            style={place(m.pos)}
          >
            {m.label} <b className="num">{m.value}</b>
          </span>
        ))}
      </div>
    </div>
  );
}

/** 평가손익 자리(오른쪽은 가격 그래프) — 지금 상태 한 줄 · 짧은 설명. */
export function EntryWaitHead({ wait }: { wait: EntryWait }) {
  return (
    <div className={`entry-wait ${wait.reached ? "is-reached" : ""}`}>
      <p className="ew-sentence">{wait.headline || wait.sentence.replace(/\.$/, "")}</p>
      {wait.distance && <p className="ew-distance">{wait.distance}</p>}
    </div>
  );
}

/** 그 아래 카드 전체 폭 — 막대 · 조건 체크. 왼쪽 칸에 넣으면 오른쪽 그래프(46px) 위가 통째로 빈다. */
export function EntryWaitDetail({ wait }: { wait: EntryWait }) {
  const at = pct(wait.meter?.progress ?? 0);
  const conds = wait.conditions ?? [];
  const ladder = wait.scale ?? wait.ladder ?? null;
  return (
    <div className={`entry-wait-detail ${wait.reached ? "is-reached" : ""}`}>
      {ladder ? (
        <Ladder ladder={ladder} />
      ) : (
        wait.meter && (
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
        )
      )}
      {conds.length > 1 && (
        <ul className="ew-conds" aria-label="사는 조건">
          {conds.map((row) => (
            <li key={row.key} className={row.ok ? "is-ok" : "is-wait"}>
              <span className="ew-cond-mark" aria-label={row.ok ? "맞음" : "아직"}>
                {row.ok && <Check size={12} strokeWidth={3} aria-hidden="true" />}
              </span>
              <span className="ew-cond-label">{row.label}</span>
              {row.now && <span className="ew-cond-now num">{row.now}</span>}
            </li>
          ))}
        </ul>
      )}
    </div>
  );
}
