// 카드의 주 버튼 — 대기·끝남이면 [▶ 시작], 실행 중이면 [■ 종료](고를 게 있으면 ▾), 확인·종료 중이면 도는 표시만 있는 꺼진 버튼.
// - 진행 문구는 카드 상태 글자에 이미 있어 버튼은 아이콘만 둠(이름·풍선 도움말에는 문구).
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
    <button
      type="button"
      className="btn btn-ghost btn-sm run-action-busy"
      aria-label={`${name} ${action.label}`}
      title={action.label}
      disabled
    >
      <LoaderCircle size={14} className="spin" aria-hidden="true" />
    </button>
  );
}
