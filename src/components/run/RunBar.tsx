// 실행 중 조작 줄 왼쪽 — 상태를 한 줄로: 모드 배지("실전으로 실행 중") · 저하 상태(있을 때만) · 경과 · 봉 마감 · 서버 연결.
// - 상태 카드 맨 윗줄에 따로 있던 '실행 중 · 경과 · 서버 연결됨'을 여기로 합쳐 같은 상태를 두 번 말하지 않음.
// - 저하 상태 글자는 카드 상태(slot.status)의 뒷부분 그대로 — 목록 카드와 같은 말.
import { Timer, TriangleAlert } from "lucide-react";
import { formatElapsed } from "../../state/format";
import { runningModeText, statusParts } from "../../state/runs";
import { serverCell } from "../../state/runStatus";
import type { RunSlot } from "../../types";
import { closeText } from "./EntryWait";
import { useRunClock } from "./useElapsed";

export function RunBar({ slot }: { slot: RunSlot }) {
  const run = slot.run && !slot.run.finished ? slot.run : null;
  const { now, elapsed } = useRunClock(run);
  const [, rest] = statusParts(slot.status.text);
  const degraded = slot.phase === "running" && slot.status.tone !== "ok" && slot.status.tone !== "muted" ? rest : "";
  const server = run ? serverCell(run, now) : null;
  const wait = run && !run.inPosition ? run.entryWait : null;
  const close = wait ? closeText(wait, now) : "";
  return (
    <div className="run-bar">
      <span
        className={`detail-mode-badge mode-${slot.mode} ${slot.phase === "running" && slot.status.tone === "ok" ? "is-live" : ""}`}
        title="실행 중에는 모드를 바꿀 수 없어요"
      >
        {runningModeText(slot.mode, slot.phase)}
      </span>
      {degraded && (
        <span className={`run-bar-alert tone-${slot.status.tone}`} title={slot.status.text}>
          <TriangleAlert size={13} aria-hidden="true" />
          {degraded}
        </span>
      )}
      {run && (
        <span className="run-bar-meta num" title="경과 시간">
          <Timer size={13} aria-hidden="true" />
          <span className="sr-only">경과 </span>
          {formatElapsed(elapsed)}
        </span>
      )}
      {wait && close && (
        <span className="run-bar-meta num" title={`${wait.interval_label} 봉이 끝날 때 조건을 다시 봐요`}>
          {close}
        </span>
      )}
      {server && (
        <span className={`run-bar-meta server-meta ${server.tone}`} title="서버 연결">
          <span className="server-dot" aria-hidden="true" />
          서버 {server.text}
        </span>
      )}
    </div>
  );
}
