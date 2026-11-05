// '내 매크로' 목록에서 여러 개 고르기 — 줄 위에서 눌러 끌면 끈 사각형에 닿은 줄이 골라짐.
// - 골라지는 줄은 목록에서 뺄 수 있는 것(대기·종료)뿐 — 실행 중 카드는 지나가도 안 골라짐(Rust remove_runs와 같은 기준).
// - 한 줄 목록에서는 위아래로 끈 범위, 좁은 창의 두 칸 격자에서는 끈 모양 그대로의 덩어리.
// - 그냥 누르면 지금처럼 그 매크로를 보여 주고 고른 것을 비움. 빈 곳을 눌러도 비움.
// - Shift+클릭: 마지막으로 고른 줄부터 여기까지(보이는 차례). Ctrl(⌘)+클릭: 한 줄 넣기·빼기.
// - Ctrl(⌘)+A: 뺄 수 있는 줄 전부(접힌 대기 묶음은 펼침). Delete·Backspace: 빼기 확인 창. Esc: 비움.
// - 순서 손잡이·[▶]·[×]·접기 단추를 누른 것은 고르기가 아님.
// - 끄는 동안 위·아래 끝에 가까우면 저절로 스크롤(순서 바꾸기와 같은 속도) — 누른 자리는 스크롤한 만큼 옮겨 셈.
import {
  useEffect,
  useRef,
  useState,
  type KeyboardEvent,
  type MouseEvent,
  type PointerEvent,
  type RefObject,
} from "react";
import { isIdle } from "../../state/runs";
import type { RunSlot } from "../../types";
import { edgeScrollSpeed, scrollBy, scrollTopOf, visibleBand } from "./listScroll";

/** 이만큼(px) 움직여야 끌어 고르기가 시작됨 — 그 전에 손을 떼면 그냥 누름. */
export const PICK_SLOP = 4;

/** 화면 좌표 사각형(DOMRect와 같은 칸). */
export interface Area {
  left: number;
  top: number;
  right: number;
  bottom: number;
}

interface Point {
  x: number;
  y: number;
}

/** 누른 자리와 지금 포인터로 만든 사각형 — 그사이 스크롤한 만큼(scrolled) 누른 자리를 위로 옮김. */
export function pickArea(from: Point, scrolled: number, to: Point): Area {
  const y0 = from.y - scrolled;
  return {
    left: Math.min(from.x, to.x),
    right: Math.max(from.x, to.x),
    top: Math.min(y0, to.y),
    bottom: Math.max(y0, to.y),
  };
}

/** 사각형에 닿은 줄 — 가장자리만 닿아도 셈. */
export function hitIds(boxes: { id: string; rect: Area }[], area: Area): string[] {
  return boxes
    .filter(
      ({ rect }) =>
        rect.left <= area.right && rect.right >= area.left && rect.top <= area.bottom && rect.bottom >= area.top,
    )
    .map((b) => b.id);
}

/** 보이는 차례에서 a부터 b까지(양 끝 포함, 앞뒤 무관). a가 차례에 없으면 b 하나. b가 없으면 빈 목록. */
export function rangeIds(order: string[], a: string, b: string): string[] {
  const j = order.indexOf(b);
  if (j < 0) return [];
  const i = order.indexOf(a);
  if (i < 0) return [b];
  return order.slice(Math.min(i, j), Math.max(i, j) + 1);
}

const NONE: ReadonlySet<string> = new Set();
const removable = (slot: RunSlot) => isIdle(slot.phase);

function union(base: ReadonlySet<string>, more: string[]): ReadonlySet<string> {
  if (!more.length) return base;
  return new Set([...base, ...more]);
}

function sameSet(a: ReadonlySet<string>, b: ReadonlySet<string>): boolean {
  return a.size === b.size && [...a].every((id) => b.has(id));
}

/** 이 요소를 누른 것은 고르기가 아님 — 순서 손잡이·주 단추·×·찾기 칸. */
const NOT_PICKING = ".run-card-grip, .run-card-action, .run-card-remove-slot, input, a";

interface Press {
  pointerId: number;
  from: Point;
  /** 누를 때 스크롤한 양. */
  scroll: number;
  /** Ctrl(⌘)을 누른 채 끌면 지금 고른 것에 더함. */
  base: ReadonlySet<string>;
  /** 누른 줄 — 빈 곳이면 null. */
  runId: string | null;
  started: boolean;
}

