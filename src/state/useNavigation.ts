// 화면 이동 — 실행 화면 ↔ 설정, 고른 슬롯, 한 열에서 상세가 열렸는지 여부, runner:navigate, 단축키(⌘/Ctrl+, 및 ⌘/Ctrl+O).
// 새 슬롯이 나타나면(웹 [실행기 열기]·매크로 추가) 그 슬롯을 고름. 고른 슬롯이 빠지면 맨 위 슬롯을 고름.
import { useCallback, useEffect, useRef, useState } from "react";
import { on, type NavigatePayload } from "../ipc";
import { EV_NAVIGATE, type AppState } from "../types";
import type { SettingsTarget } from "./runs";

export interface Navigation {
  view: "run" | "settings";
  target: { target: SettingsTarget; seq: number } | null;
  selectedId: string | null;
  detailOpen: boolean;
  openSettings(target?: SettingsTarget): void;
  openRun(): void;
  select(runId: string): void;
  closeDetail(): void;
  /** runner:navigate {to:"library"} 이벤트가 오면 부를 함수를 정함(App이 매크로 추가 창 열기로 채움). */
  setLibraryHandler(open: (() => void) | null): void;
}

export function useNavigation(state: AppState | null, demo = "", openDetail = false): Navigation {
  const [view, setView] = useState<"run" | "settings">(demo === "settings" ? "settings" : "run");
  const [target, setTarget] = useState<Navigation["target"]>(null);
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const [detailOpen, setDetailOpen] = useState(openDetail);
  const known = useRef<Set<string> | null>(null);
  const seq = useRef(0);
  const onLibrary = useRef<(() => void) | null>(null);

  const openSettings = useCallback((t?: SettingsTarget) => {
    setView("settings");
    setTarget(t ? { target: t, seq: ++seq.current } : null);
  }, []);
  const openRun = useCallback(() => {
    setView("run");
    setTarget(null);
  }, []);
  const select = useCallback((runId: string) => {
    setSelectedId(runId);
    setDetailOpen(true);
    setView("run");
  }, []);
  const closeDetail = useCallback(() => setDetailOpen(false), []);

  // 고른 슬롯 맞추기·새로 나타난 슬롯 고르기.
  const runs = state?.runs;
  useEffect(() => {
    if (!runs) return;
    const ids = runs.map((r) => r.id);
    const prev = known.current;
    known.current = new Set(ids);
    const fresh = prev ? ids.filter((id) => !prev.has(id)) : [];
    if (fresh.length > 0) {
      setSelectedId(fresh[fresh.length - 1]);
      return;
    }
    setSelectedId((cur) => (cur && ids.includes(cur) ? cur : (ids[0] ?? null)));
    if (ids.length === 0) setDetailOpen(false);
  }, [runs]);

  const navigate = useCallback(
    (to: NavigatePayload["to"]) => {
      if (to === "settings") openSettings();
      else if (to === "settings:about") openSettings({ kind: "section", section: "about" });
      else if (to === "library") onLibrary.current?.();
      else if (to === "run") openRun();
      else if (to.startsWith("run:")) select(to.slice(4));
    },
    [openSettings, openRun, select],
  );

  useEffect(() => {
    let alive = true;
    let off: (() => void) | undefined;
    on(EV_NAVIGATE, ({ to }) => alive && navigate(to))
      .then((u) => (alive ? (off = u) : u()))
      // 트레이 메뉴에서 매크로로 가기만 못 할 뿐 — 앱은 계속 쓸 수 있음.
      .catch((err) => console.error("navigate listener failed", err));
    return () => {
      alive = false;
      off?.();
    };
  }, [navigate]);

  // Windows에는 메뉴 막대가 없으므로 단축키를 화면에서도 받음(macOS는 메뉴로도 오지만 둘 다 이동일 뿐이라 겹쳐도 됨).
  useEffect(() => {
    function onKey(e: KeyboardEvent) {
      if (!(e.metaKey || e.ctrlKey) || e.altKey || e.shiftKey || isTyping(e.target)) return;
      if (e.key === ",") {
        e.preventDefault();
        navigate("settings");
      } else if (e.key.toLowerCase() === "o") {
        e.preventDefault();
        navigate("library");
      }
    }
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [navigate]);

  const setLibraryHandler = useCallback((open: (() => void) | null) => {
    onLibrary.current = open;
  }, []);
  return { view, target, selectedId, detailOpen, openSettings, openRun, select, closeDetail, setLibraryHandler };
}

function isTyping(target: EventTarget | null): boolean {
  const el = target as HTMLElement | null;
  if (!el || !el.tagName) return false;
  return el.tagName === "INPUT" || el.tagName === "TEXTAREA" || el.isContentEditable;
}
