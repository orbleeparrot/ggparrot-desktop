// 설정 › 앱 기록 — 매크로 하나에 묶이지 않는 앱 전체의 일(알림·업데이트·웹 연결·키 저장 등). 최근 것이 위.
// - 필터 칩: 전체·주의·문제. [기록 복사]는 Rust가 전체를 클립보드에 넣음(copy_app_log).
// - 카드 안에서만 굴러감(설정 화면이 끝없이 길어지지 않게).
// - focus: 알림 토스트의 [자세히] — 그 줄이 보이게 필터를 풀고, 펼치고, 굴려서 잠깐 밝힘. 없으면 묶음으로만 감.
import { Check, Copy } from "lucide-react";
import { SwapLabel } from "../SwapLabel";
import { useCallback, useEffect, useMemo, useState } from "react";
import {
  APP_LOG_FILTERS,
  countAppLog,
  inAppLogFilter,
  MAX_APP_LOG,
  visibleAppLog,
  type AppLogFilter,
} from "../../state/appLog";
import { useRunnerContext } from "../../state/context";
import type { SettingsTarget } from "../../state/runs";
import { useAppLog } from "../../state/useAppLog";
import { useCopiedFlag } from "../useCopiedFlag";
import { AppLogItem } from "./AppLogItem";
import { goToSettings } from "./goTo";
import { Section } from "./Section";

const EMPTY_TEXT: Record<AppLogFilter, string> = {
  all: "아직 기록이 없어요. 알림과 앱 전체의 일이 여기에 남아요.",
  warn: "주의할 기록이 없어요.",
  error: "문제 기록이 없어요.",
};

export function AppLogSection({ focus }: { focus: { id: number; seq: number } | null }) {
  const { ok } = useRunnerContext();
  const { entries, status, reload } = useAppLog();
  const [filter, setFilter] = useState<AppLogFilter>("all");
  const [expanded, setExpanded] = useState<ReadonlySet<number>>(() => new Set());
  const [copied, markCopied] = useCopiedFlag(1500);

  const shown = useMemo(() => visibleAppLog(entries, filter), [entries, filter]);
  const counts = useMemo(() => countAppLog(entries), [entries]);
  // 처음 읽은 뒤에 들어온 줄만 움직임을 줌(렌더 중 조건부 갱신).
  const [baseline, setBaseline] = useState<number | null>(null);
  if (baseline === null && status !== "loading") setBaseline(entries.at(-1)?.id ?? 0);

  const toggle = useCallback((id: number) => {
    setExpanded((prev) => {
      const next = new Set(prev);
      if (!next.delete(id)) next.add(id);
      return next;
    });
  }, []);

  // [자세히]로 온 줄 — 지난 기록을 다 읽은 뒤 한 번만: 그 줄이 보이게 필터를 풀고 펼침(렌더 중 조건부 갱신).
  // 최근 500줄에서 밀려났으면 묶음으로만 감.
  const [seenSeq, setSeenSeq] = useState(0);
  const [goal, setGoal] = useState<SettingsTarget | null>(null);
  if (focus && focus.seq !== seenSeq && status !== "loading") {
    setSeenSeq(focus.seq);
    const entry = entries.find((e) => e.id === focus.id);
    if (entry && !inAppLogFilter(entry.level, filter)) setFilter("all");
    if (entry?.detail && !expanded.has(entry.id)) setExpanded(new Set(expanded).add(entry.id));
    setGoal(entry ? { kind: "log", id: entry.id } : { kind: "section", section: "log" });
  }
  // 펼친 설명까지 그린 다음 가운데로 굴림.
  useEffect(() => {
    if (!goal) return;
    const id = requestAnimationFrame(() => goToSettings(goal));
    return () => cancelAnimationFrame(id);
  }, [goal]);

  return (
    <Section
      id="log"
      title="앱 기록"
      lead={`알림과 앱 전체의 일을 최근 ${MAX_APP_LOG}줄까지 보여 줘요. 앱을 끄면 지워지니, 문의할 때는 [기록 복사]로 붙여 넣어 주세요.`}
    >
      <div className="plain-card applog">
        <div className="feed-head applog-head">
          <div className="feed-filters" role="group" aria-label="기록 수준">
            {APP_LOG_FILTERS.map((f) => (
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
            onClick={async () => (await ok("copy_app_log")) && markCopied()}
            disabled={entries.length === 0}
            title="앱 기록 전체를 복사해요"
          >
            <SwapLabel
              active={copied ? 1 : 0}
              items={[
                <>
                  <Copy size={14} aria-hidden="true" />
                  기록 복사
                </>,
                <>
                  <Check size={14} aria-hidden="true" />
                  복사했어요
                </>,
              ]}
            />
          </button>
        </div>
        <div className="applog-scroll" role="log" aria-label="앱 기록" tabIndex={0}>
          {shown.length > 0 ? (
            <ol className="applog-list">
              {shown.map((e) => (
                <AppLogItem
                  key={e.id}
                  entry={e}
                  open={expanded.has(e.id)}
                  fresh={baseline !== null && e.id > baseline}
                  onToggle={toggle}
                />
              ))}
            </ol>
          ) : status === "error" ? (
            <p className="feed-empty" role="alert">
              앱 기록을 불러오지 못했어요.{" "}
              <button type="button" className="btn btn-quiet btn-sm" onClick={reload}>
                다시 불러오기
              </button>
            </p>
          ) : (
            <p className="feed-empty">{status === "loading" ? "불러오는 중…" : EMPTY_TEXT[filter]}</p>
          )}
        </div>
      </div>
    </Section>
  );
}
