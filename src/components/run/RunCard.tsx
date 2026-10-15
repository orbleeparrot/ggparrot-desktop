// '내 매크로' 카드 하나 — 고르기 버튼(카드 전체)·주 버튼·빼기(×).
import { X } from "lucide-react";
import { forwardRef, type KeyboardEvent } from "react";
import { useRunnerContext } from "../../state/context";
import { formatElapsed, formatPercent, toneOf } from "../../state/format";
import { isIdle, isLive, SIDE_LABELS } from "../../state/runs";
import type { RunSlot } from "../../types";
import { ExchangeLogo } from "../ExchangeLogo";
import { ModeBadge } from "./ModeBadge";
import { RunAction } from "./RunAction";
import { useElapsed } from "./useElapsed";

interface Props {
  slot: RunSlot;
  selected: boolean;
  onKeyNav(e: KeyboardEvent<HTMLButtonElement>): void;
}

function Figure({ slot }: { slot: RunSlot }) {
  const run = slot.run;
  if (!run || !isLive(slot)) return null;
  if (!run.inPosition) return <span className="run-card-flat">무포지션</span>;
  return (
    <span className={`run-card-pct num tone-${toneOf(run.unrealizedPct)}`}>{formatPercent(run.unrealizedPct)}</span>
  );
}

export const RunCard = forwardRef<HTMLButtonElement, Props>(function RunCard({ slot, selected, onKeyNav }, ref) {
  const { select, askRemove } = useRunnerContext();
  const elapsed = useElapsed(slot.run);
  const m = slot.macro;
  const live = isLive(slot);
  const sub = live || slot.run?.finished ? `경과 ${formatElapsed(elapsed)}` : m.title;

  return (
    <li className={`run-card mode-${slot.mode} ${selected ? "is-selected" : ""} ${live ? "is-live" : ""}`}>
      <button
        ref={ref}
        type="button"
        className="run-card-select"
        data-run-id={slot.id}
        aria-current={selected ? "true" : undefined}
        aria-controls="run-detail"
        tabIndex={selected ? 0 : -1}
        onClick={() => select(slot.id)}
        onKeyDown={onKeyNav}
      >
        <span className="run-card-head">
          <ExchangeLogo exchange={m.exchange} size={20} />
          <span className="sr-only">{m.exchangeLabel || m.exchangeName}</span>
          <span className="run-card-symbol num">{m.symbol}</span>
          <span className={`side side-${m.side}`}>{SIDE_LABELS[m.side]}</span>
          <ModeBadge mode={slot.mode} />
        </span>
        <span className="run-card-mid">
          <span className={`run-card-status tone-${slot.status.tone}`}>
            <span className={`status-dot ${slot.phase === "running" ? "is-live" : ""}`} aria-hidden="true" />
            <span className="run-card-status-text" title={slot.status.text}>
              {slot.status.text}
            </span>
          </span>
          <Figure slot={slot} />
        </span>
        <span className="run-card-sub num" title={m.title}>
          {sub}
        </span>
      </button>
      <div className="run-card-action">
        <RunAction slot={slot} />
      </div>
      {isIdle(slot.phase) && (
        <button
          type="button"
          className="icon-btn run-card-remove"
          aria-label={`${m.symbol} 목록에서 빼기`}
          title="목록에서 빼기"
          onClick={() => askRemove(slot.id)}
        >
          <X size={14} />
        </button>
      )}
    </li>
  );
});
