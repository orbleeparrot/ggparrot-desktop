// Windows 전용 창 단추(최소화·최대화·닫기). 시스템 제목 줄을 없앴으므로 머리줄 오른쪽 끝에 직접 그림.
// 닫기는 창 닫기 요청을 보냄 — 설정에 따라 트레이로 숨거나(기본) 실행 중이면 거절됨.
import { Copy, Minus, Square, X } from "lucide-react";
import { useEffect, useState } from "react";
import { isTauri } from "../ipc";

type AppWindow = {
  minimize(): Promise<void>;
  toggleMaximize(): Promise<void>;
  close(): Promise<void>;
  isMaximized(): Promise<boolean>;
  onResized(cb: () => void): Promise<() => void>;
};

async function currentWindow(): Promise<AppWindow | null> {
  if (!isTauri()) return null;
  const mod = await import("@tauri-apps/api/window");
  return mod.getCurrentWindow() as unknown as AppWindow;
}

export function WindowControls() {
  const [maximized, setMaximized] = useState(false);
  useEffect(() => {
    let off: (() => void) | undefined;
    let alive = true;
    void currentWindow().then(async (w) => {
      if (!w || !alive) return;
      setMaximized(await w.isMaximized());
      off = await w.onResized(async () => setMaximized(await w.isMaximized()));
    });
    return () => {
      alive = false;
      off?.();
    };
  }, []);
  const run = (fn: (w: AppWindow) => Promise<void>) => () => void currentWindow().then((w) => w && fn(w));
  return (
    <div className="window-controls" role="group" aria-label="창 조작">
      <button
        type="button"
        className="window-btn"
        aria-label="최소화"
        title="최소화"
        onClick={run((w) => w.minimize())}
      >
        <Minus size={16} strokeWidth={1.6} />
      </button>
      <button
        type="button"
        className="window-btn"
        aria-label={maximized ? "이전 크기로" : "최대화"}
        title={maximized ? "이전 크기로" : "최대화"}
        onClick={run((w) => w.toggleMaximize())}
      >
        {maximized ? <Copy size={13} strokeWidth={1.6} /> : <Square size={13} strokeWidth={1.6} />}
      </button>
      <button
        type="button"
        className="window-btn is-close"
        aria-label="닫기"
        title="닫기"
        onClick={run((w) => w.close())}
      >
        <X size={17} strokeWidth={1.6} />
      </button>
    </div>
  );
}
