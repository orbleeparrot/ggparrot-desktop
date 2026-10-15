// 오른쪽 상세 — 머리줄(종목·방향·거래소)·모드·시작/종료·막힌 이유·상태 카드·활동 기록.
import { CircleAlert, LoaderCircle, Play, Settings } from "lucide-react";
import { useEffect } from "react";
import { useRunnerContext } from "../../state/context";
import { isIdle, primaryAction, settingsTargetOf, stopChoices } from "../../state/runs";
import type { Activity, RunSlot } from "../../types";
import { ActivityFeed } from "../activity/ActivityFeed";
import { CapitalRow } from "./CapitalRow";
import { DetailHeader } from "./DetailHeader";
import { IdleCard } from "./IdleCard";
import { MacroNotes } from "./MacroNotes";
import { ModeSegment } from "./ModeSegment";
import { StatusCard } from "./StatusCard";

interface Props {
  slot: RunSlot;
  activity: Activity[];
  prices: number[];
  /** 그래프 첫 점을 받은 시각(epoch ms). */
  pricesSince?: number;
  onBack?: () => void;
  loadActivity(runId: string): void;
  activityFailed?: boolean;
}

function Controls({ slot }: { slot: RunSlot }) {
  const { startRun, askStop } = useRunnerContext();
  const action = primaryAction(slot.phase);
  if (action.kind === "start") {
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
  if (action.kind === "stop") {
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
      {action.label}
    </button>
  );
}

export function RunDetail({
  slot,
  activity,
  prices,
  pricesSince = 0,
  onBack,
  loadActivity,
  activityFailed = false,
}: Props) {
  const { openSettings } = useRunnerContext();
  const m = slot.macro;
  const r = slot.readiness;
  const target = settingsTargetOf(r.needs);
  const showBlocker = isIdle(slot.phase) && !r.canStart && !!r.blocker;

  useEffect(() => loadActivity(slot.id), [slot.id, loadActivity]);

  return (
    <section id="run-detail" className="run-detail" aria-labelledby="detail-title">
      <DetailHeader slot={slot} onBack={onBack} />

      {/* 세그먼트와 주 버튼은 한 줄·같은 높이. 모드 설명은 세그먼트 단추의 풍선 도움말(ModeSegment). */}
      <div className="detail-controls">
        <div className="detail-mode">
          <ModeSegment slot={slot} />
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
      <MacroNotes slot={slot} blockerShown={showBlocker} />
      {m.summary &&
        (slot.run ? (
          // 실행 중에는 한 줄로 줄여 상태 카드를 밀어내지 않게 — 전체는 마우스를 올리면 보임.
          <p className="macro-summary is-compact" title={m.summary}>
            {m.summary}
          </p>
        ) : (
          <p className="macro-summary">{m.summary}</p>
        ))}
      {isIdle(slot.phase) && <CapitalRow slot={slot} />}

      {slot.run ? (
        <StatusCard
          run={slot.run}
          macro={m}
          mode={slot.mode}
          phase={slot.phase}
          status={slot.status}
          prices={prices}
          pricesSince={pricesSince}
        />
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
