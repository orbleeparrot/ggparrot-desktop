// 오른쪽 상세 — 머리줄(종목·방향·거래소·모드·시작/종료)·막힌 이유·상태 카드·활동 기록.
import { ArrowLeft, CircleAlert, LoaderCircle, Play, Settings, TriangleAlert } from "lucide-react";
import { useEffect } from "react";
import { useRunnerContext } from "../../state/context";
import {
  canStop,
  isIdle,
  MARKET_LABELS,
  MODE_NOTES,
  settingsTargetOf,
  SIDE_LABELS,
  stopChoices,
} from "../../state/runs";
import type { Activity, RunSlot } from "../../types";
import { ActivityFeed } from "../activity/ActivityFeed";
import { ExchangeLogo } from "../ExchangeLogo";
import { CapitalRow } from "./CapitalRow";
import { IdleCard } from "./IdleCard";
import { StatusCard } from "./StatusCard";
import { ModeSegment } from "./ModeSegment";

interface Props {
  slot: RunSlot;
  activity: Activity[];
  prices: number[];
  onBack?: () => void;
  loadActivity(runId: string): void;
  activityFailed?: boolean;
}

function Controls({ slot }: { slot: RunSlot }) {
  const { startRun, askStop } = useRunnerContext();
  if (isIdle(slot.phase)) {
    return (
      <button
        type="button"
        className={`btn btn-lg ${slot.mode === "live" ? "btn-live" : "btn-primary"} detail-start`}
        onClick={() => startRun(slot.id)}
      >
        <Play size={15} fill="currentColor" aria-hidden="true" />
        {slot.mode === "live" ? "실전으로 시작" : "시작"}
      </button>
    );
  }
  if (canStop(slot.phase)) {
    return (
      <div className="detail-stop">
        {stopChoices(slot).map((c) => (
          <button
            key={c.mode}
            type="button"
            className={`btn ${c.danger ? "btn-danger" : "btn-ghost"} btn-lg`}
            onClick={() => askStop(slot.id, c.mode)}
          >
            {c.label}
          </button>
        ))}
      </div>
    );
  }
  return (
    <button type="button" className="btn btn-ghost btn-lg" disabled>
      <LoaderCircle size={15} className="spin" aria-hidden="true" />
      {slot.phase === "checking" ? "연결 확인 중…" : "종료 처리 중…"}
    </button>
  );
}

function MacroNotes({ slot }: { slot: RunSlot }) {
  const m = slot.macro;
  if (!m.exchange)
    return (
      <p className="note note-danger">
        이 앱이 지원하지 않는 거래소({m.exchangeName || "알 수 없음"})예요. 바이낸스 · 업비트 · 빗썸 매크로만 실행할 수
        있어요.
      </p>
    );
  if (m.signature === "modified")
    return (
      <p className="note note-warn">
        <TriangleAlert size={14} aria-hidden="true" />이 매크로는 웹에서 받은 원본과 설정이 달라요(투자 금액 등). 이
        기기에서 바꾼 설정으로 실행돼요.
      </p>
    );
  if (m.signature === "unsigned")
    return <p className="note note-muted">서명이 없는 예전 파일이에요. 실행은 되지만 원본인지 확인할 수 없어요.</p>;
  return null;
}

export function RunDetail({ slot, activity, prices, onBack, loadActivity, activityFailed = false }: Props) {
  const { openSettings } = useRunnerContext();
  const m = slot.macro;
  const r = slot.readiness;
  const target = settingsTargetOf(r.needs);
  const showBlocker = isIdle(slot.phase) && !r.canStart && !!r.blocker;

  useEffect(() => loadActivity(slot.id), [slot.id, loadActivity]);

  const facts = [
    m.source === "web" ? "웹 매크로" : "파일 매크로",
    m.strategy,
    MARKET_LABELS[m.market],
    m.leverage > 1 ? `${m.leverage}배` : "",
  ].filter(Boolean);

  return (
    <section id="run-detail" className="run-detail" aria-labelledby="detail-title">
      <div className="detail-head">
        {onBack && (
          <button type="button" className="icon-btn detail-back" aria-label="내 매크로 목록으로" onClick={onBack}>
            <ArrowLeft size={18} />
          </button>
        )}
        <ExchangeLogo exchange={m.exchange} size={30} />
        <div className="detail-name">
          <h2 id="detail-title" className="detail-title">
            <span className="num">{m.symbol}</span>
            <span className={`side side-${m.side}`}>{SIDE_LABELS[m.side]}</span>
            <span className="detail-exchange">{m.exchangeLabel || m.exchangeName}</span>
          </h2>
          <p className="detail-sub" title={m.title}>
            <span className="detail-file">{m.title}</span>
            {facts.map((f) => (
              <span key={f}> · {f}</span>
            ))}
          </p>
        </div>
      </div>

      <div className="detail-controls">
        <div className="detail-mode">
          <ModeSegment slot={slot} />
          <p className={`mode-note ${slot.mode === "live" ? "is-live" : ""}`}>{MODE_NOTES[slot.mode]}</p>
        </div>
        <Controls slot={slot} />
      </div>

      {showBlocker && (
        <div className="blocker" role="note">
          <CircleAlert size={15} aria-hidden="true" />
          <span className="blocker-text">{r.blocker}</span>
          {target && (
            <button type="button" className="btn btn-ghost btn-sm" onClick={() => openSettings(target)}>
              <Settings size={13} aria-hidden="true" />
              설정에서 입력
            </button>
          )}
        </div>
      )}
      <MacroNotes slot={slot} />
      {m.summary && !slot.run && <p className="macro-summary">{m.summary}</p>}
      {isIdle(slot.phase) && <CapitalRow slot={slot} />}

      {slot.run ? (
        <StatusCard run={slot.run} macro={m} mode={slot.mode} phase={slot.phase} status={slot.status} prices={prices} />
      ) : (
        <IdleCard />
      )}

      <ActivityFeed
        runId={slot.id}
        items={activity}
        loadFailed={activityFailed}
        onRetry={() => loadActivity(slot.id)}
      />
    </section>
  );
}
