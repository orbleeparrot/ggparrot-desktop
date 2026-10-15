// 왼쪽 '내 매크로' 목록. ↑·↓·Home·End로 고르고(고른 카드를 따라 상세가 바뀜), 맨 아래 [+ 매크로 추가].
import { Plus } from "lucide-react";
import { useRef, type KeyboardEvent } from "react";
import { useRunnerContext } from "../../state/context";
import { RunCard } from "./RunCard";

export function RunList({ selectedId }: { selectedId: string | null }) {
  const { state, select, openLibrary } = useRunnerContext();
  const refs = useRef(new Map<string, HTMLButtonElement>());
  const runs = state.runs;
  const running = runs.filter((r) => r.phase === "running").length;

  function onKeyNav(e: KeyboardEvent<HTMLButtonElement>) {
    const ids = runs.map((r) => r.id);
    const i = ids.indexOf(e.currentTarget.dataset.runId ?? "");
    let next = -1;
    if (e.key === "ArrowDown") next = Math.min(ids.length - 1, i + 1);
    else if (e.key === "ArrowUp") next = Math.max(0, i - 1);
    else if (e.key === "Home") next = 0;
    else if (e.key === "End") next = ids.length - 1;
    if (next < 0) return;
    e.preventDefault();
    select(ids[next]);
    refs.current.get(ids[next])?.focus();
  }

  return (
    <nav className="run-list-panel" aria-label="내 매크로">
      <div className="panel-head">
        <h2 className="panel-title">내 매크로</h2>
        <span className="panel-count num">
          {running > 0 ? `${runs.length}개 중 ${running}개 실행 중` : `${runs.length}개`}
        </span>
      </div>
      <ul className="run-list">
        {runs.map((slot) => (
          <RunCard
            key={slot.id}
            ref={(el) => {
              if (el) refs.current.set(slot.id, el);
              else refs.current.delete(slot.id);
            }}
            slot={slot}
            selected={slot.id === selectedId}
            onKeyNav={onKeyNav}
          />
        ))}
      </ul>
      <button type="button" className="add-run" onClick={openLibrary}>
        <Plus size={16} aria-hidden="true" />
        매크로 추가
      </button>
    </nav>
  );
}
