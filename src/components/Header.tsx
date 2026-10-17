// 머리줄: 심볼·앱 이름(+ 새 버전이 있을 때만 업데이트 알림)·확인할 일이 있을 때만 상태 알약·[웹]·⚙ 설정.
// - 상태 알약은 확인 필요 · 연결 다시 시도 · 웹에서 받는 중일 때만 — '실행 중 N개'는 목록 머리('N개 중 N개 실행 중')와
//   같은 말이라 따로 두지 않음. 확인 필요면 눌러서 목록의 '확인 필요' 묶음으로 감.
// - 껄무새 웹은 지구본(언어 선택처럼 보임) 대신 글자 단추 [↗ 웹].
// 지금 버전은 설정 › 정보에서 봄.
import { ExternalLink, Settings } from "lucide-react";
import { useRunnerContext } from "../state/context";
import { detectPlatform } from "../platform";
import { isTauri } from "../ipc";
import { UpdateChip } from "./UpdateChip";
import { WindowControls } from "./WindowControls";

const APP_NAME = "껄무새";

/** 목록의 '확인 필요' 묶음을 보이게 하고 그 첫 카드에 포커스. */
function revealAttention() {
  const group = document.getElementById("run-group-attention");
  if (!group) return;
  if (typeof group.scrollIntoView === "function") group.scrollIntoView({ block: "nearest" });
  group.querySelector<HTMLElement>(".run-card-select")?.focus({ preventScroll: true });
}

/** ⚙은 설정을 여닫는 토글임(설정에서 누르면 실행 화면으로). */
export function Header({ view, onGear }: { view: "run" | "settings"; onGear(): void }) {
  const { state, act, openSettings } = useRunnerContext();
  const { status } = state;
  const attention = status.tone === "danger" || status.text.includes("확인 필요");
  const show = state.claiming || status.tone === "warn" || status.tone === "danger";
  const showWindowControls = isTauri() && detectPlatform() === "windows";
  const text = state.claiming ? "웹에서 매크로를 받는 중…" : status.text;
  return (
    <header className="app-header" data-tauri-drag-region>
      <div className="brand" data-tauri-drag-region>
        <img src="/brand/mark-ggparrot.svg" width={24} height={24} alt="" aria-hidden="true" data-tauri-drag-region />
        <h1 className="app-name" data-tauri-drag-region>
          {APP_NAME}
        </h1>
        <UpdateChip update={state.update} onOpen={() => openSettings({ kind: "section", section: "about" })} />
      </div>
      <div className="header-status" role="status" aria-live="polite">
        {show &&
          (attention && view === "run" ? (
            <button
              type="button"
              className={`status tone-${status.tone}`}
              title="확인 필요 묶음으로"
              onClick={revealAttention}
            >
              <span className="status-dot" aria-hidden="true" />
              <span className="status-text">{text}</span>
            </button>
          ) : (
            <span className={`status tone-${state.claiming ? "warn" : status.tone}`}>
              <span className="status-dot" aria-hidden="true" />
              <span className="status-text">{text}</span>
            </span>
          ))}
      </div>
      <button
        type="button"
        className="btn btn-quiet btn-sm header-web"
        aria-label="껄무새 웹 열기"
        title="껄무새 웹 열기 (브라우저)"
        onClick={() => act("open_web")}
      >
        <ExternalLink size={14} aria-hidden="true" />웹
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
