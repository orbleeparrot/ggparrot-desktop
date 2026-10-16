// 왼쪽 '내 매크로' 목록. ↑·↓·Home·End로 고르고(고른 카드를 따라 상세가 바뀜), 맨 아래 [+ 매크로 추가].
// 순서 바꾸기: 카드 왼쪽 손잡이를 끌거나 Alt+↑·↓. 순서는 Rust가 저장함(move_run).
// - HTML 끌어 놓기(draggable)는 쓰지 않음 — Windows에서 창의 파일 끌어 놓기와 겹쳐 이벤트가 오지 않음. 포인터로 직접 처리.
// 목록이 길 때:
// - 머리('내 매크로 · N개')와 [+ 매크로 추가]는 위·아래에 붙어 있음(sticky). 가려진 카드가 있으면 그쪽 가장자리에 그림자.
// - 고른 카드는 늘 보이게 가장 가까운 만큼만 스크롤함. 끄는 중 위·아래 가장자리에 가까우면 저절로 스크롤함.
// - 넓은 창은 이 패널이, 좁은 창(한 열)은 문서가 스크롤함 — 둘 다 같은 계산(화면상 위치)으로 다룸.
import { Plus } from "lucide-react";
import { useEffect, useRef, useState, type KeyboardEvent, type PointerEvent } from "react";
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

/** 끄는 중 자동 스크롤 속도(px/프레임). 보이는 영역 [top, bottom]의 가장자리 zone 안으로 들어갈수록 빨라짐. 밖이면 0. */
export function edgeScrollSpeed(y: number, top: number, bottom: number, zone = 48, max = 14): number {
  if (y < top + zone) return -Math.ceil(max * Math.min(1, (top + zone - y) / zone));
  if (y > bottom - zone) return Math.ceil(max * Math.min(1, (y - (bottom - zone)) / zone));
  return 0;
}

