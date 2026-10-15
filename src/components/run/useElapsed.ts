// 경과 시간 — 실행 중이면 1초마다, 끝났으면 끝난 시각에서 멈춤.
import { useEffect, useState } from "react";
import type { RunStatus } from "../../types";

export function useElapsed(run: RunStatus | null): number {
  const [now, setNow] = useState(() => Date.now());
  const ticking = !!run && !run.finished;
  useEffect(() => {
    if (!ticking) return;
    setNow(Date.now());
    const t = setInterval(() => setNow(Date.now()), 1000);
    return () => clearInterval(t);
  }, [ticking]);
  if (!run) return 0;
  const end = run.finished ? run.finished.at : now;
  return Math.max(0, end - run.startedAt);
}
