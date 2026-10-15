// '내 매크로' 카드 하나 — 두 줄. 고르기 버튼(카드 전체)·주 버튼·빼기(×)·순서 손잡이(왼쪽 끝, 두 개 이상일 때).
// - 1줄: 거래소 로고 · 종목 · 롱/숏 · 모드 …… 오른쪽 칸(실행 중: 손익 %, 대기·끝남: × 자리, 시작·종료 중: 비움)
// - 2줄: ● 상태 · 경과(실행 중·끝남) · 전략 …… 주 버튼 자리
// - 좁을 때 줄이는 순서는 run.css(.run-card-head·.run-card-meta)에서 정함 — 글자 길이를 재는 스크립트는 없음.
//   전략은 세 글자쯤 보일 자리가 없으면 앞의 구분점(·)과 함께 통째로 숨음('· …'만 남지 않게).
// - ×·주 버튼은 고르기 버튼 밖의 형제 — 비워 둔 자리를 네모난 감싸개가 통째로 덮어 모서리를 눌러도 카드가 골라지지 않음.
import { GripVertical, Timer, X } from "lucide-react";
import { forwardRef, type KeyboardEvent, type PointerEventHandler } from "react";
import { useRunnerContext } from "../../state/context";
import { formatElapsed, formatPercent, toneOf } from "../../state/format";
import { isIdle, isLive, primaryAction, SIDE_LABELS, statusParts, stopChoices } from "../../state/runs";
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

/** 1줄 오른쪽 칸 — 실행 중이면 손익 %(상태 카드와 같은 값), 대기·끝남이면 × 자리, 시작·종료 중이면 비움. */
function End({ slot }: { slot: RunSlot }) {
  const run = slot.run;
  if (isIdle(slot.phase)) return <span className="run-card-x-slot" aria-hidden="true" />;
  if (!run || run.finished || slot.phase !== "running") return null;
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
  const idle = isIdle(slot.phase);
  // 주 버튼 종류 — 버튼 너비만큼 2줄 오른쪽을 비움(run.css --act-w).
  const actionKind = primaryAction(slot.phase).kind;
  const actionClass = actionKind === "stop" && stopChoices(slot).length > 1 ? "act-stop-menu" : `act-${actionKind}`;
  const [statusHead, statusRest] = statusParts(slot.status.text);
  const showElapsed = live || !!slot.run?.finished;
  // 전략이 없으면 매크로 이름 — 전체(전략 · 파일 이름)는 마우스를 올리면 보이고, 상세 머리줄에도 있음.
  const strategy = m.strategy || m.title;
  const strategyTitle = m.strategy && m.strategy !== m.title ? `${m.strategy} · ${m.title}` : m.title;

  return (
    <li
      className={`run-card mode-${slot.mode} ${actionClass} ${idle ? "has-remove" : ""} ${selected ? "is-selected" : ""} ${live ? "is-live" : ""} ${sorting}`}
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
        <span className="run-card-top">
          <span className="run-card-head">
            <ExchangeLogo exchange={m.exchange} size={20} />
            <span className="sr-only">{m.exchangeLabel || m.exchangeName}</span>
            <span className="run-card-symbol num" title={m.symbol}>
              {m.symbol}
            </span>
            <span className={`side side-${m.side}`}>{SIDE_LABELS[m.side]}</span>
            <ModeBadge mode={slot.mode} />
          </span>
          <End slot={slot} />
        </span>
        <span className="run-card-meta">
          <span className={`run-card-status tone-${slot.status.tone}`} title={slot.status.text}>
            <span className={`status-dot ${slot.phase === "running" ? "is-live" : ""}`} aria-hidden="true" />
            <span className="run-card-status-head">{statusHead}</span>
          </span>
          {statusRest && (
            <span className={`run-card-status-rest tone-${slot.status.tone}`} title={slot.status.text}>
              <span className="run-card-status-more">{statusRest}</span>
            </span>
          )}
          {showElapsed && (
            <span className="run-card-elapsed num" title="경과 시간">
              <Timer size={12} aria-hidden="true" />
              <span className="sr-only">경과 </span>
              {formatElapsed(elapsed)}
            </span>
          )}
          {/* 바깥은 자리(좁으면 통째로 가려짐), 안쪽은 구분점과 글자(줄임표) — run.css .run-card-strategy */}
          <span className="run-card-strategy" title={strategyTitle}>
            <span className="run-card-strategy-text">{strategy}</span>
          </span>
        </span>
      </button>
      <div className="run-card-action">
        <RunAction slot={slot} />
      </div>
      {idle && (
        <div className="run-card-remove-slot">
          <button
            type="button"
            className="icon-btn run-card-remove"
            aria-label={`${m.symbol} 목록에서 빼기`}
            title="목록에서 빼기"
            onClick={() => askRemove(slot.id)}
          >
            <X size={14} />
          </button>
        </div>
      )}
    </li>
  );
});
