// 업데이트 — 확인 → 내려받기(진행 막대·크기) → 다시 시작해 설치.
// 내려받기는 매크로가 돌아도 됨. 설치(다시 시작)는 실행 중인 매크로가 없을 때만 — Rust도 거절함.
import {
  ArrowDownToLine,
  CircleCheck,
  LoaderCircle,
  PartyPopper,
  RefreshCw,
  RotateCcw,
  TriangleAlert,
} from "lucide-react";
import { useState, type ReactNode } from "react";
import { ReleaseNotes } from "./ReleaseNotes";
import { SwapLabel } from "../SwapLabel";
import { call, errorText } from "../../ipc";
import { detectPlatform } from "../../platform";
import { useRunnerContext } from "../../state/context";
import { isIdle } from "../../state/runs";
import { progressPct, progressText } from "../../state/update";
import { updateCopy, type UpdateIcon } from "../../state/updateCopy";
import type { UpdateState } from "../../types";

const HEADLINE_ICONS: Record<NonNullable<UpdateIcon>, ReactNode> = {
  updated: <PartyPopper size={14} className="tone-ok" aria-hidden="true" />,
  latest: <CircleCheck size={14} className="tone-ok" aria-hidden="true" />,
  installing: <LoaderCircle size={14} className="spin" aria-hidden="true" />,
  error: <TriangleAlert size={14} className="tone-warn" aria-hidden="true" />,
};

function Steps({ status }: { status: UpdateState["status"] }) {
  // 새 버전을 찾았으면 1단계는 끝났고, 내려받기가 지금 할 일임.
  const step = status === "ready" ? 2 : 1;
  const items = ["새 버전 확인", "내려받기", "다시 시작해 설치"];
  return (
    <ol className="update-steps" aria-label="업데이트 순서">
      {items.map((t, i) => (
        <li key={t} className={i < step ? "is-done" : i === step ? "is-now" : ""}>
          <span className="update-step-num" aria-hidden="true">
            {i < step ? <CircleCheck size={13} /> : i + 1}
          </span>
          {t}
        </li>
      ))}
    </ol>
  );
}

export function UpdatePanel() {
  const { state, act } = useRunnerContext();
  const u = state.update;
  const [restartError, setRestartError] = useState("");
  const running = state.runs.filter((r) => !isIdle(r.phase)).length;
  const checking = u.status === "checking";
  const windows = detectPlatform() === "windows";
  const flow = u.status === "available" || u.status === "downloading" || u.status === "ready";

  async function restart() {
    setRestartError("");
    try {
      await call("update_restart");
    } catch (err) {
      setRestartError(errorText(err));
    }
  }

  const checkButton = (
    <button type="button" className="btn btn-ghost btn-sm" onClick={() => act("update_check")} disabled={checking}>
      <SwapLabel
        active={checking ? 2 : u.status === "error" ? 1 : 0}
        items={[
          <>
            <RefreshCw size={14} aria-hidden="true" />
            업데이트 확인
          </>,
          <>
            <RefreshCw size={14} aria-hidden="true" />
            다시 확인
          </>,
          <>
            <LoaderCircle size={14} className="spin" aria-hidden="true" />
            확인 중…
          </>,
        ]}
      />
    </button>
  );

  // 앱 버전 줄 아래 두 줄 — 상태(무엇이 일어났나)와 덧붙임(다음에 할 일·마지막 확인).
  const copy = updateCopy(u, running, windows);
  const icon = copy.headline.icon && HEADLINE_ICONS[copy.headline.icon];
  const headline = icon ? (
    <>
      {icon} {copy.headline.text}
    </>
  ) : (
    copy.headline.text
  );
  const detail = copy.detail;
  const pct = progressPct(u);

  return (
    <div className={`update-panel is-${u.status}`} aria-live="polite">
      <div className="update-row">
        <span className="about-label">앱 버전</span>
        <div className="update-text">
          <span className="strong num">v{state.version}</span>
          <span className="update-headline">{headline}</span>
          {detail && <span className="muted">{detail}</span>}
        </div>
        {(u.status === "idle" || u.status === "checking" || u.status === "none" || u.status === "error") && checkButton}
        {u.status === "available" && !u.blocked && (
          <button type="button" className="btn btn-primary btn-sm" onClick={() => act("update_install")}>
            <ArrowDownToLine size={14} aria-hidden="true" />
            내려받기
          </button>
        )}
        {u.status === "ready" && (
          <button type="button" className="btn btn-primary btn-sm" onClick={restart} disabled={running > 0}>
            <RotateCcw size={14} aria-hidden="true" />
            다시 시작해 설치
          </button>
        )}
      </div>

      {flow && !u.blocked && <Steps status={u.status} />}

      {u.status === "downloading" && (
        <div className="update-progress">
          <div
            className={`progress progress-lg ${pct === null ? "is-indeterminate" : ""}`}
            role="progressbar"
            aria-label="업데이트 내려받기"
            aria-valuemin={0}
            aria-valuemax={100}
            aria-valuenow={pct ?? undefined}
          >
            <span className="progress-bar" style={pct === null ? undefined : { width: `${pct}%` }} />
          </div>
          <span className="update-progress-text num">{progressText(u)}</span>
        </div>
      )}
      {u.blocked && u.status !== "none" && <p className="note note-warn">{u.blocked}</p>}
      {u.status === "error" && u.error && <p className="note note-warn">{u.error}</p>}
      {restartError && u.status === "ready" && <p className="note note-warn">{restartError}</p>}
      {flow && u.notes && (
        <div className="update-notes">
          <span className="eyebrow">바뀐 점</span>
          <ReleaseNotes body={u.notes} />
        </div>
      )}
    </div>
  );
}
