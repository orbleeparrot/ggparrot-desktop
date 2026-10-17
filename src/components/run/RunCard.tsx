// '내 매크로' 카드 하나. 고르기 버튼(카드 전체)·작은 주 단추·빼기(×)·순서 손잡이(왼쪽 끝, 두 개 이상일 때).
// - 확인 필요 · 실행 중: 두 줄 카드
//   - 1줄: 거래소 로고 · 종목 · 숏 · 모드(실전 · 테스트넷만) …… 오른쪽 칸(실행 중: 손익 %, 대기·끝남: × 자리, 시작·종료 중: 비움)
//   - 2줄: ● 상태 · 경과(실행 중·끝남) · 전략 …… 작은 주 단추 자리
// - 대기(compact): 한 줄 행 — 로고 · 종목 · 숏 · 모드 · 상태 · 전략 …… [▶] · [×] 자리. 시작 전이라 손익이 없어 카드가 필요 없음.
// - 배지는 기본이 아닌 것만(롱 · 모의는 생략) — 배지가 많으면 정작 다른 점이 안 보임.
// - [▶] · [■] · [×]는 마우스를 올리거나 키보드 포커스가 있을 때만 보임.
//   - 두 줄 카드는 자리를 늘 비워 둠(글자가 움직이지 않게)
//   - 한 줄 행은 자리를 비우지 않고 글자 끝 위에 겹쳐 뜸(글자가 일찍 잘리고 오른쪽이 비지 않게) — 숨어 있을 때는 눌리지 않음
// - 좁을 때 줄이는 순서는 run.css(.run-card-head·.run-card-meta)에서 정함 — 글자 길이를 재는 스크립트는 없음.
//   전략은 세 글자쯤 보일 자리가 없으면 앞의 구분점(·)과 함께 통째로 숨음('· …'만 남지 않게).
// - 주 단추·×는 고르기 버튼 밖의 형제 — 비워 둔 자리를 네모난 감싸개가 통째로 덮어 모서리를 눌러도 카드가 골라지지 않음.
import { GripVertical, Timer, X } from "lucide-react";
import { forwardRef, type KeyboardEvent, type PointerEventHandler } from "react";
import { useRunnerContext } from "../../state/context";
import { formatElapsed, formatPercent, toneOf } from "../../state/format";
import { isIdle, isLive, primaryAction, SIDE_LABELS, statusParts } from "../../state/runs";
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
  /** 대기 묶음의 한 줄 행. */
  compact?: boolean;
}

/** 1줄 오른쪽 칸 — 실행 중이면 손익 %(상태 카드와 같은 값), 대기·끝남이면 × 자리, 시작·종료 중이면 비움. */
function End({ slot }: { slot: RunSlot }) {
  const run = slot.run;
  if (isIdle(slot.phase)) return <span className="run-card-x-slot" aria-hidden="true" />;
  if (!run || run.finished || slot.phase !== "running") return null;
  if (!run.inPosition && run.entryWait && !run.finished) {
    return (
      <span className="run-card-wait num" title={`${run.entryWait.sentence} ${run.entryWait.distance}`}>
        {run.entryWait.gap || "진입 대기"}
      </span>
    );
  }
  if (!run.inPosition) return <span className="run-card-flat">무포지션</span>;
  return (
    <span className={`run-card-pct num tone-${toneOf(run.unrealizedPct)}`}>{formatPercent(run.unrealizedPct)}</span>
  );
}

/** 기본이 아닌 것만 배지 — 숏 · 실전 · 테스트넷. */
function Badges({ slot }: { slot: RunSlot }) {
  return (
    <>
      {slot.macro.side === "short" && <span className="side side-short">{SIDE_LABELS.short}</span>}
      {slot.mode !== "mock" && <ModeBadge mode={slot.mode} />}
    </>
  );
}

export const RunCard = forwardRef<HTMLButtonElement, Props>(function RunCard(
  { slot, selected, onKeyNav, grab, sorting = "", compact = false },
  ref,
) {
  const { select, askRemove } = useRunnerContext();
  const elapsed = useElapsed(slot.run);
  const m = slot.macro;
  const live = isLive(slot);
  const idle = isIdle(slot.phase);
  const actionKind = primaryAction(slot.phase).kind;
  const [statusHead, statusRest] = statusParts(slot.status.text);
  const showElapsed = live || !!slot.run?.finished;
  // 전략이 없으면 매크로 이름 — 전체(전략 · 파일 이름)는 마우스를 올리면 보이고, 상세 머리줄에도 있음.
  const strategy = m.strategy || m.title;
  const strategyTitle = m.strategy && m.strategy !== m.title ? `${m.strategy} · ${m.title}` : m.title;
  const cls = `run-card mode-${slot.mode} act-${actionKind} ${compact ? "is-compact" : ""} ${idle ? "has-remove" : ""} ${selected ? "is-selected" : ""} ${live ? "is-live" : ""} ${sorting}`;

  const grip = grab && (
    <span className="run-card-grip" title="끌어서 순서 바꾸기 (Alt+↑·↓)" aria-hidden="true" {...grab}>
      <GripVertical size={14} />
    </span>
  );
  const remove = idle && (
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
  );
  const selectProps = {
    ref,
    className: "run-card-select",
    "data-run-id": slot.id,
    "aria-current": selected ? ("true" as const) : undefined,
    "aria-controls": "run-detail",
    "aria-keyshortcuts": grab ? "Alt+ArrowUp Alt+ArrowDown" : undefined,
    tabIndex: selected ? 0 : -1,
    onClick: () => select(slot.id),
    onKeyDown: onKeyNav,
  };

  if (compact) {
    return (
      <li className={cls}>
        {grip}
        <button type="button" {...selectProps}>
          <ExchangeLogo exchange={m.exchange} size={18} />
          <span className="sr-only">{m.exchangeLabel || m.exchangeName}</span>
          <span className="run-card-symbol num" title={m.symbol}>
            {m.symbol}
          </span>
          <Badges slot={slot} />
          <span className="run-row-line" title={`${slot.status.text} · ${strategyTitle}`}>
            <span className={`run-row-status tone-${slot.status.tone}`}>{slot.status.text}</span>
            <span className="run-row-strategy">{strategy}</span>
          </span>
        </button>
        <div className="run-card-action">
          <RunAction slot={slot} />
        </div>
        {remove}
      </li>
    );
  }

  return (
    <li className={cls}>
      {grip}
      <button type="button" {...selectProps}>
        <span className="run-card-top">
          <span className="run-card-head">
            <ExchangeLogo exchange={m.exchange} size={20} />
            <span className="sr-only">{m.exchangeLabel || m.exchangeName}</span>
            <span className="run-card-symbol num" title={m.symbol}>
              {m.symbol}
            </span>
            <Badges slot={slot} />
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
      {remove}
    </li>
  );
});
