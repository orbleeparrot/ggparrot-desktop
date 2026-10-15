// '내 매크로' 카드 하나 — 고르기 버튼(카드 전체)·주 버튼·빼기(×)·순서 손잡이(왼쪽 끝, 두 개 이상일 때).
import { GripVertical, X } from "lucide-react";
import { forwardRef, type KeyboardEvent, type PointerEventHandler } from "react";
import { useRunnerContext } from "../../state/context";
import { formatElapsed, formatPercent, toneOf } from "../../state/format";
import { isIdle, isLive, primaryAction, SIDE_LABELS } from "../../state/runs";
import type { RunSlot } from "../../types";
import { ExchangeLogo } from "../ExchangeLogo";
import { ModeBadge } from "./ModeBadge";
import { RunAction } from "./RunAction";
import { useElapsed } from "./useElapsed";

interface Grab {
  onPointerDown: PointerEventHandler<HTMLElement>;
  onPointerMove: PointerEventHandler<HTMLElement>;
  onPointerUp: PointerEventHandler<HTMLElement>;
  onPointerCancel: PointerEventHandler<HTMLElement>;
}

interface Props {
  slot: RunSlot;
  selected: boolean;
  onKeyNav(e: KeyboardEvent<HTMLButtonElement>): void;
  /** 순서 손잡이 — 목록이 하나뿐이면 없음. */
  grab?: Grab;
  /** 끄는 중 표시: "is-dragging"(끄는 카드)·"is-drop-before"·"is-drop-after"(놓을 자리). */
  sorting?: string;
}

function Figure({ slot }: { slot: RunSlot }) {
  const run = slot.run;
  if (!run || !isLive(slot)) return null;
  if (!run.inPosition) return <span className="run-card-flat">무포지션</span>;
  return (
    <span className={`run-card-pct num tone-${toneOf(run.unrealizedPct)}`}>{formatPercent(run.unrealizedPct)}</span>
  );
}

export const RunCard = forwardRef<HTMLButtonElement, Props>(function RunCard(
  { slot, selected, onKeyNav, grab, sorting = "" },
  ref,
) {
  const { select, askRemove } = useRunnerContext();
  const elapsed = useElapsed(slot.run);
  const m = slot.macro;
  const live = isLive(slot);
  // 주 버튼 종류 — 버튼 너비만큼 아래 줄 글자 자리를 비움(run.css .act-*).
  const actionKind = primaryAction(slot.phase).kind;
  // 대기 중에는 어떤 매크로인지(전략·요약) — 파일 이름은 길어서 마우스를 올리면 보이고, 상세 머리줄에도 있음.
  const sub = live || slot.run?.finished ? `경과 ${formatElapsed(elapsed)}` : m.strategy || m.title;

  return (
    <li
      className={`run-card mode-${slot.mode} act-${actionKind} ${selected ? "is-selected" : ""} ${live ? "is-live" : ""} ${sorting}`}
    >
      {grab && (
        <span className="run-card-grip" title="끌어서 순서 바꾸기 (Alt+↑·↓)" aria-hidden="true" {...grab}>
          <GripVertical size={14} />
        </span>
      )}
      <button
        ref={ref}
        type="button"
        className="run-card-select"
        data-run-id={slot.id}
        aria-current={selected ? "true" : undefined}
        aria-controls="run-detail"
        aria-keyshortcuts={grab ? "Alt+ArrowUp Alt+ArrowDown" : undefined}
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
