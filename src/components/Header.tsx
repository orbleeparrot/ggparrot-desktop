// 머리줄: 심볼·앱 이름(+ 새 버전이 있을 때만 업데이트 알림)·확인할 일이 있을 때만 상태 알약·[에이전트(web)]·⚙ 설정.
// - 거래소 사이트 바로가기는 설정 › 거래소 연결의 칸마다(매크로를 고를 때마다 머리줄 단추가 바뀌던 것 대신, 10/18).
// - 상태 알약은 확인 필요·연결 다시 시도·웹에서 받는 중일 때만 — 실행 중 개수는 목록의 '실행 중' 묶음 머리가 말함.
//   확인 필요면 눌러서 목록의 '확인 필요' 묶음으로 감.
// - [↗ 에이전트(web)]: 상세에 보이는 실행이 웹 기록을 가지면 웹 내 에이전트의 그 실행 기록, 아니면 내 에이전트.
// 지금 버전은 설정 › 정보에서 봄.
import { ExternalLink, Settings } from "lucide-react";
import { useRunnerContext } from "../state/context";
import { detectPlatform } from "../platform";
import { isTauri } from "../ipc";
import type { RunSlot } from "../types";
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

interface Props {
  view: "run" | "settings";
  /** 지금 상세에 보이는 실행(설정 화면·한 열 목록이면 null). */
  slot: RunSlot | null;
  onGear(): void;
}

/** ⚙은 설정을 여닫는 토글임(설정에서 누르면 실행 화면으로). */
export function Header({ view, slot, onGear }: Props) {
  const { state, act, ok, openSettings } = useRunnerContext();
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
        title={
          slot?.webRunPage
            ? `껄무새 웹 내 에이전트에서 ${slot.macro.symbol} 실행 기록을 열어요`
            : "껄무새 웹 내 에이전트를 열어요"
        }
        onClick={() =>
          slot?.webRunPage ? void ok("open_run_web_page", { runId: slot.id }) : void act("open_web_agents")
        }
      >
        <ExternalLink size={14} aria-hidden="true" />
        에이전트(web)
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
