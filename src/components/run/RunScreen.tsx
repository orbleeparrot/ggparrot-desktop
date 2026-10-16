// 실행 화면 — ≥ 900px 두 열(목록·상세), 좁으면 목록 → 상세(←로 돌아옴). 슬롯이 없으면 첫 실행 안내.
import type { Runner } from "../../state/useRunner";
import { RunDetail } from "./RunDetail";
import { EmptyState } from "./EmptyState";
import { RunList } from "./RunList";

interface Props {
  runner: Runner;
  selectedId: string | null;
  wide: boolean;
  detailOpen: boolean;
  onBack(): void;
}

export function RunScreen({ runner, selectedId, wide, detailOpen, onBack }: Props) {
  const runs = runner.state?.runs ?? [];
  if (runs.length === 0) return <EmptyState />;
  const slot = runs.find((r) => r.id === selectedId) ?? null;
  const detail = slot && (
    <RunDetail
      key={slot.id}
      slot={slot}
      activity={runner.activity[slot.id] ?? []}
      prices={runner.prices[slot.id]?.points ?? []}
      priceTimes={runner.prices[slot.id]?.times ?? []}
      loadActivity={runner.loadActivity}
      activityFailed={!!runner.activityFailed[slot.id]}
      onBack={wide ? undefined : onBack}
    />
  );
  if (!wide)
    return (
      <div className="run-screen is-narrow">{detailOpen && detail ? detail : <RunList selectedId={selectedId} />}</div>
    );
  return (
    <div className="run-screen">
      <RunList selectedId={selectedId} />
      {detail}
    </div>
  );
}
