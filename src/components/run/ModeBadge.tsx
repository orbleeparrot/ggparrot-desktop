import { MODE_LABELS } from "../../state/runs";
import type { RunMode } from "../../types";

/** 모의·테스트넷·실전 배지. 실전은 초록(#15803D). */
export function ModeBadge({ mode }: { mode: RunMode }) {
  return <span className={`mode-badge mode-${mode}`}>{MODE_LABELS[mode]}</span>;
}