interface Options {
  runs: RunSlot[];
  /** 화면에 보이는 차례 — Shift 범위. */
  visible: RunSlot[];
  /** Ctrl+A 대상 — 찾는 중이면 찾은 것만(접혀 안 보이는 대기 줄 포함). */
  all: RunSlot[];
  selectedId: string | null;
  panel: RefObject<HTMLElement | null>;
  head: RefObject<HTMLDivElement | null>;
  body: RefObject<HTMLDivElement | null>;
  /** 접힌·줄여 보인 대기 묶음을 펼침(Ctrl+A로 다 골랐을 때 보이게). */
  revealIdle(): void;
  askRemoveMany(runIds: string[]): void;
}

export function useRunPicking(o: Options) {
  const [picked, setPicked] = useState<ReadonlySet<string>>(NONE);
  const [picking, setPicking] = useState(false);
  const press = useRef<Press | null>(null);
  const pointer = useRef<Point>({ x: 0, y: 0 });
  /** Shift 범위의 시작 — 마지막으로 고르기 시작한 줄. */
  const anchor = useRef<string | null>(null);
  /** 끌어 고른 뒤 따라오는 click은 '그냥 누름'이 아님. */
  const skipClick = useRef(false);
  const removableIds = useRef<Set<string>>(new Set());

  // 목록이 바뀌면 뺄 수 없게 된 줄(실행 시작·목록에서 빠짐)은 고른 것에서 뺌 — 렌더 중 조건부 갱신.
  const [seenRuns, setSeenRuns] = useState(o.runs);
  if (seenRuns !== o.runs) {
    setSeenRuns(o.runs);
    const ok = new Set(o.runs.filter(removable).map((r) => r.id));
    const next = [...picked].filter((id) => ok.has(id));
    if (next.length !== picked.size) setPicked(next.length ? new Set(next) : NONE);
  }
  useEffect(() => {
    removableIds.current = new Set(o.runs.filter(removable).map((r) => r.id));
  }, [o.runs]);

  function clear() {
    setPicked(NONE);
    anchor.current = null;
  }

  /** 끄는 중 — 지금 사각형에 닿은 뺄 수 있는 줄을 고름(Ctrl을 누르고 시작했으면 원래 고른 것에 더함). */
  function sweep() {
    const p = press.current;
    const list = o.body.current;
    if (!p?.started || !list) return;
    const area = pickArea(p.from, scrollTopOf(o.panel.current) - p.scroll, pointer.current);
    const boxes = Array.from(list.querySelectorAll<HTMLElement>(".run-card-select[data-run-id]"))
      .filter((el) => removableIds.current.has(el.dataset.runId ?? ""))
      .map((el) => ({ id: el.dataset.runId ?? "", rect: (el.closest(".run-card") ?? el).getBoundingClientRect() }));
    const next = union(p.base, hitIds(boxes, area));
    setPicked((prev) => (sameSet(prev, next) ? prev : next));
  }
  // 자동 스크롤(effect)이 부를 최신 sweep.
  const sweepRef = useRef(sweep);
  useEffect(() => {
    sweepRef.current = sweep;
  });

  // 끄는 동안 포인터가 위·아래 끝 가까이 있으면 프레임마다 조금씩 스크롤하고 다시 고름.
  const { head, panel } = o;
  useEffect(() => {
    if (!picking) return;
    let frame = 0;
    function tick() {
      const [top, bottom] = visibleBand(head.current, panel.current);
      const dy = edgeScrollSpeed(pointer.current.y, top, bottom);
      if (dy !== 0) {
        scrollBy(panel.current, dy);
        sweepRef.current();
      }
      frame = requestAnimationFrame(tick);
    }
    frame = requestAnimationFrame(tick);
    return () => cancelAnimationFrame(frame);
  }, [picking, head, panel]);

  function onPointerDown(e: PointerEvent<HTMLElement>) {
    skipClick.current = false;
    // 손가락(터치)으로 끌면 스크롤 — 고르기는 마우스·펜만.
    if (e.button !== 0 || e.pointerType === "touch") return;
    const target = e.target as HTMLElement;
    if (target.closest(NOT_PICKING)) return;
    const row = target.closest<HTMLElement>(".run-card-select");
    if (!row && target.closest("button")) return;
    press.current = {
      pointerId: e.pointerId,
      from: { x: e.clientX, y: e.clientY },
      scroll: scrollTopOf(o.panel.current),
      base: e.ctrlKey || e.metaKey ? picked : NONE,
      runId: row?.dataset.runId ?? null,
      started: false,
    };
    pointer.current = { x: e.clientX, y: e.clientY };
  }

  function onPointerMove(e: PointerEvent<HTMLElement>) {
    const p = press.current;
    if (!p || p.pointerId !== e.pointerId) return;
    pointer.current = { x: e.clientX, y: e.clientY };
    if (!p.started) {
      if (Math.hypot(e.clientX - p.from.x, e.clientY - p.from.y) < PICK_SLOP) return;
      p.started = true;
      if (p.runId && removableIds.current.has(p.runId)) anchor.current = p.runId;
      e.currentTarget.setPointerCapture(e.pointerId);
      window.getSelection()?.removeAllRanges();
      setPicking(true);
    }
    sweep();
  }

  function onPointerUp(e: PointerEvent<HTMLElement>) {
    const p = press.current;
    if (!p || p.pointerId !== e.pointerId) return;
    press.current = null;
    if (p.started) {
      skipClick.current = true;
      setPicking(false);
    } else if (!p.runId && !(e.ctrlKey || e.metaKey)) {
      clear();
    }
  }

  function onPointerCancel() {
    press.current = null;
    setPicking(false);
  }

  function onClickCapture(e: MouseEvent<HTMLElement>) {
    if (skipClick.current) {
      skipClick.current = false;
      e.preventDefault();
      e.stopPropagation();
      return;
    }
    const id = (e.target as HTMLElement).closest<HTMLElement>(".run-card-select")?.dataset.runId;
    if (!id) return;
    const toggle = e.ctrlKey || e.metaKey;
    if (!toggle && !e.shiftKey) {
      // 그냥 누름 — 고른 것을 비우고 지금처럼 그 매크로를 보여 줌.
      if (picked.size) clear();
      return;
    }
    // 실행 중은 고를 수 없음 — 지금처럼 그 매크로를 보여 줌.
    if (!removableIds.current.has(id)) return;
    e.preventDefault();
    e.stopPropagation();
    if (e.shiftKey) {
      const order = o.visible.filter(removable).map((r) => r.id);
      const sel = o.selectedId && removableIds.current.has(o.selectedId) ? o.selectedId : id;
      const from = anchor.current ?? sel;
      anchor.current = from;
      setPicked(union(toggle ? picked : NONE, rangeIds(order, from, id)));
      return;
    }
    const next = new Set(picked);
    if (next.has(id)) next.delete(id);
    else next.add(id);
    setPicked(next.size ? next : NONE);
    anchor.current = id;
  }

  /** 고른 줄 — 목록 차례대로(확인 창의 이름 차례). */
  const pickedIds = o.runs.filter((r) => picked.has(r.id)).map((r) => r.id);
  function removePicked() {
    if (pickedIds.length) o.askRemoveMany(pickedIds);
  }

  function onKeyDown(e: KeyboardEvent<HTMLElement>) {
    if ((e.target as HTMLElement).closest("input, textarea")) return;
    if ((e.ctrlKey || e.metaKey) && !e.altKey && e.key.toLowerCase() === "a") {
      e.preventDefault();
      const ids = o.all.filter(removable).map((r) => r.id);
      if (!ids.length) return;
      o.revealIdle();
      setPicked(new Set(ids));
      anchor.current = ids[0];
      return;
    }
    if (!picked.size) return;
    if (e.key === "Escape") {
      e.preventDefault();
      clear();
    } else if (e.key === "Delete" || e.key === "Backspace") {
      e.preventDefault();
      removePicked();
    }
  }

  return {
    picked,
    count: pickedIds.length,
    picking,
    clear,
    removePicked,
    onKeyDown,
    handlers: { onPointerDown, onPointerMove, onPointerUp, onPointerCancel, onClickCapture },
  };
}
