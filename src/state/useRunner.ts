// Rust 상태 구독. 처음 get_state를 읽고, 이벤트로 오는 스냅샷·활동·알림을 반영함.
// 활동은 슬롯마다 따로 둠 — 슬롯이 처음 보일 때 get_activity로 채우고, 그 뒤로는 runner:activity를 덧붙임.
import { useCallback, useEffect, useRef, useState } from "react";
import { call, on } from "../ipc";
import type { Activity, AppState, Notice, StateEvent } from "../types";
import { EV_ACTIVITY, EV_NOTICE, EV_STATE } from "../types";
import { mergeActivity } from "./activity";

export const MAX_PRICE_POINTS = 180;

/** 슬롯별 현재가 흐름 — 상태 카드의 작은 선 그래프용. 실행(run.startedAt)이 바뀌면 새로 시작함. UI 로컬 값임. */
export type PriceTrail = { startedAt: number; points: number[] };

/** 상태 이벤트가 이만큼 없으면 get_state로 다시 읽음. */
const RESYNC_MS = 15_000;

export interface Runner {
  state: AppState | null;
  activity: Record<string, Activity[]>;
  prices: Record<string, PriceTrail>;
  /** 슬롯이 처음 보일 때 부름. 두 번째부터는 아무것도 하지 않음(실패했으면 다시 부를 수 있음). */
  loadActivity(runId: string): void;
  /** get_activity가 실패한 슬롯 — 빈 기록과 구분해 [다시 불러오기]를 보임. */
  activityFailed: Record<string, boolean>;
  /** 처음 연결에 실패했을 때의 이유. */
  loadError: string;
}

export function useRunner(onNotice: (n: Notice) => void): Runner {
  const [state, setState] = useState<AppState | null>(null);
  const [activity, setActivity] = useState<Record<string, Activity[]>>({});
  const [prices, setPrices] = useState<Record<string, PriceTrail>>({});
  const [activityFailed, setActivityFailed] = useState<Record<string, boolean>>({});
  const [loadError, setLoadError] = useState("");
  const noticeRef = useRef(onNotice);
  const lastEvent = useRef(Date.now());
  noticeRef.current = onNotice;
  const loaded = useRef(new Set<string>());

  // 매크로 폴더 목록은 바뀐 상태 이벤트에만 실림 — 마지막 목록을 들고 있다가 빠진 이벤트에 채움.
  const library = useRef<AppState["library"] | null>(null);
  /** 받은 상태를 씀. 목록을 아직 모르는데 목록이 빠진 이벤트면 쓰지 않고 거짓(get_state를 기다림). */
  const accept = useCallback((incoming: StateEvent): boolean => {
    const lib = incoming.library ?? library.current;
    if (!lib) return false;
    library.current = lib;
    const next: AppState = { ...incoming, library: lib };
    setState(next);
    setPrices((prev) => trackPrices(prev, next));
    // 목록에서 빠진 슬롯의 활동은 버림.
    const ids = new Set(next.runs.map((r) => r.id));
    setActivity((prev) => {
      const stale = Object.keys(prev).filter((id) => !ids.has(id));
      if (stale.length === 0) return prev;
      const copy = { ...prev };
      for (const id of stale) {
        delete copy[id];
        loaded.current.delete(id);
      }
      return copy;
    });
    return true;
  }, []);

  const append = useCallback((runId: string, items: Activity[]) => {
    setActivity((prev) => {
      const before = prev[runId] ?? [];
      const after = mergeActivity(before, items);
      return after === before && prev[runId] ? prev : { ...prev, [runId]: after };
    });
  }, []);

  const loadActivity = useCallback(
    (runId: string) => {
      if (loaded.current.has(runId)) return;
      loaded.current.add(runId);
      call("get_activity", { runId }).then(
        (items) => {
          setActivityFailed((prev) => (prev[runId] ? { ...prev, [runId]: false } : prev));
          append(runId, items);
        },
        () => {
          loaded.current.delete(runId);
          setActivityFailed((prev) => ({ ...prev, [runId]: true }));
        },
      );
    },
    [append],
  );

  useEffect(() => {
    let alive = true;
    const unlisten: Array<() => void> = [];
    (async () => {
      // 첫 get_state 응답이 그사이 온 더 새 상태를 덮지 않게 — 이벤트가 먼저 오면 첫 응답은 버림.
      let pushed = false;
      try {
        const subs = await Promise.all([
          on(EV_STATE, (s) => {
            lastEvent.current = Date.now();
            if (alive && accept(s)) pushed = true;
          }),
          on(EV_ACTIVITY, (a) => alive && append(a.runId, [a])),
          on(EV_NOTICE, (n) => alive && noticeRef.current(n)),
        ]);
        unlisten.push(...subs);
        if (!alive) return;
        const s = await call("get_state");
        if (alive && !pushed) accept(s);
      } catch (err) {
        if (alive) setLoadError(err instanceof Error ? err.message : String(err));
      }
    })();
    // 상태 이벤트가 끊겨도 화면이 옛 값에 멈춰 있지 않게, 한동안 소식이 없으면 직접 다시 읽음.
    // 실행 중에는 5초마다 이벤트가 오므로 보통은 아무것도 하지 않음.
    const resync = setInterval(() => {
      if (!alive || Date.now() - lastEvent.current < RESYNC_MS) return;
      lastEvent.current = Date.now();
      call("get_state").then(
        (s) => alive && accept(s),
        (err) => console.error("state resync failed", err),
      );
    }, RESYNC_MS);
    return () => {
      alive = false;
      clearInterval(resync);
      unlisten.forEach((u) => u());
    };
  }, [accept, append]);

  return { state, activity, prices, loadActivity, activityFailed, loadError };
}

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
    if (!old || old.startedAt !== run.startedAt) {
      out[slot.id] = { startedAt: run.startedAt, points: run.price > 0 ? [run.price] : [] };
      changed = true;
    } else if (!run.finished && run.price > 0 && old.points[old.points.length - 1] !== run.price) {
      const points = old.points.concat(run.price);
      out[slot.id] = {
        startedAt: run.startedAt,
        points: points.length > MAX_PRICE_POINTS ? points.slice(-MAX_PRICE_POINTS) : points,
      };
      changed = true;
    } else {
      out[slot.id] = old;
    }
  }
  if (!changed && Object.keys(prev).length === Object.keys(out).length) return prev;
  return out;
}
