// 오른쪽 상세 — 머리줄(종목·방향·거래소)·모드·시작/종료·막힌 이유·상태 카드·활동 기록.
// - 대기·끝남: 모드 세그먼트 + [시작]. 그 밖(확인·시작·실행·종료 중): 모드를 바꿀 수 없어 세그먼트 대신
//   "실전으로 실행 중" 배지와 매크로 설명 한 줄을 두고 오른쪽에 종료 단추.
import { CircleAlert, LoaderCircle, Play, Settings } from "lucide-react";
import { useEffect } from "react";
import { useRunnerContext } from "../../state/context";
import { isIdle, MODE_LABELS, primaryAction, runningModeText, settingsTargetOf, stopChoices } from "../../state/runs";
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
  /** 그래프 점마다 받은 시각(epoch ms). */
  priceTimes?: number[];
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

/** 실행 중 조작 줄 왼쪽 — 모드 배지와 매크로 설명 한 줄(전체는 마우스를 올리면). */
function RunningMode({ slot }: { slot: RunSlot }) {
  const summary = slot.macro.summary;
  return (
    <div className="detail-running">
      <span
        className={`detail-mode-badge mode-${slot.mode}`}
        title={`${MODE_LABELS[slot.mode]} 모드 — 실행 중에는 모드를 바꿀 수 없어요`}
      >
        {runningModeText(slot.mode, slot.phase)}
      </span>
      {summary && (
        <p className="detail-running-summary" title={summary}>
          {summary}
        </p>
      )}
    </div>
  );
}

export function RunDetail({
  slot,
  activity,
  prices,
  priceTimes = [],
  onBack,
  loadActivity,
  activityFailed = false,
}: Props) {
  const { openSettings } = useRunnerContext();
  const m = slot.macro;
  const r = slot.readiness;
  const target = settingsTargetOf(r.needs);
  const idle = isIdle(slot.phase);
  const showBlocker = idle && !r.canStart && !!r.blocker;

  useEffect(() => loadActivity(slot.id), [slot.id, loadActivity]);

  return (
    <section id="run-detail" className="run-detail" aria-labelledby="detail-title">
      <DetailHeader slot={slot} onBack={onBack} />

      {/* 세그먼트(또는 실행 중 배지)와 주 버튼은 한 줄. 모드 설명은 세그먼트 단추의 풍선 도움말(ModeSegment). */}
      <div className={`detail-controls ${idle ? "" : "is-running"}`}>
        {idle ? (
          <div className="detail-mode">
            <ModeSegment slot={slot} />
          </div>
        ) : (
          <RunningMode slot={slot} />
        )}
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
      {/* 실행 중이면 설명은 조작 줄 안(RunningMode). 끝난 뒤에는 한 줄로 줄여 상태 카드를 밀어내지 않게. */}
      {m.summary && idle && (
        <p className={`macro-summary ${slot.run ? "is-compact" : ""}`} title={slot.run ? m.summary : undefined}>
          {m.summary}
        </p>
      )}
      {idle && <CapitalRow slot={slot} />}

      {slot.run ? (
        <StatusCard
          run={slot.run}
          macro={m}
          mode={slot.mode}
          phase={slot.phase}
          status={slot.status}
          prices={prices}
          priceTimes={priceTimes}
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
