// 포커스 가두기·움직임 줄이기 — 대화상자·시트·연결 도우미가 같이 씀.
import { useEffect, useRef, useState, type KeyboardEvent, type RefObject } from "react";

export const FOCUSABLE =
  'button:not([disabled]), [href], input:not([disabled]), select:not([disabled]), textarea:not([disabled]), [tabindex]:not([tabindex="-1"])';

/** Tab·Shift+Tab이 root 밖으로 나가지 않게 함. */
export function trapTab(e: KeyboardEvent<HTMLElement>, root: HTMLElement | null) {
  if (e.key !== "Tab" || !root) return;
  const items = Array.from(root.querySelectorAll<HTMLElement>(FOCUSABLE));
  if (items.length === 0) {
    e.preventDefault();
    return;
  }
  const first = items[0];
  const last = items[items.length - 1];
  const active = document.activeElement;
  if (e.shiftKey && (active === first || active === root)) {
    e.preventDefault();
    last.focus();
  } else if (!e.shiftKey && active === last) {
    e.preventDefault();
    first.focus();
  }
}

/** 열릴 때 [data-autofocus](없으면 첫 칸)에 포커스를 두고, 닫히면 이전 포커스로 돌려줌. */
export function useModalFocus(panel: RefObject<HTMLElement | null>) {
  useEffect(() => {
    const previous = document.activeElement as HTMLElement | null;
    const root = panel.current;
    const target = root?.querySelector<HTMLElement>("[data-autofocus]") ?? root?.querySelector<HTMLElement>(FOCUSABLE);
    (target ?? root)?.focus();
    return () => {
      if (previous && document.contains(previous)) previous.focus?.();
    };
  }, [panel]);
}

export function reducedMotion(): boolean {
  return typeof window !== "undefined" && !!window.matchMedia?.("(prefers-reduced-motion: reduce)").matches;
}

/** 창 너비 등 미디어 쿼리. matchMedia가 없으면(jsdom) fallback. */
export function useMediaQuery(query: string, fallback = true): boolean {
  const get = () => (typeof window !== "undefined" && window.matchMedia ? window.matchMedia(query).matches : fallback);
  const [matches, setMatches] = useState(get);
  const queryRef = useRef(query);
  queryRef.current = query;
  useEffect(() => {
    if (typeof window === "undefined" || !window.matchMedia) return;
    const mq = window.matchMedia(query);
    const onChange = () => setMatches(mq.matches);
    onChange();
    mq.addEventListener?.("change", onChange);
    return () => mq.removeEventListener?.("change", onChange);
  }, [query]);
  return matches;
}
