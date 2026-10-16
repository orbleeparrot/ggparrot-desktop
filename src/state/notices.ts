// 알림 대기열 — Rust의 error/warn 알림은 대화상자(한 번에 하나), info는 4초 뒤 사라지는 토스트.
// 실패한 명령은 오류 토스트로 보이고, 놓치지 않게 사용자가 닫을 때까지 남음.
import { useCallback, useEffect, useRef, useState } from "react";
import type { Notice } from "../types";

const TOAST_MS = 4000;

export interface Toast {
  id: number;
  tone: "info" | "error";
  title: string;
  message: string;
  /** 이 알림을 남긴 앱 기록 번호 — 있으면 [자세히]로 설정 › 앱 기록의 그 줄을 엶. */
  logId?: number;
}

export interface Notices {
  dialog: Notice | null;
  toasts: Toast[];
  push(n: Notice): void;
  toast(message: string, tone?: Toast["tone"], title?: string, logId?: number): void;
  closeDialog(): void;
  dismissToast(id: number): void;
}

export function useNotices(): Notices {
  const [queue, setQueue] = useState<Notice[]>([]);
  const [toasts, setToasts] = useState<Toast[]>([]);
  const seq = useRef(0);
  const timers = useRef(new Map<number, ReturnType<typeof setTimeout>>());

  const dismissToast = useCallback((id: number) => {
    setToasts((prev) => prev.filter((t) => t.id !== id));
    const t = timers.current.get(id);
    if (t) clearTimeout(t);
    timers.current.delete(id);
  }, []);

  const toast = useCallback(
    (message: string, tone: Toast["tone"] = "info", title = "", logId?: number) => {
      const id = ++seq.current;
      setToasts((prev) => [...prev.slice(-3), { id, tone, title, message, logId }]);
      if (tone !== "error")
        timers.current.set(
          id,
          setTimeout(() => dismissToast(id), TOAST_MS),
        );
    },
    [dismissToast],
  );

  const push = useCallback(
    (n: Notice) => {
      if (n.level === "info") toast(n.message, "info", n.title, n.logId);
      else setQueue((prev) => [...prev, n]);
    },
    [toast],
  );

  const closeDialog = useCallback(() => setQueue((prev) => prev.slice(1)), []);

  useEffect(() => {
    const map = timers.current;
    return () => map.forEach((t) => clearTimeout(t));
  }, []);

  return { dialog: queue[0] ?? null, toasts, push, toast, closeDialog, dismissToast };
}
