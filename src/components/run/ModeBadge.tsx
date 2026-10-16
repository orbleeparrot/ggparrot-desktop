import { MODE_LABELS } from "../../state/runs";
import type { RunMode } from "../../types";

/**
 * 모의·테스트넷·실전 배지. 실전은 초록(#15803D).
 * - 자리가 모자라면 색 점으로 줄어듦(run.css .run-card-mode) — 글자 배지는 둘째 줄로 밀려 가려지고 점만 남음.
 * - 줄어도 모드 이름은 title(마우스)과 가려진 글자(화면 읽기 프로그램)로 남음.
 */
export function ModeBadge({ mode }: { mode: RunMode }) {
  const label = MODE_LABELS[mode];
  return (
    <span className={`run-card-mode mode-${mode}`} title={label}>
      <span className="mode-dot" aria-hidden="true" />
      <span className={`mode-badge mode-${mode}`}>{label}</span>
    </span>
  );
}
