// 머리줄의 버전 표시 바로 옆 업데이트 알림 — 새 버전·내려받는 중(진행 막대·%)·설치 준비·설치 중.
// 누르면 설정 › 정보 › 업데이트로 가서 내려받기·설치를 이어 감.
import { LoaderCircle, RotateCcw } from "lucide-react";
import type { UpdateState } from "../types";

export function updateChipLabel(u: UpdateState): string | null {
  switch (u.status) {
    case "available":
      return `v${u.version} 새 버전`;
    case "downloading":
      return u.progress >= 0 ? `내려받는 중 ${Math.round(u.progress * 100)}%` : "내려받는 중";
    case "ready":
      return "설치 준비됨";
    case "installing":
      return "설치하는 중";
    default:
      return null;
  }
}

export function UpdateChip({ update, onOpen }: { update: UpdateState; onOpen(): void }) {
  const label = updateChipLabel(update);
  if (!label) return null;
  const pct = update.status === "downloading" && update.progress >= 0 ? Math.round(update.progress * 100) : null;
  const icon =
    update.status === "downloading" || update.status === "installing" ? (
      <LoaderCircle size={13} className="spin" aria-hidden="true" />
    ) : update.status === "ready" ? (
      <RotateCcw size={13} aria-hidden="true" />
    ) : null;
  return (
    <button
      type="button"
      className={`update-chip is-${update.status}`}
      onClick={onOpen}
      title="설정 › 정보 › 업데이트에서 보기"
      aria-label={`${label} · 설정의 업데이트 열기`}
      style={pct === null ? undefined : ({ "--p": `${pct}%` } as React.CSSProperties)}
    >
      {update.status === "available" && <span className="update-dot" aria-hidden="true" />}
      {icon}
      <span className="update-chip-text">{label}</span>
    </button>
  );
}
