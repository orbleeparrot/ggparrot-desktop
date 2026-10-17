// 왼쪽 '내 매크로' 목록 — 확인 필요 · 실행 중 · 대기 세 묶음(색 · 칸 · 행 모양이 다름). ↑·↓·Home·End로 고르고
// (고른 카드를 따라 상세가 바뀜), 맨 아래 [+ 매크로 추가]. 매크로가 8개를 넘으면 맨 위에 찾기 칸.
// - 확인 필요: 노란 칸 안의 카드(머리줄 '확인 필요 N개'를 누르면 이 묶음으로 굴러옴) · 실행 중: 흰 카드 · 대기: 회색 칸의 한 줄 행
// - 대기 묶음은 머리를 눌러 접음(이 PC에 기억). 찾는 중에는 모두 펼침.
// 순서 바꾸기: 카드 왼쪽 손잡이를 끌거나 Alt+↑·↓ — 같은 묶음 안에서만. 순서는 Rust가 저장함(move_run).
// - HTML 끌어 놓기(draggable)는 쓰지 않음 — Windows에서 창의 파일 끌어 놓기와 겹쳐 이벤트가 오지 않음. 포인터로 직접 처리.
// 목록이 길 때:
// - 머리('내 매크로 · N개')와 [+ 매크로 추가]는 위·아래에 붙어 있음(sticky). 가려진 카드가 있으면 그쪽 가장자리에 그림자.
// - 고른 카드는 늘 보이게 가장 가까운 만큼만 스크롤함. 끄는 중 위·아래 가장자리에 가까우면 저절로 스크롤함.
// - 넓은 창은 이 패널이, 좁은 창(한 열)은 문서가 스크롤함 — 둘 다 같은 계산(화면상 위치)으로 다룸.
import { ChevronDown, Pause, Plus, Search, TriangleAlert } from "lucide-react";
import { useEffect, useRef, useState, type KeyboardEvent, type PointerEvent } from "react";
import { useRunnerContext } from "../../state/context";
import { GROUP_LABELS, globalIndex, groupOf, groupRuns, matches, type GroupKey } from "../../state/runGroups";
import { RunCard } from "./RunCard";

interface Drag {
  id: string;
  /** 끄는 카드의 묶음 — 그 묶음 안에서만 놓음. */
  group: GroupKey;
  /** 묶음 안 번호. */
  from: number;
  /** 놓을 자리 — 묶음 안 이 번호의 카드 앞(카드 수와 같으면 맨 뒤). */
  slot: number;
}

/** 찾기 칸을 보이는 매크로 수(이보다 많을 때). */
export const SEARCH_FROM = 8;
/** 대기 묶음을 접었는지 — 이 PC에만 기억. */
export const IDLE_KEY = "ggp.runner.list.idle";
/** 대기 묶음에서 처음 보이는 줄 수 — 더 있으면 [N개 모두 보기]. */
export const IDLE_PEEK = 3;

