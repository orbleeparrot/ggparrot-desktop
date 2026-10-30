// 앱 기록 구독 — 설정 › 앱 기록이 열려 있는 동안만. runner:app-log를 먼저 듣고 get_app_log로 지난 줄을 채움.
// 둘이 같은 줄을 함께 줄 수 있으니 id로 합침(mergeAppLog). 화면을 닫으면 버리고, 다시 열면 새로 읽음.
import { useCallback, useEffect, useRef, useState } from "react";
import { call, on } from "../ipc";
import { EV_APP_LOG, type AppLogEntry } from "../types";
import { mergeAppLog } from "./appLog";

export interface AppLog {
  /** 오래된 것 → 최근 순서. 최근 500줄. */
  entries: AppLogEntry[];
  /** loading: 처음 읽는 중·ready: 읽음·error: get_app_log 실패(이벤트로 온 줄은 그대로 보임). */
  status: "loading" | "ready" | "error";
  /** 실패했을 때 [다시 불러오기]. */
  reload(): void;
}

export function useAppLog(): AppLog {
  const [entries, setEntries] = useState<AppLogEntry[]>([]);
  const [status, setStatus] = useState<AppLog["status"]>("loading");
  const alive = useRef(true);

  const load = useCallback(() => {
    setStatus("loading");
    call("get_app_log").then(
      (items) => {
        if (!alive.current) return;
        setEntries((prev) => mergeAppLog(prev, items));
        setStatus("ready");
      },
      (err) => {
        console.error("get_app_log failed", err);
        if (alive.current) setStatus("error");
      },
    );
  }, []);

  useEffect(() => {
    alive.current = true;
    let live = true;
    let off: (() => void) | undefined;
    on(EV_APP_LOG, (e) => live && setEntries((prev) => mergeAppLog(prev, [e])))
      .then((u) => (live ? (off = u) : u()))
      // 새 줄을 실시간으로 못 받을 뿐 — 지난 기록은 읽음.
      .catch((err) => console.error("app-log listener failed", err))
      .finally(() => live && load());
    return () => {
      live = false;
      alive.current = false;
      off?.();
    };
  }, [load]);

  return { entries, status, reload: load };
}
