// '내 매크로' 목록의 선택 모드 — [선택]을 누르면 들어가고, 뺄 수 있는 줄을 눌러 고른 뒤 머리의 [목록에서 빼기]로 한 번에 뺌.
// - 선택 모드에서 줄을 누르면 체크·해제(상세는 바뀌지 않음). Shift+클릭은 마지막으로 누른 줄부터 여기까지(보이는 차례).
// - 고를 수 있는 줄은 대기·종료뿐(Rust remove_runs와 같은 기준) — 실행 중·시작 중인 줄은 흐리게 두고 눌러도 아무 일 없음.
// - 머리의 동그라미는 전체 선택·해제(찾는 중이면 찾은 줄만, 접혀 안 보이는 대기 줄 포함). Ctrl(⌘)+A도 같음.
// - Delete·Backspace는 빼기 확인 창, Esc·[취소]는 나가기. 고른 줄이 다 빠지면(빼기가 끝나면) 선택 모드도 끝남.
// - 선택 모드 밖에서는 아무것도 가로채지 않음 — 줄을 누르면 지금처럼 그 매크로를 보여 줌.
import { useRef, useState, type KeyboardEvent, type MouseEvent } from "react";
import { isIdle } from "../../state/runs";
import type { RunSlot } from "../../types";

/** 보이는 차례에서 a부터 b까지(양 끝 포함, 앞뒤 무관). a가 차례에 없으면 b 하나. b가 없으면 빈 목록. */
export function rangeIds(order: string[], a: string, b: string): string[] {
  const j = order.indexOf(b);
  if (j < 0) return [];
  const i = order.indexOf(a);
  if (i < 0) return [b];
  return order.slice(Math.min(i, j), Math.max(i, j) + 1);
}

const NONE: ReadonlySet<string> = new Set();
/** 목록에서 뺄 수 있는 줄 — 고를 수 있는 줄. */
export const removable = (slot: RunSlot) => isIdle(slot.phase);

interface Options {
  runs: RunSlot[];
  /** 화면에 보이는 차례 — Shift 범위. */
  visible: RunSlot[];
  /** 전체 선택 대상 — 찾는 중이면 찾은 것만(접혀 안 보이는 대기 줄 포함). */
  all: RunSlot[];
  /** 선택 모드에 들어갈 때 접힌·줄여 보인 대기 묶음을 펼침(고를 줄이 다 보이게). */
  revealIdle(): void;
  askRemoveMany(runIds: string[]): void;
}

export function useRunPicking(o: Options) {
  const [selecting, setSelecting] = useState(false);
  const [picked, setPicked] = useState<ReadonlySet<string>>(NONE);
  /** Shift 범위의 시작 — 마지막으로 누른 줄. */
  const anchor = useRef<string | null>(null);
  const removableIds = new Set(o.runs.filter(removable).map((r) => r.id));

  // 목록이 바뀌면 뺄 수 없게 된 줄(실행 시작·목록에서 빠짐)은 고른 것에서 뺌 — 렌더 중 조건부 갱신.
  // 고른 줄이 다 빠졌으면 빼기가 끝난 것 — 선택 모드도 끝냄.
  const [seenRuns, setSeenRuns] = useState(o.runs);
  if (seenRuns !== o.runs) {
    setSeenRuns(o.runs);
    const next = [...picked].filter((id) => removableIds.has(id));
    if (next.length !== picked.size) {
      setPicked(next.length ? new Set(next) : NONE);
      if (!next.length) setSelecting(false);
    }
  }

  const candidates = o.all.filter(removable).map((r) => r.id);
  const allPicked = candidates.length > 0 && candidates.every((id) => picked.has(id));
  /** 고른 줄 — 목록 차례대로(확인 창의 이름 차례). */
  const pickedIds = o.runs.filter((r) => picked.has(r.id)).map((r) => r.id);

  function enter() {
    setSelecting(true);
    o.revealIdle();
  }
  function leave() {
    setSelecting(false);
    setPicked(NONE);
    anchor.current = null;
  }
  function toggleAll() {
    setPicked(allPicked ? NONE : new Set(candidates));
    anchor.current = null;
  }
  function removePicked() {
    if (pickedIds.length) o.askRemoveMany(pickedIds);
  }

  /** 선택 모드에서 줄을 누름 — 상세를 바꾸지 않고 체크·해제. */
  function onClickCapture(e: MouseEvent<HTMLElement>) {
    if (!selecting) return;
    const id = (e.target as HTMLElement).closest<HTMLElement>(".run-card-select")?.dataset.runId;
    if (!id) return;
    e.preventDefault();
    e.stopPropagation();
    if (!removableIds.has(id)) return;
    if (e.shiftKey && anchor.current) {
      const order = o.visible.filter(removable).map((r) => r.id);
      setPicked(new Set([...picked, ...rangeIds(order, anchor.current, id)]));
      return;
    }
    const next = new Set(picked);
    if (next.has(id)) next.delete(id);
    else next.add(id);
    setPicked(next.size ? next : NONE);
    anchor.current = id;
  }

  function onKeyDown(e: KeyboardEvent<HTMLElement>) {
    if (!selecting || (e.target as HTMLElement).closest("input, textarea")) return;
    if ((e.ctrlKey || e.metaKey) && !e.altKey && e.key.toLowerCase() === "a") {
      e.preventDefault();
      setPicked(candidates.length ? new Set(candidates) : NONE);
    } else if (e.key === "Escape") {
      e.preventDefault();
      leave();
    } else if ((e.key === "Delete" || e.key === "Backspace") && pickedIds.length) {
      e.preventDefault();
      removePicked();
    }
  }

  return {
    selecting,
    picked,
    count: pickedIds.length,
    allPicked,
    /** 고를 수 있는 줄이 하나라도 있는지 — 없으면 [선택]을 두지 않음. */
    canSelect: removableIds.size > 0,
    isPickable: (slot: RunSlot) => selecting && removable(slot),
    enter,
    leave,
    toggleAll,
    removePicked,
    onKeyDown,
    onClickCapture,
  };
}