function readIdleOpen(): boolean {
  try {
    return window.localStorage.getItem(IDLE_KEY) !== "closed";
  } catch {
    return true;
  }
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
function visibleBand(head: HTMLElement | null, panel: HTMLElement | null): [number, number] {
  const bottom = panel ? Math.min(panel.getBoundingClientRect().bottom, window.innerHeight) : window.innerHeight;
  return [head?.getBoundingClientRect().bottom ?? 0, bottom];
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
  const body = useRef<HTMLDivElement>(null);
  const lists = useRef(new Map<GroupKey, HTMLUListElement>());
  const pointer = useRef({ x: 0, y: 0 });
  // 끄는 카드의 묶음 — 가장자리 자동 스크롤(effect)이 읽음.
  const dragGroup = useRef<GroupKey>("running");
  const [drag, setDrag] = useState<Drag | null>(null);
  const [more, setMore] = useState({ above: false, below: false });
  const [query, setQuery] = useState("");
  const [idleOpen, setIdleOpen] = useState(readIdleOpen);
  const [idleAll, setIdleAll] = useState(false);
  const runs = state.runs;
  const searchable = runs.length > SEARCH_FROM;
  const searching = searchable && query.trim() !== "";
  const groups = groupRuns(searching ? runs.filter((r) => matches(r, query)) : runs);
  const idleShown = idleOpen || searching;
  /** 묶음에서 보이는 카드 — 대기는 처음 IDLE_PEEK줄만(찾는 중 · 모두 보기 · 고른 카드가 뒤에 있으면 다). */
  function shownOf(g: (typeof groups)[number]) {
    if (g.key !== "idle") return g.items;
    if (!idleShown) return [];
    const at = g.items.findIndex((r) => r.id === selectedId);
    return idleAll || searching || at >= IDLE_PEEK ? g.items : g.items.slice(0, IDLE_PEEK);
  }
  // 화면에 보이는 순서 — ↑·↓가 오가는 차례(접힌 대기 묶음 · 가려진 대기 줄은 뺌).
  const visible = groups.flatMap((g) => shownOf(g));
  const canMove = runs.length > 1 && !searching;
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
        const r = body.current?.getBoundingClientRect();
        if (!r) return;
        const [top, bottom] = visibleBand(head.current, panel.current);
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
    if (body.current) ro?.observe(body.current);
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
    const group = dragGroup.current;
    let frame = 0;
    function tick() {
      const [top, bottom] = visibleBand(head.current, panel.current);
      const { x, y } = pointer.current;
      const dy = edgeScrollSpeed(y, top, bottom);
      if (dy !== 0) {
        scrollBy(panel.current, dy);
        const slot = slotAt(lists.current.get(group) ?? null, x, y);
        if (slot !== null) setDrag((d) => (d && d.slot !== slot ? { ...d, slot } : d));
      }
      frame = requestAnimationFrame(tick);
    }
    frame = requestAnimationFrame(tick);
    return () => cancelAnimationFrame(frame);
  }, [dragging]);

  /** 묶음 안에서 to(자기를 뺀 묶음 안 자리)로 옮김. keyboard면 옮긴 카드에 포커스를 두고 보이게 함. */
  function move(id: string, to: number, keyboard = false) {
    const slot = runs.find((r) => r.id === id);
    if (!slot) return;
    const groupIds = runs.filter((r) => groupOf(r) === groupOf(slot)).map((r) => r.id);
    void act("move_run", {
      runId: id,
      index: globalIndex(
        runs.map((r) => r.id),
        groupIds,
        id,
        to,
      ),
    });
    if (keyboard) requestAnimationFrame(() => reveal(id, true));
  }

  function onKeyNav(e: KeyboardEvent<HTMLButtonElement>) {
    const ids = visible.map((r) => r.id);
    const id = e.currentTarget.dataset.runId ?? "";
    const i = ids.indexOf(id);
    if (e.altKey && (e.key === "ArrowUp" || e.key === "ArrowDown")) {
      e.preventDefault();
      if (!canMove) return;
      const slot = runs.find((r) => r.id === id);
      if (!slot) return;
      const groupIds = runs.filter((r) => groupOf(r) === groupOf(slot)).map((r) => r.id);
      const at = groupIds.indexOf(id);
      const to = e.key === "ArrowUp" ? at - 1 : at + 1;
      if (to >= 0 && to < groupIds.length) move(id, to, true);
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

  function onGrabStart(e: PointerEvent<HTMLElement>, id: string, group: GroupKey, from: number) {
    if (e.button !== 0) return;
    e.preventDefault();
    e.currentTarget.setPointerCapture(e.pointerId);
    pointer.current = { x: e.clientX, y: e.clientY };
    dragGroup.current = group;
    setDrag({ id, group, from, slot: from });
  }
  function onGrabMove(e: PointerEvent<HTMLElement>) {
    if (!drag) return;
    pointer.current = { x: e.clientX, y: e.clientY };
    const slot = slotAt(lists.current.get(drag.group) ?? null, e.clientX, e.clientY);
    if (slot !== null && slot !== drag.slot) setDrag({ ...drag, slot });
  }
  function onGrabEnd() {
    if (!drag) return;
    const to = dropIndex(drag.from, drag.slot);
    if (to !== drag.from) move(drag.id, to);
    setDrag(null);
  }

  function toggleIdle() {
    setIdleOpen((open) => {
      try {
        window.localStorage.setItem(IDLE_KEY, open ? "closed" : "open");
      } catch {
        // 저장소를 못 쓰면 이 화면에서만 접음.
      }
      return !open;
    });
  }

  /** 끄는 중 카드 표시 — 같은 묶음 안에서만. */
  function sortingOf(group: GroupKey, i: number, count: number, id: string): string {
    if (!drag || drag.group !== group) return "";
    if (drag.id === id) return "is-dragging";
    if (drag.slot === i && dropIndex(drag.from, i) !== drag.from) return "is-drop-before";
    if (drag.slot === i + 1 && i === count - 1 && dropIndex(drag.from, i + 1) !== drag.from) return "is-drop-after";
    return "";
  }

  return (
    <nav
      ref={panel}
      className={`run-list-panel ${more.above ? "has-more-above" : ""} ${more.below ? "has-more-below" : ""}`}
      aria-label="내 매크로"
    >
      <div ref={head} className="panel-head">
        <h2 className="panel-title">내 매크로</h2>
        <button type="button" className="btn btn-ghost btn-sm panel-add" aria-label="매크로 추가" onClick={openLibrary}>
          <Plus size={14} aria-hidden="true" />
          추가
        </button>
      </div>
      <div ref={body} className="run-groups">
        {searchable && (
          <label className="run-search">
            <Search size={14} aria-hidden="true" />
            <input
              type="search"
              className="run-search-input"
              placeholder="종목 · 이름으로 찾기"
              aria-label="매크로 찾기"
              value={query}
              onChange={(e) => setQuery(e.target.value)}
            />
          </label>
        )}
        {searching && groups.length === 0 && <p className="run-empty">찾는 매크로가 없어요.</p>}
        {groups.map((g) => {
          const open = g.key !== "idle" || idleShown;
          const shown = shownOf(g);
          const hidden = g.items.length - shown.length;
          const list = (
            <ul
              id={`run-group-${g.key}-list`}
              ref={(el) => {
                if (el) lists.current.set(g.key, el);
                else lists.current.delete(g.key);
              }}
              className={`run-list ${drag?.group === g.key ? "is-sorting" : ""}`}
            >
              {shown.map((slot, i) => (
                <RunCard
                  key={slot.id}
                  ref={(el) => {
                    if (el) refs.current.set(slot.id, el);
                    else refs.current.delete(slot.id);
                  }}
                  slot={slot}
                  selected={slot.id === selectedId}
                  onKeyNav={onKeyNav}
                  compact={g.key === "idle"}
                  grab={
                    canMove && g.items.length > 1
                      ? {
                          onPointerDown: (e) => onGrabStart(e, slot.id, g.key, i),
                          onPointerMove: onGrabMove,
                          onPointerUp: onGrabEnd,
                          onPointerCancel: () => setDrag(null),
                        }
                      : undefined
                  }
                  sorting={sortingOf(g.key, i, shown.length, slot.id)}
                />
              ))}
            </ul>
          );
          return (
            <section
              key={g.key}
              id={`run-group-${g.key}`}
              className={`run-group is-${g.key}`}
              aria-labelledby={`run-group-${g.key}-title`}
            >
              <div className="run-group-head">
                <span className="run-group-icon" aria-hidden="true">
                  {g.key === "attention" && <TriangleAlert size={13} strokeWidth={2.4} />}
                  {g.key === "running" && <span className="run-group-dot" />}
                  {g.key === "idle" && <Pause size={11} fill="currentColor" />}
                </span>
                <h3 id={`run-group-${g.key}-title`} className="run-group-name">
                  {GROUP_LABELS[g.key]}
                </h3>
                <span className="run-group-count num">{g.items.length}</span>
                <span className="run-group-line" aria-hidden="true" />
                {g.key === "idle" && !searching && (
                  <button
                    type="button"
                    className="run-group-toggle"
                    aria-expanded={idleOpen}
                    aria-controls="run-group-idle-list"
                    onClick={toggleIdle}
                  >
                    {idleOpen ? "접기" : "펼치기"}
                    <ChevronDown
                      size={13}
                      className={`run-group-chev ${idleOpen ? "is-open" : ""}`}
                      aria-hidden="true"
                    />
                  </button>
                )}
              </div>
              {open &&
                (g.key === "idle" ? (
                  <div className="run-idle-box">
                    {list}
                    {(hidden > 0 || (idleAll && g.items.length > IDLE_PEEK)) && !searching && (
                      <button type="button" className="run-idle-more" onClick={() => setIdleAll((v) => !v)}>
                        {idleAll ? "줄여 보기" : `${g.items.length}개 모두 보기`}
                      </button>
                    )}
                  </div>
                ) : (
                  list
                ))}
            </section>
          );
        })}
      </div>
    </nav>
  );
}
