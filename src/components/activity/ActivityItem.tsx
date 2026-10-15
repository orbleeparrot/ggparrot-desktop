// 활동 한 줄 — 시각·종류 아이콘·굵은 제목·흐린 둘째 줄·[자세히](원문 줄). 원문 줄(dev)은 작고 흐린 고정폭 한 줄.
import {
  ArrowDownLeft,
  ArrowUpRight,
  Bell,
  ChevronRight,
  Info,
  PauseCircle,
  Play,
  Shield,
  Square,
  TriangleAlert,
  Wifi,
  WifiOff,
  type LucideIcon,
} from "lucide-react";
import { memo, useId, useState } from "react";
import { formatClock, formatClockSeconds } from "../../state/format";
import type { Activity, ActivityKind } from "../../types";

const ICONS: Record<Exclude<ActivityKind, "connection">, LucideIcon> = {
  start: Play,
  signal: Bell,
  buy: ArrowDownLeft,
  sell: ArrowUpRight,
  stop_loss: Shield,
  blocked: PauseCircle,
  error: TriangleAlert,
  stop: Square,
  info: Info,
};

export const KIND_LABELS: Record<ActivityKind, string> = {
  start: "시작",
  signal: "신호",
  buy: "진입",
  sell: "정리",
  stop_loss: "손절",
  blocked: "보류",
  connection: "연결",
  error: "오류",
  stop: "종료",
  info: "안내",
};

function iconOf(a: Activity): LucideIcon {
  if (a.kind === "connection") return a.tone === "ok" ? Wifi : WifiOff;
  return ICONS[a.kind];
}

/** fresh: 화면을 연 뒤에 들어온 줄 — 살짝 떠오르며 나타남(처음 불러온 기록은 그대로). */
export const ActivityItem = memo(function ActivityItem({ item, fresh = false }: { item: Activity; fresh?: boolean }) {
  const [open, setOpen] = useState(false);
  const rawId = useId();
  const time = (
    <time className="act-time num" dateTime={new Date(item.at).toISOString()} title={formatClockSeconds(item.at)}>
      {formatClock(item.at)}
    </time>
  );

  if (item.dev) {
    return (
      <li className="act act-dev">
        {time}
        <span className="act-dev-mark" aria-hidden="true" />
        <span className="act-dev-text">{item.title}</span>
      </li>
    );
  }

  const Icon = iconOf(item);
  const fill = item.kind === "start" || item.kind === "stop";
  return (
    <li className={`act kind-${item.kind} ${fresh ? "is-fresh" : ""}`} data-tone={item.tone}>
      {time}
      <span className="act-icon" aria-hidden="true">
        <Icon size={14} strokeWidth={2.4} fill={fill ? "currentColor" : "none"} />
      </span>
      <div className="act-body">
        <p className="act-title">
          <span className="sr-only">{KIND_LABELS[item.kind]}: </span>
          {item.title}
        </p>
        {item.detail && <p className="act-detail">{item.detail}</p>}
        {item.raw && (
          <>
            <button
              type="button"
              className="act-more"
              aria-expanded={open}
              aria-controls={rawId}
              onClick={() => setOpen(!open)}
            >
              <ChevronRight size={12} className={`chevron ${open ? "is-open" : ""}`} aria-hidden="true" />
              자세히
            </button>
            {open && (
              <pre id={rawId} className="act-raw">
                {item.raw}
              </pre>
            )}
          </>
        )}
      </div>
    </li>
  );
});
