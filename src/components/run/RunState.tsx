// 상세 머리 제목 옆 상태 — "● 실행 중 06:02". 모드는 제목 옆 배지(실전 · 테스트넷)가, 끝내는 단추는 머리 오른쪽이 맡음.
// - 서버가 잘 붙어 있으면 말하지 않음. 끊기거나 거절되면 경과 대신 그 자리에 "서버 다시 연결하는 중 · 3분째"(주의 색).
// - 저하 상태(잔고 부족 등)는 카드 상태(slot.status)의 뒷부분 그대로 — 목록 카드와 같은 말.
import { formatElapsed } from "../../state/format";
import { runStateText, statusParts } from "../../state/runs";
import { serverCell } from "../../state/runStatus";
import type { RunSlot } from "../../types";
import { useRunClock } from "./useElapsed";

export function RunState({ slot }: { slot: RunSlot }) {
  const run = slot.run && !slot.run.finished ? slot.run : null;
  const { now, elapsed } = useRunClock(run);
  const running = slot.phase === "running";
  const [, rest] = statusParts(slot.status.text);
  const degraded = running && slot.status.tone !== "ok" && slot.status.tone !== "muted" ? rest : "";
  const server = run ? serverCell(run, now) : null;
  const serverDown = server && (server.tone === "tone-warn" || server.tone === "tone-down") ? server : null;
  // 서버 쪽 저하("서버 재시도 중")는 서버 글자가 더 자세히 말함 — 겹치면 하나만.
  const issues = [
    serverDown ? `서버 ${serverDown.text}` : "",
    degraded && !(serverDown && degraded.startsWith("서버")) ? degraded : "",
  ].filter(Boolean);
  const tone =
    serverDown?.tone === "tone-down" || (issues.length && slot.status.tone === "danger")
      ? "danger"
      : issues.length
        ? "warn"
        : running
          ? "ok"
          : "muted";
  return (
    <span className={`run-state tone-${tone}`} title={issues.length ? slot.status.text : undefined}>
      <span className={`run-state-dot ${tone === "ok" ? "is-live" : ""}`} aria-hidden="true" />
      {issues.length ? issues.join(" · ") : runStateText(slot.phase)}
      {!issues.length && running && run && (
        <span className="run-state-elapsed num">
          <span className="sr-only">경과 </span>
          {formatElapsed(elapsed)}
        </span>
      )}
    </span>
  );
}
