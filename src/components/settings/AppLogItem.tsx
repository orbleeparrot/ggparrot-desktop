// 앱 기록 한 줄 — 시각·수준 아이콘·출처 꼬리표·굵은 제목(끝에 작은 [›] 펼치기)·펼친 설명.
// - 시각·펼치기 단추·아이콘 모양은 활동 기록(ActivityItem)과 같음. 펼침 여부는 부모가 들고 있음([자세히]로 열 때 펼쳐 두려고).
import { ChevronRight, CircleX, Info, TriangleAlert, type LucideIcon } from "lucide-react";
import { createElement, memo, useId } from "react";
import { APP_LOG_LEVEL_LABELS, APP_LOG_SOURCE_LABELS } from "../../state/appLog";
import { formatClockMillis, formatClockSeconds } from "../../state/format";
import type { AppLogEntry, AppLogLevel } from "../../types";

const ICONS: Record<AppLogLevel, LucideIcon> = { info: Info, warn: TriangleAlert, error: CircleX };

export const AppLogItem = memo(function AppLogItem({
  entry,
  open,
  fresh = false,
  onToggle,
}: {
  entry: AppLogEntry;
  open: boolean;
  /** 화면을 연 뒤에 들어온 줄 — 살짝 떠오르며 나타남. */
  fresh?: boolean;
  onToggle(id: number): void;
}) {
  const detailId = useId();
  const level = APP_LOG_LEVEL_LABELS[entry.level];
  return (
    <li
      id={`app-log-${entry.id}`}
      className={`applog-item level-${entry.level} ${fresh ? "is-fresh" : ""}`}
      tabIndex={-1}
    >
      <time className="act-time num" dateTime={new Date(entry.at).toISOString()} title={formatClockMillis(entry.at)}>
        {formatClockSeconds(entry.at)}
      </time>
      <span className="act-icon applog-icon" title={level} aria-hidden="true">
        {createElement(ICONS[entry.level], { size: 14, strokeWidth: 2.4 })}
      </span>
      <div className="applog-body">
        <p className="applog-title">
          <span className="sr-only">{level}: </span>
          <span className="applog-source">{APP_LOG_SOURCE_LABELS[entry.source]}</span>
          {entry.title}
          {entry.detail && (
            <button
              type="button"
              className="act-toggle"
              aria-expanded={open}
              aria-controls={detailId}
              aria-label={open ? "접기" : "자세히 보기"}
              title={open ? "접기" : "자세히 보기"}
              onClick={() => onToggle(entry.id)}
            >
              <ChevronRight size={13} strokeWidth={2.6} className={`chevron ${open ? "is-open" : ""}`} />
            </button>
          )}
        </p>
        {entry.detail && open && (
          <p id={detailId} className="applog-detail">
            {entry.detail}
          </p>
        )}
      </div>
    </li>
  );
});
