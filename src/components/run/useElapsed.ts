// 경과 시간 — 실행 중이면 1초마다, 끝났으면 끝난 시각에서 멈춤.
import { useEffect, useState } from "react";
import type { RunStatus } from "../../types";

/** 실행 중이면 1초마다 바뀌는 지금 시각과 경과 시간. 끝났으면 경과는 끝난 시각 기준. */
export function useRunClock(run: RunStatus | null): { now: number; elapsed: number } {
  const [now, setNow] = useState(() => Date.now());
  const ticking = !!run && !run.finished;
  useEffect(() => {
    if (!ticking) return;
    const tick = () => setNow(Date.now());
    // 실행이 막 시작됐으면 첫 1초를 기다리지 않고 바로 맞춤(effect 안에서 바로 바꾸지 않고 다음 차례에).
    const first = setTimeout(tick, 0);
    const t = setInterval(tick, 1000);
    return () => {
      clearTimeout(first);
      clearInterval(t);
    };
  }, [ticking]);
  if (!run) return { now, elapsed: 0 };
  const end = run.finished ? run.finished.at : now;
  return { now, elapsed: Math.max(0, end - run.startedAt) };
}

export function useElapsed(run: RunStatus | null): number {
  return useRunClock(run).elapsed;
}
