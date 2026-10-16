// 앱 기록 다루기 — id로 합치기·최근 500줄 유지·필터(전체·주의·문제)·수준과 출처의 한국어 이름.
import type { AppLogEntry, AppLogLevel, AppLogSource } from "../types";
import { mergeById } from "./activity";

/** 화면이 들고 있는 앱 기록 줄 수 한도(Rust APP_LOG_LIMIT과 같음). */
export const MAX_APP_LOG = 500;

/** get_app_log 응답과 runner:app-log 이벤트를 합침 — 같은 id는 한 번만, 오래된 것 → 최근 순서. */
export function mergeAppLog(prev: AppLogEntry[], incoming: AppLogEntry[]): AppLogEntry[] {
  return mergeById(prev, incoming, MAX_APP_LOG);
}

export type AppLogFilter = "all" | "warn" | "error";

export const APP_LOG_FILTERS: { id: AppLogFilter; label: string }[] = [
  { id: "all", label: "전체" },
  { id: "warn", label: "주의" },
  { id: "error", label: "문제" },
];

export const APP_LOG_LEVEL_LABELS: Record<AppLogLevel, string> = { info: "정보", warn: "주의", error: "문제" };

export const APP_LOG_SOURCE_LABELS: Record<AppLogSource, string> = {
  app: "앱",
  web: "웹 연결",
  update: "업데이트",
  keys: "키",
  storage: "저장소",
  library: "매크로 폴더",
  server: "서버",
  run: "실행",
  wizard: "연결 도우미",
};

export function inAppLogFilter(level: AppLogLevel, filter: AppLogFilter): boolean {
  return filter === "all" || level === filter;
}

/** 화면에 보일 줄 — 최근 것이 위(가끔 들여다보는 기록이라 새 일이 먼저 보이게). */
export function visibleAppLog(entries: AppLogEntry[], filter: AppLogFilter): AppLogEntry[] {
  const out: AppLogEntry[] = [];
  for (let i = entries.length - 1; i >= 0; i -= 1) if (inAppLogFilter(entries[i].level, filter)) out.push(entries[i]);
  return out;
}

/** 필터 칩의 숫자. */
export function countAppLog(entries: AppLogEntry[]): Record<AppLogFilter, number> {
  const out: Record<AppLogFilter, number> = { all: entries.length, warn: 0, error: 0 };
  for (const e of entries) if (e.level !== "info") out[e.level] += 1;
  return out;
}
