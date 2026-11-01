// 묶음(포트폴리오)의 한 레그라는 안내 + 묶음 전체 종료 (2026-10-30).
//
// 묶음은 레그마다 **세션 하나·봇 하나·줄 하나**로 돎 — 레그 하나는 지금까지 돌려 온
// 단일 종목 실행과 똑같음. 그래서 목록에서는 따로 보이고, 여기서 "같은 묶음이 더 있다"와
// "한 번에 끝내기"를 말해 줌.
//
// **반만 남는 것이 가장 나쁨.** 한 종목만 끄면 나머지는 계속 돌아 사용자가 생각한 분산이
// 깨짐. 그래서 묶음 종료를 한 자리에 둠.
import { useState } from "react";
import { call } from "../../ipc";
import type { RunSlot, StopMode } from "../../types";

export function BundleBar({ slot }: { slot: RunSlot }) {
  const [busy, setBusy] = useState(false);
  const [done, setDone] = useState("");
  const bundle = slot.bundle;
  // 끝난 레그에는 보이지 않음 — 끌 것이 없음.
  if (!bundle || !slot.run || slot.run.finished) return null;

  const holding = !!slot.run.inPosition;
  // 포지션을 들고 있으면 '청산하고 종료'가 기본 — 묶음에서 한쪽만 들고 남는 것을 막음.
  const mode: StopMode = holding ? "close_and_stop" : "stop_only";

  async function stopAll() {
    setBusy(true);
    try {
      const n = await call("stop_bundle", { bundleKey: bundle!.key, mode });
      setDone(`${n}개에 종료를 걸었어요.`);
    } catch (reason) {
      setDone(reason instanceof Error ? reason.message : String(reason));
    } finally {
      setBusy(false);
    }
  }

  return (
    <div className="bundle-bar">
      <p className="bundle-bar-text">
        여러 종목 묶음의 <b>{bundle.index + 1}번째</b>예요 · 모두 {bundle.size}종목이 따로 돌고 있어요.
      </p>
      <button type="button" className="bundle-bar-stop" disabled={busy} onClick={() => void stopAll()}>
        {busy ? "거는 중…" : holding ? "묶음 전체 청산하고 종료" : "묶음 전체 종료"}
      </button>
      {done && (
        <p className="bundle-bar-done" role="status">
          {done}
        </p>
      )}
    </div>
  );
}