/** 포인터 위치 → 놓을 자리. 한 줄 목록은 위아래, 여러 칸(좁은 창의 격자)은 좌우 절반으로 앞뒤를 가름. */
function slotAt(list: HTMLElement | null, x: number, y: number): number | null {
  const cards = Array.from(list?.querySelectorAll<HTMLElement>(":scope > .run-card") ?? []);
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

/** 목록이 실제로 보이는 세로 범위 — 붙어 있는 머리 아래 ~ 붙어 있는 [+ 매크로 추가] 위. */
function visibleBand(head: HTMLElement | null, foot: HTMLElement | null): [number, number] {
  return [head?.getBoundingClientRect().bottom ?? 0, foot?.getBoundingClientRect().top ?? window.innerHeight];
}

/** 카드 전체가 보이게 가장 가까운 만큼만 스크롤(시험 환경 jsdom에는 scrollIntoView가 없음). */
function nearest(button: HTMLElement) {
  const card = button.closest<HTMLElement>(".run-card");
  if (card && typeof card.scrollIntoView === "function") card.scrollIntoView({ block: "nearest" });
}

/** 넓은 창은 패널이 스크롤하고, 좁은 창은 문서가 스크롤함. */
function scrollBy(panel: HTMLElement | null, dy: number) {
  if (panel && panel.scrollHeight > panel.clientHeight + 1) panel.scrollTop += dy;
  else window.scrollBy(0, dy);
}

export function RunList({ selectedId }: { selectedId: string | null }) {
  const { state, select, openLibrary, act } = useRunnerContext();
  const refs = useRef(new Map<string, HTMLButtonElement>());
  const panel = useRef<HTMLElement>(null);
  const head = useRef<HTMLDivElement>(null);
  const list = useRef<HTMLUListElement>(null);
  const foot = useRef<HTMLDivElement>(null);
  const pointer = useRef({ x: 0, y: 0 });
  const [drag, setDrag] = useState<Drag | null>(null);
  const [more, setMore] = useState({ above: false, below: false });
  const runs = state.runs;
  const running = runs.filter((r) => r.phase === "running").length;
  const canMove = runs.length > 1;
  const dragging = drag !== null;

  /** 카드를 보이게 — 가장 가까운 만큼만(붙어 있는 머리·아래 단추 높이는 run.css scroll-margin으로 비움). */
  function reveal(id: string, focus = false) {
    const el = refs.current.get(id);
    if (!el) return;
    if (focus) el.focus({ preventScroll: true });
    nearest(el);
  }

  // 고른 카드가 바뀌면(매크로 추가로 새 카드가 골라진 때 등) 보이게 함.
  useEffect(() => {
    const el = selectedId ? refs.current.get(selectedId) : undefined;
    if (el) nearest(el);
  }, [selectedId]);

  // 위·아래로 가려진 카드가 있는지 — 스크롤·창 크기·목록 크기가 바뀔 때 다시 셈.
  useEffect(() => {
    let frame = 0;
    function measure() {
      cancelAnimationFrame(frame);
      frame = requestAnimationFrame(() => {
        const r = list.current?.getBoundingClientRect();
        if (!r) return;
        const [top, bottom] = visibleBand(head.current, foot.current);
        const above = r.top < top - 1;
        const below = r.bottom > bottom + 1;
        setMore((prev) => (prev.above === above && prev.below === below ? prev : { above, below }));
      });
    }
    const el = panel.current;
    el?.addEventListener("scroll", measure, { passive: true });
    window.addEventListener("scroll", measure, { passive: true });
    window.addEventListener("resize", measure);
    // 시험 환경(jsdom)에는 ResizeObserver가 없음.
    const ro = typeof ResizeObserver === "undefined" ? null : new ResizeObserver(measure);
    if (list.current) ro?.observe(list.current);
    measure();
    return () => {
      cancelAnimationFrame(frame);
      el?.removeEventListener("scroll", measure);
      window.removeEventListener("scroll", measure);
      window.removeEventListener("resize", measure);
      ro?.disconnect();
    };
  }, []);

  // 끄는 동안 포인터가 가장자리 가까이 있으면 프레임마다 조금씩 스크롤하고 놓을 자리를 다시 셈.
  useEffect(() => {
    if (!dragging) return;
    let frame = 0;
    function tick() {
      const [top, bottom] = visibleBand(head.current, foot.current);
      const { x, y } = pointer.current;
      const dy = edgeScrollSpeed(y, top, bottom);
      if (dy !== 0) {
        scrollBy(panel.current, dy);
        const slot = slotAt(list.current, x, y);
        if (slot !== null) setDrag((d) => (d && d.slot !== slot ? { ...d, slot } : d));
      }
      frame = requestAnimationFrame(tick);
    }
    frame = requestAnimationFrame(tick);
    return () => cancelAnimationFrame(frame);
  }, [dragging]);

  /** keyboard면 옮긴 카드에 포커스를 두고 보이게 함(마우스로 끈 때는 그대로). */
  function move(id: string, index: number, keyboard = false) {
    void act("move_run", { runId: id, index });
    if (keyboard) requestAnimationFrame(() => reveal(id, true));
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
    reveal(ids[next], true);
  }

  function onGrabStart(e: PointerEvent<HTMLElement>, id: string) {
    if (e.button !== 0) return;
    e.preventDefault();
    e.currentTarget.setPointerCapture(e.pointerId);
    pointer.current = { x: e.clientX, y: e.clientY };
    const from = runs.findIndex((r) => r.id === id);
    setDrag({ id, from, slot: from });
  }
  function onGrabMove(e: PointerEvent<HTMLElement>) {
    if (!drag) return;
    pointer.current = { x: e.clientX, y: e.clientY };
    const slot = slotAt(list.current, e.clientX, e.clientY);
    if (slot !== null && slot !== drag.slot) setDrag({ ...drag, slot });
  }
  function onGrabEnd() {
    if (!drag) return;
    const to = dropIndex(drag.from, drag.slot);
    if (to !== drag.from) move(drag.id, to);
    setDrag(null);
  }

  return (
    <nav
      ref={panel}
      className={`run-list-panel ${more.above ? "has-more-above" : ""} ${more.below ? "has-more-below" : ""}`}
      aria-label="내 매크로"
    >
      <div ref={head} className="panel-head">
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
      <div ref={foot} className="run-list-foot">
        <button type="button" className="add-run" onClick={openLibrary}>
          <Plus size={16} aria-hidden="true" />
          매크로 추가
        </button>
      </div>
    </nav>
  );
}
