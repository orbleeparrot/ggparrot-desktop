// 카드의 작은 주 단추 — 대기·끝남이면 [▶], 실행 중이면 [■](고를 게 있으면 메뉴), 확인·종료 중이면 도는 표시만 있는 꺼진 단추.
// - 글자 없는 아이콘 단추(이름은 aria-label·풍선 도움말). [▶]·[■]는 카드에 마우스를 올리거나 키보드 포커스가 있을 때만
//   보임(run.css) — 목록이 늘 단추로 어수선하지 않게. 도는 표시는 진행 상태라 늘 보임.
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
        className="btn btn-brand-outline btn-sm is-icon"
        aria-label={`${name} 시작`}
        title="시작"
        onClick={() => startRun(slot.id)}
      >
        <Play size={12} fill="currentColor" aria-hidden="true" />
      </button>
    );
  }
  if (action.kind === "stop") {
    const choices = stopChoices(slot);
    return (
      <StopMenu
        label={`${name} ${choices.length === 1 ? choices[0].label : "종료"}`}
        choices={choices}
        onChoose={(mode) => askStop(slot.id, mode)}
        iconOnly
      />
    );
  }
  return (
    <button
      type="button"
      className="btn btn-ghost btn-sm is-icon run-action-busy"
      aria-label={`${name} ${action.label}`}
      title={action.label}
      disabled
    >
      <LoaderCircle size={14} className="spin" aria-hidden="true" />
    </button>
  );
}
