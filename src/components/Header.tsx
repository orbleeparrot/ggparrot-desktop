// 머리줄: 심볼·앱 이름·버전(+ 바로 옆 업데이트 알림)·요약 상태·껄무새 웹·⚙ 설정.
import { Globe, Settings } from "lucide-react";
import { useRunnerContext } from "../state/context";
import { detectPlatform } from "../platform";
import { isTauri } from "../ipc";
import { UpdateChip } from "./UpdateChip";
import { WindowControls } from "./WindowControls";

export const APP_NAME = "껄무새";

/** ⚙은 설정을 여닫는 토글임(설정에서 누르면 실행 화면으로). */
export function Header({ view, onGear }: { view: "run" | "settings"; onGear(): void }) {
  const { state, act, openSettings } = useRunnerContext();
  const { status } = state;
  const live = status.tone === "ok" && state.runs.some((r) => r.phase === "running");
  const showWindowControls = isTauri() && detectPlatform() === "windows";
  return (
    <header className="app-header" data-tauri-drag-region>
      <div className="brand" data-tauri-drag-region>
        <img src="/brand/mark-ggparrot.svg" width={24} height={24} alt="" aria-hidden="true" data-tauri-drag-region />
        <h1 className="app-name" data-tauri-drag-region>
          {APP_NAME}
        </h1>
        <span className="version-group">
          <span className="badge-version num">v{state.version}</span>
          <UpdateChip update={state.update} onOpen={() => openSettings({ kind: "section", section: "about" })} />
        </span>
      </div>
      <div className={`status tone-${status.tone}`} role="status" aria-live="polite">
        <span className={`status-dot ${live ? "is-live" : ""}`} aria-hidden="true" />
        <span className="status-text">{state.claiming ? "웹에서 매크로를 받는 중…" : status.text}</span>
      </div>
      <button
        type="button"
        className="icon-btn header-web"
        aria-label="껄무새 웹 열기"
        title="껄무새 웹 열기 (브라우저)"
        onClick={() => act("open_web")}
      >
        <Globe size={18} />
      </button>
      <button
        type="button"
        className={`icon-btn header-gear ${view === "settings" ? "is-on" : ""}`}
        aria-label="설정"
        aria-pressed={view === "settings"}
        title={detectPlatform() === "mac" ? "설정 (⌘,)" : "설정 (Ctrl+,)"}
        onClick={onGear}
      >
        <Settings size={18} />
      </button>
      {showWindowControls && <WindowControls />}
    </header>
  );
}
