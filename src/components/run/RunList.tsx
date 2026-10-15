// 왼쪽 '내 매크로' 목록. ↑·↓·Home·End로 고르고(고른 카드를 따라 상세가 바뀜), 맨 아래 [+ 매크로 추가].
// 순서 바꾸기: 카드 왼쪽 손잡이를 끌거나 Alt+↑·↓. 순서는 Rust가 저장함(move_run).
// - HTML 끌어 놓기(draggable)는 쓰지 않음 — Windows에서 창의 파일 끌어 놓기와 겹쳐 이벤트가 오지 않음. 포인터로 직접 처리.
import { Plus } from "lucide-react";
import { useRef, useState, type KeyboardEvent, type PointerEvent } from "react";
import { useRunnerContext } from "../../state/context";
import { RunCard } from "./RunCard";

interface Drag {
  id: string;
  from: number;
  /** 놓을 자리 — 이 번호의 카드 앞(카드 수와 같으면 맨 뒤). */
  slot: number;
}

/** 끌어서 놓을 자리 → 옮긴 뒤 번호(자기 자리를 빼고 셈). */
export function dropIndex(from: number, slot: number): number {
  return slot > from ? slot - 1 : slot;
}

export function RunList({ selectedId }: { selectedId: string | null }) {
  const { state, select, openLibrary, act } = useRunnerContext();
  const refs = useRef(new Map<string, HTMLButtonElement>());
  const list = useRef<HTMLUListElement>(null);
  const [drag, setDrag] = useState<Drag | null>(null);
  const runs = state.runs;
  const running = runs.filter((r) => r.phase === "running").length;
  const canMove = runs.length > 1;

  /** keyboard면 옮긴 카드에 포커스를 둠(마우스로 끈 때는 그대로). */
  function move(id: string, index: number, keyboard = false) {
    void act("move_run", { runId: id, index });
    if (keyboard) requestAnimationFrame(() => refs.current.get(id)?.focus());
  }

  function onKeyNav(e: KeyboardEvent<HTMLButtonElement>) {
    const ids = runs.map((r) => r.id);
    const id = e.currentTarget.dataset.runId ?? "";
    const i = ids.indexOf(id);
    if (e.altKey && (e.key === "ArrowUp" || e.key === "ArrowDown")) {
      e.preventDefault();
      const to = e.key === "ArrowUp" ? i - 1 : i + 1;
      if (to >= 0 && to < ids.length) move(id, to, true);
      return;
    }
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

  /** 포인터 위치 → 놓을 자리. 한 줄 목록은 위아래, 여러 칸(좁은 창의 격자)은 좌우 절반으로 앞뒤를 가름. */
  function slotAt(x: number, y: number): number | null {
    const cards = Array.from(list.current?.querySelectorAll<HTMLElement>(":scope > .run-card") ?? []);
    if (!cards.length) return null;
    const grid = cards.length > 1 && Math.abs(cards[0].offsetTop - cards[1].offsetTop) < 4;
    for (let i = 0; i < cards.length; i++) {
      const r = cards[i].getBoundingClientRect();
      if (grid ? y < r.bottom && x < r.right : y < r.bottom) {
        const before = grid ? x < r.left + r.width / 2 : y < r.top + r.height / 2;
        return before ? i : i + 1;
      }
    }
    return cards.length;
  }

  function onGrabStart(e: PointerEvent<HTMLElement>, id: string) {
    if (e.button !== 0) return;
    e.preventDefault();
    e.currentTarget.setPointerCapture(e.pointerId);
    const from = runs.findIndex((r) => r.id === id);
    setDrag({ id, from, slot: from });
  }
  function onGrabMove(e: PointerEvent<HTMLElement>) {
    if (!drag) return;
    const slot = slotAt(e.clientX, e.clientY);
    if (slot !== null && slot !== drag.slot) setDrag({ ...drag, slot });
  }
  function onGrabEnd() {
    if (!drag) return;
    const to = dropIndex(drag.from, drag.slot);
    if (to !== drag.from) move(drag.id, to);
    setDrag(null);
  }

  return (
    <nav className="run-list-panel" aria-label="내 매크로">
      <div className="panel-head">
        <h2 className="panel-title">내 매크로</h2>
        <span className="panel-count num">
          {running > 0 ? `${runs.length}개 중 ${running}개 실행 중` : `${runs.length}개`}
        </span>
      </div>
      <ul ref={list} className={`run-list ${drag ? "is-sorting" : ""}`}>
        {runs.map((slot, i) => (
          <RunCard
            key={slot.id}
            ref={(el) => {
              if (el) refs.current.set(slot.id, el);
              else refs.current.delete(slot.id);
            }}
            slot={slot}
            selected={slot.id === selectedId}
            onKeyNav={onKeyNav}
            grab={
              canMove
                ? {
                    onPointerDown: (e) => onGrabStart(e, slot.id),
                    onPointerMove: onGrabMove,
                    onPointerUp: onGrabEnd,
                    onPointerCancel: () => setDrag(null),
                  }
                : undefined
            }
            sorting={
              !drag
                ? ""
                : drag.id === slot.id
                  ? "is-dragging"
                  : drag.slot === i && dropIndex(drag.from, i) !== drag.from
                    ? "is-drop-before"
                    : drag.slot === i + 1 && i === runs.length - 1 && dropIndex(drag.from, i + 1) !== drag.from
                      ? "is-drop-after"
                      : ""
            }
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
