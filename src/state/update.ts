// 앱 업데이트 표시 규칙 — 내려받기 진행률(머리줄 칩·설정의 진행 막대가 같이 씀).
import type { UpdateState } from "../types";
import { formatBytes } from "./format";

/** 내려받기 진행률(0~100 정수). 크기를 몰라 진행률이 없으면(-1) null. */
export function progressPct(u: Pick<UpdateState, "progress">): number | null {
  return u.progress < 0 ? null : Math.round(u.progress * 100);
}

/** 진행 막대 옆 글 — "42% · 6.1 MB / 14.0 MB", 크기를 모르면 "6.1 MB 받음". */
export function progressText(u: UpdateState): string {
  if (u.total > 0) return `${progressPct(u) ?? 0}% · ${formatBytes(u.downloaded)} / ${formatBytes(u.total)}`;
  return u.downloaded > 0 ? `${formatBytes(u.downloaded)} 받음` : "연결하는 중…";
}
