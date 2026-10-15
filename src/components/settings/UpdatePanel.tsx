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
import { useState } from "react";
import { ReleaseNotes } from "./ReleaseNotes";
import { call, errorText } from "../../ipc";
import { detectPlatform } from "../../platform";
import { useRunnerContext } from "../../state/context";
import { formatWhen } from "../../state/format";
import { iga } from "../../state/particle";
import type { UpdateState } from "../../types";

const ACTIVE = new Set(["checking", "starting", "running", "stopping"]);

export function formatBytes(n: number): string {
  if (n >= 1024 * 1024) return `${(n / (1024 * 1024)).toFixed(1)} MB`;
  return `${Math.max(0, Math.round(n / 1024))} KB`;
}

/** 진행 막대 옆 글 — "42% · 6.1 MB / 14.0 MB", 크기를 모르면 "6.1 MB 받음". */
export function progressText(u: UpdateState): string {
  if (u.total > 0) {
    const pct = Math.round(Math.max(0, u.progress) * 100);
    return `${pct}% · ${formatBytes(u.downloaded)} / ${formatBytes(u.total)}`;
  }
  return u.downloaded > 0 ? `${formatBytes(u.downloaded)} 받음` : "연결하는 중…";
}

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
  const running = state.runs.filter((r) => ACTIVE.has(r.phase)).length;
  const checking = u.status === "checking";
  const checked = u.checkedAt > 0 ? `마지막 확인 ${formatWhen(u.checkedAt)}` : "아직 확인하지 않았어요";
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
      {checking ? (
        <LoaderCircle size={14} className="spin" aria-hidden="true" />
      ) : (
        <RefreshCw size={14} aria-hidden="true" />
      )}
      {checking ? "확인 중…" : u.status === "error" ? "다시 확인" : "업데이트 확인"}
    </button>
  );

  // 앱 버전 줄 아래 두 줄 — 상태(무엇이 일어났나)와 덧붙임(다음에 할 일·마지막 확인).
  const headline =
    u.status === "none" ? (
      u.updatedFrom ? (
        <>
          <PartyPopper size={14} className="tone-ok" aria-hidden="true" /> v{u.updatedFrom}에서 업데이트했어요
        </>
      ) : (
        <>
          <CircleCheck size={14} className="tone-ok" aria-hidden="true" /> 최신 버전이에요
        </>
      )
    ) : u.status === "available" ? (
      `새 버전 ${iga(`v${u.version}`)} 나왔어요`
    ) : u.status === "downloading" ? (
      `v${u.version} 내려받는 중`
    ) : u.status === "ready" ? (
      `v${u.version} 설치 준비가 끝났어요`
    ) : u.status === "installing" ? (
      <>
        <LoaderCircle size={14} className="spin" aria-hidden="true" /> v{u.version} 설치하는 중
      </>
    ) : u.status === "error" ? (
      <>
        <TriangleAlert size={14} className="tone-warn" aria-hidden="true" /> 업데이트 중 문제가 생겼어요
      </>
    ) : u.status === "unsupported" ? (
      "이 빌드는 자동 업데이트를 쓰지 않아요"
    ) : u.status === "checking" ? (
      "새 버전을 확인하는 중…"
    ) : (
      "아직 업데이트를 확인하지 않았어요"
    );

  const detail =
    u.status === "available"
      ? "내려받는 동안에도 매크로는 계속 돌아요."
      : u.status === "downloading"
        ? "다 받으면 서명을 확인해요. 매크로는 계속 돌아요."
        : u.status === "ready"
          ? running > 0
            ? `실행 중인 매크로 ${running}개를 모두 종료하면 설치할 수 있어요. 받은 파일은 그대로 있어요.`
            : windows
              ? "다시 시작하면 설치 창이 잠깐 뜬 뒤 새 버전으로 켜져요."
              : "다시 시작하면 새 버전으로 바뀌어요."
          : u.status === "installing"
            ? windows
              ? "설치 창이 뜨고 끝나면 앱이 다시 켜져요."
              : "곧 앱이 다시 켜져요."
            : u.status === "unsupported"
              ? "개발용 빌드예요."
              : u.status === "none" || u.status === "error"
                ? checked
                : "";

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
            className={`progress progress-lg ${u.progress < 0 ? "is-indeterminate" : ""}`}
            role="progressbar"
            aria-label="업데이트 내려받기"
            aria-valuemin={0}
            aria-valuemax={100}
            aria-valuenow={u.progress < 0 ? undefined : Math.round(u.progress * 100)}
          >
            <span
              className="progress-bar"
              style={u.progress < 0 ? undefined : { width: `${Math.round(u.progress * 100)}%` }}
            />
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
