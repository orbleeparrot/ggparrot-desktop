// 실행 모드 세그먼트 — 대기·끝남일 때만 바꿈. 국내 거래소는 테스트넷이 없음(modeChoices).
import { useRef, type KeyboardEvent } from "react";
import { useRunnerContext } from "../../state/context";
import { isIdle, MODE_LABELS } from "../../state/runs";
import type { RunMode, RunSlot } from "../../types";

export function ModeSegment({ slot }: { slot: RunSlot }) {
  const { act } = useRunnerContext();
  const group = useRef<HTMLDivElement>(null);
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
            tabIndex={selected ? 0 : -1}
            disabled={!editable}
            className={`segment ${selected ? "is-selected" : ""} segment-${mode}`}
            onClick={() => choose(mode)}
          >
            {MODE_LABELS[mode]}
          </button>
        );
      })}
      {!editable && (
        <span id={`${slot.id}-mode-lock`} className="sr-only">
          실행 중에는 모드를 바꿀 수 없어요
        </span>
      )}
    </div>
  );
}
