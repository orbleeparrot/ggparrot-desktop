// 화면 테마 — 시스템(OS 설정을 따라감) · 라이트 · 다크. 고른 값은 이 PC(웹뷰 저장소)에 기억.
// - 실제 색은 <html data-theme="light|dark">로 정함(theme.css). '시스템'이면 OS 설정을 읽어 넣고, OS가 바뀌면 따라 바꿈.
// - 앱 보안 정책(CSP)이 인라인 스크립트를 막아 index.html에서 미리 못 넣음 — main.tsx가 첫 렌더 전에 initTheme()을 부름.
import { useEffect, useState } from "react";

export type ThemePref = "system" | "light" | "dark";
export const THEME_KEY = "ggp.runner.theme";
export const THEME_LABELS: Record<ThemePref, string> = { system: "시스템", light: "라이트", dark: "다크" };

const darkQuery = () =>
  typeof window !== "undefined" && window.matchMedia ? window.matchMedia("(prefers-color-scheme: dark)") : null;

export function readThemePref(): ThemePref {
  try {
    const v = window.localStorage.getItem(THEME_KEY);
    return v === "light" || v === "dark" ? v : "system";
  } catch {
    return "system";
  }
}

/** 고른 값 → 실제로 그릴 테마. */
export function resolveTheme(pref: ThemePref, systemDark: boolean): "light" | "dark" {
  return pref === "system" ? (systemDark ? "dark" : "light") : pref;
}

function apply(pref: ThemePref) {
  document.documentElement.dataset.theme = resolveTheme(pref, !!darkQuery()?.matches);
}

/** 첫 렌더 전에 한 번 — 저장된 값을 넣고, '시스템'이면 OS 설정이 바뀔 때 따라감. */
export function initTheme() {
  apply(readThemePref());
  darkQuery()?.addEventListener?.("change", () => {
    if (readThemePref() === "system") apply("system");
  });
}

/** 설정 화면용 — 고르면 바로 바뀌고 저장됨. */
export function useThemePref(): [ThemePref, (pref: ThemePref) => void] {
  const [pref, setPref] = useState<ThemePref>(readThemePref);
  useEffect(() => apply(pref), [pref]);
  const choose = (next: ThemePref) => {
    try {
      window.localStorage.setItem(THEME_KEY, next);
    } catch {
      // 저장소를 못 쓰면 이번 실행 동안만 바뀜.
    }
    setPref(next);
  };
  return [pref, choose];
}
