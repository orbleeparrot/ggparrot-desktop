// 카드의 주 버튼 — 대기·끝남이면 [▶ 시작], 실행 중이면 [■ 종료](고를 게 있으면 ▾), 확인·종료 중이면 꺼진 버튼.
import { LoaderCircle, Play } from "lucide-react";
import { useRunnerContext } from "../../state/context";
import { primaryAction, stopChoices } from "../../state/runs";
import type { RunSlot } from "../../types";
import { StopMenu } from "./StopMenu";

export function RunAction({ slot }: { slot: RunSlot }) {
  const { startRun, askStop } = useRunnerContext();
  const action = primaryAction(slot.phase);
  const name = slot.macro.symbol;
  if (action.kind === "start") {
    return (
      <button
        type="button"
        className="btn btn-primary btn-sm"
        aria-label={`${name} 시작`}
        onClick={() => startRun(slot.id)}
      >
        <Play size={12} fill="currentColor" aria-hidden="true" />
        시작
      </button>
    );
  }
  if (action.kind === "stop") {
    return <StopMenu label={`${name} 종료`} choices={stopChoices(slot)} onChoose={(mode) => askStop(slot.id, mode)} />;
  }
  return (
    <button type="button" className="btn btn-ghost btn-sm" disabled>
      <LoaderCircle size={13} className="spin" aria-hidden="true" />
      {action.label}
    </button>
  );
}
