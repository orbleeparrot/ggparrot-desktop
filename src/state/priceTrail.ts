// 상태 카드 현재가 그래프의 점 모음 — 실행 시작부터 지금까지를 한 그래프에 담음(UI 로컬 값).
// - 값이 바뀔 때만 점을 더함. 점마다 받은 시각을 같이 둬 그래프 가로축을 시간에 맞춤.
// - 점이 MAX_TRAIL_POINTS를 넘으면 오래된 부분을 시간 구간마다 최저·최고 두 점으로 줄임(모양 유지).
//   첫 점(시작 시각)과 최근 점들은 그대로 둬 덮는 시간과 지금의 움직임이 줄지 않음.
// - 웹뷰 메모리라 앱을 다시 열면 비어서 시작함 — 그래서 첫 점 시각과 실행 시작 시각을 비교해 이름을 달리 붙임.
import type { AppState } from "../types";
import { formatDuration } from "./format";

/** 슬롯 하나의 점 모음. 실행(run.startedAt)이 바뀌면 새로 시작함. */
export type PriceTrail = { startedAt: number; points: number[]; times: number[] };

/** 점 수 상한. 넘으면 줄여 대략 3/4로 만듦. */
export const MAX_TRAIL_POINTS = 240;
/** 첫 점이 실행 시작에서 이만큼 안쪽이면 '실행 시작부터'로 봄(첫 상태가 오기까지 몇 초 걸림). */
export const FROM_START_SLACK_MS = 60_000;

/**
 * 점이 cap을 넘으면 줄임 — 첫 점·최근 cap/4개는 그대로, 그 사이는 같은 시간 폭의 구간으로 나눠
 * 구간마다 최저·최고(시간 순서대로) 두 점만 남김. 시각은 늘 오름차순을 지킴.
 */
export function downsampleTrail(points: number[], times: number[], cap = MAX_TRAIL_POINTS) {
  const n = points.length;
  if (n <= cap) return { points, times };
  const recent = Math.floor(cap / 4);
  const midFrom = 1;
  const midTo = n - recent; // 이 앞까지가 줄일 부분
  const buckets = Math.max(1, Math.floor((cap / 2 - 1) / 2));
  const t0 = times[midFrom];
  const span = times[midTo - 1] - t0;
  const keep: number[] = [0];
  let start = midFrom;
  for (let b = 0; b < buckets && start < midTo; b++) {
    // 구간 끝 — 시간 폭이 0이면 개수로 나눔.
    const edge = span > 0 ? t0 + (span * (b + 1)) / buckets : Number.NaN;
    let end = start;
    if (b === buckets - 1) end = midTo;
    else if (span > 0) while (end < midTo && times[end] <= edge) end++;
    else end = Math.min(midTo, start + Math.ceil((midTo - midFrom) / buckets));
    if (end === start) continue;
    let lo = start;
    let hi = start;
    for (let i = start; i < end; i++) {
      if (points[i] < points[lo]) lo = i;
      if (points[i] > points[hi]) hi = i;
    }
    if (lo === hi) keep.push(lo);
    else keep.push(Math.min(lo, hi), Math.max(lo, hi));
    start = end;
  }
  for (let i = midTo; i < n; i++) keep.push(i);
  return { points: keep.map((i) => points[i]), times: keep.map((i) => times[i]) };
}

/** 점 하나를 더함 — 값이 같으면 그대로 돌려줌. */
export function appendPrice(trail: PriceTrail, price: number, at: number): PriceTrail {
  if (!(price > 0) || trail.points[trail.points.length - 1] === price) return trail;
  const next = downsampleTrail(trail.points.concat(price), trail.times.concat(at));
  return { startedAt: trail.startedAt, ...next };
}

/** 새 상태 스냅샷을 받아 슬롯별 점 모음을 갱신함. 바뀐 게 없으면 prev를 그대로 돌려줌. */
export function trackPrices(prev: Record<string, PriceTrail>, next: AppState): Record<string, PriceTrail> {
  let changed = false;
  const out: Record<string, PriceTrail> = {};
  for (const slot of next.runs) {
    const run = slot.run;
    const old = prev[slot.id];
    if (!run) {
      if (old) changed = true;
      continue;
    }
    const at = run.updatedAt > 0 ? run.updatedAt : run.startedAt;
    if (!old || old.startedAt !== run.startedAt) {
      const first = run.price > 0;
      out[slot.id] = { startedAt: run.startedAt, points: first ? [run.price] : [], times: first ? [at] : [] };
      changed = true;
    } else if (!run.finished) {
      const after = appendPrice(old, run.price, at);
      if (after !== old) changed = true;
      out[slot.id] = after;
    } else {
      out[slot.id] = old;
    }
  }
  if (!changed && Object.keys(prev).length === Object.keys(out).length) return prev;
  return out;
}

/**
 * 그래프 아래 덮는 시간 — "실행 시작부터 · 1시간 20분". 앱을 실행 뒤에 열어 앞부분이 없으면 "앱을 연 뒤부터 · 12분".
 * - firstAt: 첫 점 시각, lastAt: 마지막 상태 시각. 1초 안쪽이거나 모르면 "".
 */
export function trailSpanLabel(startedAt: number, firstAt: number, lastAt: number): string {
  if (!(firstAt > 0) || !(lastAt > firstAt)) return "";
  const fromStart = startedAt > 0 && firstAt - startedAt <= FROM_START_SLACK_MS;
  const since = fromStart ? Math.min(startedAt, firstAt) : firstAt;
  const span = formatDuration(lastAt - since);
  if (!span) return "";
  return `${fromStart ? "실행 시작부터" : "앱을 연 뒤부터"} · ${span}`;
}
