// 활동 기록 — 밝은 카드. 아래가 최신이고, 맨 아래에 있으면 따라 내려감(위로 굴리면 멈춤).
// 필터 칩: 전체·거래·신호·문제. 원문 줄은 설정의 '개발자 로그 보기'를 켰을 때만. [복사]는 원문 포함 전체.
import { ArrowDown, Check, Copy } from "lucide-react";
import { useEffect, useLayoutEffect, useMemo, useRef, useState } from "react";
import { ACTIVITY_FILTERS, countByFilter, visibleActivity, type ActivityFilter } from "../../state/activity";
import { useRunnerContext } from "../../state/context";
import type { Activity } from "../../types";
import { ActivityItem } from "./ActivityItem";

const NEAR_BOTTOM_PX = 32;

export function ActivityFeed({
  runId,
  items,
  loadFailed = false,
  onRetry,
}: {
  runId: string;
  items: Activity[];
  loadFailed?: boolean;
  onRetry?: () => void;
}) {
  const { state, ok } = useRunnerContext();
  const showDev = state.settings.showDevLog;
  const [filter, setFilter] = useState<ActivityFilter>("all");
  const [follow, setFollow] = useState(true);
  const [copied, setCopied] = useState(false);
  const box = useRef<HTMLDivElement>(null);
  const programmatic = useRef(false);

  const shown = useMemo(() => visibleActivity(items, filter, showDev), [items, filter, showDev]);
  const counts = useMemo(() => countByFilter(items), [items]);
  const lastId = shown.length ? shown[shown.length - 1].id : 0;
  const seenId = useRef(lastId);
  // 처음 그린 뒤에 들어온 줄만 움직임을 줌.
  const baseline = useRef<number | null>(null);
  if (baseline.current === null && items.length > 0) baseline.current = items[items.length - 1].id;
  if (follow) seenId.current = lastId;
  const unseen = !follow && lastId > seenId.current;

  useLayoutEffect(() => {
    const el = box.current;
    if (!el || !follow) return;
    programmatic.current = true;
    el.scrollTop = el.scrollHeight;
  }, [lastId, shown.length, follow]);

  useEffect(() => {
    if (!copied) return;
    const t = setTimeout(() => setCopied(false), 1500);
    return () => clearTimeout(t);
  }, [copied]);

  function onScroll() {
    const el = box.current;
    if (!el) return;
    if (programmatic.current) {
      programmatic.current = false;
      return;
    }
    const atBottom = el.scrollHeight - el.scrollTop - el.clientHeight <= NEAR_BOTTOM_PX;
    if (atBottom !== follow) setFollow(atBottom);
  }

  function jump() {
    setFollow(true);
  }

  return (
    <section className="feed" aria-labelledby="feed-title">
      <div className="feed-head">
        <h3 id="feed-title" className="feed-title">
          활동 기록
        </h3>
        <div className="feed-filters" role="group" aria-label="기록 종류">
          {ACTIVITY_FILTERS.map((f) => (
            <button
              key={f.id}
              type="button"
              className={`feed-chip ${filter === f.id ? "is-on" : ""}`}
              aria-pressed={filter === f.id}
              onClick={() => setFilter(f.id)}
            >
              {f.label}
              {f.id !== "all" && counts[f.id] > 0 && <span className="feed-chip-count num">{counts[f.id]}</span>}
            </button>
          ))}
        </div>
        <button
          type="button"
          className="btn btn-quiet btn-sm feed-copy"
          onClick={async () => (await ok("copy_activity", { runId })) && setCopied(true)}
          disabled={items.length === 0}
        >
          {copied ? <Check size={14} aria-hidden="true" /> : <Copy size={14} aria-hidden="true" />}
          {copied ? "복사됨" : "전체 기록 복사"}
        </button>
      </div>
      <div
        ref={box}
        className={`feed-body ${showDev ? "with-dev" : ""}`}
        role="log"
        aria-label="활동 기록"
        tabIndex={0}
        onScroll={onScroll}
      >
        {shown.length === 0 && loadFailed ? (
          <p className="feed-empty" role="alert">
            지난 기록을 불러오지 못했어요.{" "}
            {onRetry && (
              <button type="button" className="btn btn-quiet btn-sm" onClick={onRetry}>
                다시 불러오기
              </button>
            )}
          </p>
        ) : shown.length === 0 ? (
          <p className="feed-empty">
            {items.length === 0 || filter === "all"
              ? "아직 기록이 없어요. 시작하면 신호와 거래가 여기에 쌓여요."
              : "이 종류의 기록은 아직 없어요."}
          </p>
        ) : (
          <ol className="feed-list">
            {shown.map((a) => (
              <ActivityItem key={a.id} item={a} fresh={baseline.current !== null && a.id > baseline.current} />
            ))}
          </ol>
        )}
      </div>
      {!follow && (
        <button type="button" className={`feed-jump ${unseen ? "has-new" : ""}`} onClick={jump}>
          <ArrowDown size={14} aria-hidden="true" />
          {unseen ? "새 기록" : "맨 아래로"}
        </button>
      )}
    </section>
  );
}
