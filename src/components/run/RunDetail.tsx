// 오른쪽 상세 — 머리줄(종목·칩)·모드·시작/종료·돈·안전 칸·상태 카드(끝났으면 결과, 시작 전이면 '시작 전 확인')·활동 기록.
// - 대기·끝남: 모드 카드(세그먼트 + [시작], 아래 한 줄은 고른 모드의 설명 modeNote). 그 밖(확인·시작·실행·종료 중):
//   모드를 바꿀 수 없어 모드 카드를 없애고, 상태("● 실행 중 06:02" — RunState)는 제목 옆, 끝내는 단추는 머리 오른쪽.
// - 키가 없으면 [시작] 자리가 [○○ 키 연결하기](연결 도우미), 회원 키가 없으면 [회원 키 입력하기] — 막힌 이유 띠를 따로 두지 않음.
import { KeyRound, LoaderCircle, Play } from "lucide-react";
import { useEffect } from "react";
import { useRunnerContext } from "../../state/context";
import { isIdle, modeNote, primaryAction, stopChoices } from "../../state/runs";
import type { Activity, RunSlot } from "../../types";
import { ActivityFeed } from "../activity/ActivityFeed";
import { CapitalRow } from "./CapitalRow";
import { DetailHeader } from "./DetailHeader";
import { FinishedCard } from "./FinishedCard";
import { BundleBar } from "./BundleBar";
import { LiveSettings } from "./LiveSettings";
import { MacroNotes } from "./MacroNotes";
import { ModeSegment } from "./ModeSegment";
import { PreStartChecks } from "./PreStartChecks";
import { RunState } from "./RunState";
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
  const { startRun, askStop, state, act, openSettings } = useRunnerContext();
  const action = primaryAction(slot.phase);
  const needs = slot.readiness.needs;
  if (action.kind === "start" && needs?.kind === "key") {
    // 키가 없으면 [시작]을 눌러도 막힘 — 그 자리에서 바로 연결 도우미로(시트를 다 본 뒤에 뜨지 않게).
    const card = state.settings.slots.find((c) => c.slot === needs.slot);
    const exchange = card?.exchange ?? slot.macro.exchange;
    return (
      <button
        type="button"
        className="btn btn-lg btn-primary detail-start"
        onClick={() => exchange && void act("wizard_open", { exchange, slot: needs.slot })}
      >
        <KeyRound size={15} aria-hidden="true" />
        {card?.label ?? slot.macro.exchangeLabel} 키 연결하기
      </button>
    );
  }
  if (action.kind === "start" && needs?.kind === "member") {
    return (
      <button
        type="button"
        className="btn btn-lg btn-primary detail-start"
        onClick={() => openSettings({ kind: "member" })}
      >
        <KeyRound size={15} aria-hidden="true" />
        회원 키 입력하기
      </button>
    );
  }
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
        {stopChoices(slot).map((c, i, all) => (
          <button
            key={c.mode}
            type="button"
            // 끝내는 단추는 빨간 채움 — 두 개면 '매크로만 종료'(포지션은 그대로)만 보조 단추
            className={`btn ${all.length > 1 && i === 0 ? "btn-ghost" : "btn-danger-solid"} btn-lg`}
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
  priceTimes = [],
  onBack,
  loadActivity,
  activityFailed = false,
}: Props) {
  const m = slot.macro;
  const r = slot.readiness;
  const idle = isIdle(slot.phase);
  // 키·회원 키 말고 막힌 이유는 '시작 전 확인' 첫 줄이 말함.
  const blockerShown = idle && !r.canStart && !!r.blocker && !r.needs;
  // 시작 전 확인 — 아직 돌린 적 없거나(결과 카드가 없음), 끝났지만 다시 시작할 수 없을 때.
  const showChecks = idle && (!slot.run?.finished || !r.canStart);

  useEffect(() => loadActivity(slot.id), [slot.id, loadActivity]);

  return (
    <section id="run-detail" className="run-detail" aria-labelledby="detail-title">
      {/* 넓은 창(1360px~)에서는 왼쪽 칸 — 활동 기록이 오른쪽 칸을 위아래로 다 차지함. 그보다 좁으면 감싸개 없이 한 줄로 쌓임(run.css). */}
      <div className="detail-main">
        {idle ? (
          <>
            <DetailHeader slot={slot} onBack={onBack} />
            {/* 세그먼트와 주 버튼은 한 줄, 그 아래에 고른 모드의 설명 한 줄. */}
            <div className="detail-mode-card">
              <div className="detail-controls">
                <div className="detail-mode">
                  <ModeSegment slot={slot} />
                </div>
                <Controls slot={slot} />
              </div>
              <p className={`mode-note mode-${slot.mode}`}>{modeNote(slot.mode, slot.macro.market)}</p>
            </div>
          </>
        ) : (
          <DetailHeader
            slot={slot}
            onBack={onBack}
            runState={<RunState slot={slot} />}
            actions={<Controls slot={slot} />}
          />
        )}

        {/* 묶음의 한 레그라는 안내 + 묶음 전체 종료 — 반만 남는 것이 가장 나쁨. */}
        <BundleBar slot={slot} />
        <MacroNotes slot={slot} blockerShown={blockerShown} />
        {/* 돌고 있는 그대로 금액·조건 바꾸기 — 실행 중에만 보임(끝난 실행은 서버가 칸을 주지 않음). */}
        <LiveSettings slot={slot} />
        {/* 실행 중에는 설명을 두지 않음(청산 막대가 기준을 그림으로 보임). 끝난 뒤에는 한 줄로 줄여 결과 카드를 밀어내지 않게. */}
        {m.summary && idle && (
          <p className={`macro-summary ${slot.run ? "is-compact" : ""}`} title={slot.run ? m.summary : undefined}>
            {m.summary}
          </p>
        )}
        {idle && <CapitalRow slot={slot} />}

        {slot.run && !slot.run.finished ? (
          <StatusCard
            run={slot.run}
            macro={m}
            mode={slot.mode}
            phase={slot.phase}
            prices={prices}
            priceTimes={priceTimes}
          />
        ) : slot.run?.finished && idle ? (
          <FinishedCard run={slot.run} macro={m} mode={slot.mode} activity={activity} />
        ) : null}
        {showChecks && <PreStartChecks slot={slot} />}
      </div>

      <ActivityFeed
        runId={slot.id}
        items={activity}
        loadFailed={activityFailed}
        onRetry={() => loadActivity(slot.id)}
      />
    </section>
  );
}
