// 실행 모드 세그먼트 — 대기·끝남일 때만 바꿈. 국내 거래소는 테스트넷이 없음(modeChoices).
// - 모드 설명은 따로 줄을 두지 않고 단추마다 풍선 도움말(title)·접근성 설명(aria-describedby)으로 둠.
// - 실전 시작 확인은 시작할 때 대화상자로 한 번 더 함(useRunDialogs).
import { useId, useRef, type KeyboardEvent } from "react";
import { useRunnerContext } from "../../state/context";
import { isIdle, MODE_LABELS, MODE_NOTES } from "../../state/runs";
import type { RunMode, RunSlot } from "../../types";

export function ModeSegment({ slot }: { slot: RunSlot }) {
  const { act } = useRunnerContext();
  const group = useRef<HTMLDivElement>(null);
  const noteId = useId();
  const choices = slot.modeChoices;
  const editable = isIdle(slot.phase);

  function choose(mode: RunMode) {
    if (mode !== slot.mode) void act("set_run_mode", { runId: slot.id, mode });
  }

  function onKeyDown(e: KeyboardEvent<HTMLDivElement>) {
    const d =
      e.key === "ArrowRight" || e.key === "ArrowDown" ? 1 : e.key === "ArrowLeft" || e.key === "ArrowUp" ? -1 : 0;
    if (!d || !editable) return;
    e.preventDefault();
    const next = choices[(choices.indexOf(slot.mode) + d + choices.length) % choices.length];
    choose(next);
    requestAnimationFrame(() => group.current?.querySelector<HTMLButtonElement>(`[data-mode="${next}"]`)?.focus());
  }

  return (
    <div
      ref={group}
      className={`segmented ${editable ? "" : "is-locked"}`}
      role="radiogroup"
      aria-label="실행 모드"
      aria-disabled={!editable || undefined}
      aria-describedby={editable ? undefined : `${slot.id}-mode-lock`}
      style={{ gridTemplateColumns: `repeat(${choices.length}, 1fr)` }}
      onKeyDown={onKeyDown}
      title={editable ? undefined : "실행 중에는 모드를 바꿀 수 없어요"}
    >
      {choices.map((mode) => {
        const selected = mode === slot.mode;
        return (
          <button
            key={mode}
            type="button"
            role="radio"
            data-mode={mode}
            aria-checked={selected}
            aria-describedby={`${noteId}-${mode}`}
            title={editable ? MODE_NOTES[mode] : undefined}
            tabIndex={selected ? 0 : -1}
            disabled={!editable}
            className={`segment ${selected ? "is-selected" : ""} segment-${mode}`}
            onClick={() => choose(mode)}
          >
            {MODE_LABELS[mode]}
          </button>
        );
      })}
      {choices.map((mode) => (
        <span key={mode} id={`${noteId}-${mode}`} className="sr-only">
          {MODE_NOTES[mode]}
        </span>
      ))}
      {!editable && (
        <span id={`${slot.id}-mode-lock`} className="sr-only">
          실행 중에는 모드를 바꿀 수 없어요
        </span>
      )}
    </div>
  );
}
