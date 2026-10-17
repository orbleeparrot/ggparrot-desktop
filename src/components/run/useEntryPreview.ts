// 시작 전 화면용 진입 대기 안내(entry_preview)를 미리 물어 둠 — 기다리는 매크로를 대기 중에 열었을 때만.
// - 매크로마다 10분 동안은 다시 묻지 않음(같은 매크로를 오가며 서버를 두드리지 않게).
// - 서버가 답하지 않으면 Rust가 waiting=false를 돌려줌 — 오류 토스트 없이 '평소 기다린 시간'만 빠짐.
import { useEffect, useState } from "react";
import { useRunnerContext } from "../../state/context";
import { isIdle } from "../../state/runs";
import type { EntryPreview, RunSlot } from "../../types";

const FRESH_MS = 10 * 60_000;
const cache = new Map<string, { at: number; preview: EntryPreview | null }>();

export function useEntryPreview(slot: Pick<RunSlot, "id" | "waits" | "phase">): EntryPreview | null {
  const { act } = useRunnerContext();
  const want = slot.waits && isIdle(slot.phase);
  const [got, setGot] = useState(() => ({ id: slot.id, preview: cache.get(slot.id)?.preview ?? null }));

  useEffect(() => {
    if (!want) return;
    const hit = cache.get(slot.id);
    if (hit && Date.now() - hit.at < FRESH_MS) return;
    let alive = true;
    // 묻는 동안 같은 매크로를 다시 열어도 또 묻지 않게 먼저 적어 둠(지난 답은 그대로 둠).
    cache.set(slot.id, { at: Date.now(), preview: hit?.preview ?? null });
    void act("entry_preview", { runId: slot.id }).then((p) => {
      const preview = p?.waiting ? p : null;
      cache.set(slot.id, { at: Date.now(), preview });
      if (alive) setGot({ id: slot.id, preview });
    });
    return () => {
      alive = false;
    };
  }, [want, slot.id, act]);

  if (!want) return null;
  if (got.id === slot.id) return got.preview;
  return cache.get(slot.id)?.preview ?? null;
}
