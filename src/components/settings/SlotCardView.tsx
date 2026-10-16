// 거래소 연결 칸 하나 — 로고·이름·상태 배지·키 두 칸·기억하기·안내·세 줄 확인·[다시 확인]·[연결 도우미].
// - 쓰지 않는 칸(키 없음·연결 안 됨·실행에 안 쓰임)은 한 줄로 접어 둠: 로고 · 이름 · 연결 안 됨 · [연결 도우미] · 펼치기(›).
//   펼치면 키 칸이 보임. 접혀 있어도 고쳐야 할 안내(warn)는 그대로 보임.
// - 안내 색은 Rust가 주는 noteTone("warn"·"muted"). 없으면 막힘(blocked)·확인 실패·잔고·권한·허용 IP 문구로 짐작함.
import { ChevronRight, LoaderCircle, RefreshCw, TriangleAlert, Wand2 } from "lucide-react";
import { useRunnerContext } from "../../state/context";
import { noteToneOf, slotInUse } from "../../state/slots";
import type { SlotCard } from "../../types";
import { CheckChips } from "../../wizard/CheckRows";
import { ExchangeLogo } from "../ExchangeLogo";
import { SecretField } from "../SecretField";
import { InUseNote } from "./InUseNote";
import { keyStoreName } from "../../platform";

// 바이낸스 연습 칸의 로고 꼬리표.
const PRACTICE_TAG: Partial<Record<SlotCard["slot"], string>> = { binance_testnet: "TEST", binance_demo: "DEMO" };

// 바이낸스 칸마다 어느 사이트 키를 넣는지·어느 시장으로 확인하는지.
const BINANCE_NOTES: Partial<Record<SlotCard["slot"], string>> = {
  binance:
    "연결 확인은 이 칸을 쓰는 첫 매크로의 시장(현물 · 선물) 기준이에요. 시장이 다른 매크로는 시작할 때 다시 확인해요.",
  binance_testnet: "현물 테스트넷(testnet.binance.vision) 키를 넣어요. 선물 연습 키는 바이낸스 데모 칸에 넣어요.",
  binance_demo: "선물 데모(demo.binance.com) 키를 넣어요. 현물 연습 키는 바이낸스 테스트넷 칸에 넣어요.",
};

function badgeTone(s: SlotCard): string {
  if (s.status === "connected") return "ok";
  if (s.status === "blocked") return "warn";
  return "muted";
}

interface Props {
  slot: SlotCard;
  /** 쓰지 않는 칸을 펼쳤는지 — 설정 화면이 들고 있음(두 칸 배치에서 짝 없는 칸을 넓히려고). 쓰는 칸은 늘 펼침. */
  open: boolean;
  onToggle(open: boolean): void;
  /** 한 줄을 다 씀 — 펼친 칸 가운데 짝이 없는 칸(wideSlots). */
  wide?: boolean;
}

export function SlotCardView({ slot, open, onToggle, wide = false }: Props) {
  const { act } = useRunnerContext();
  const filled = slot.accessFilled && slot.secretFilled;
  const id = `settings-slot-${slot.slot}`;
  const showChecks = slot.status === "connected" || slot.status === "blocked";
  const fields = ["access", "secret"] as const;
  const tag = PRACTICE_TAG[slot.slot];
  const binanceNote = BINANCE_NOTES[slot.slot];
  const warn = noteToneOf(slot) === "warn";
  const collapsible = !slotInUse(slot);
  const expanded = !collapsible || open;
  const bodyId = `${id}-body`;

  return (
    <article
      id={id}
      className={`slot-card status-${slot.status} ${expanded ? "" : "is-collapsed"} ${wide ? "is-wide" : ""}`}
      tabIndex={-1}
      aria-labelledby={`${id}-title`}
    >
      <header className="slot-head">
        <span className="slot-logo">
          <ExchangeLogo exchange={slot.exchange} size={collapsible ? 22 : 26} />
          {tag && (
            <span className="slot-test" aria-hidden="true">
              {tag}
            </span>
          )}
        </span>
        <h4 id={`${id}-title`} className="slot-title">
          {slot.label}
        </h4>
        <span className={`slot-badge tone-${badgeTone(slot)}`} aria-live="polite">
          {slot.status === "checking" && <LoaderCircle size={12} className="spin" aria-hidden="true" />}
          {slot.headline}
        </span>
        {slot.wizardAvailable && (
          <button
            type="button"
            className={`btn btn-sm ${slot.status === "connected" || collapsible ? "btn-ghost" : "btn-primary"} slot-wizard`}
            onClick={() => act("wizard_open", { exchange: slot.exchange, slot: slot.slot })}
          >
            <Wand2 size={13} aria-hidden="true" />
            연결 도우미
          </button>
        )}
        {collapsible && (
          <button
            type="button"
            className={`icon-btn slot-toggle ${slot.wizardAvailable ? "" : "is-alone"}`}
            aria-expanded={expanded}
            aria-controls={bodyId}
            aria-label={`${slot.label} 키 칸 ${expanded ? "접기" : "펼치기"}`}
            title={expanded ? "접기" : "키 직접 넣기"}
            onClick={() => onToggle(!expanded)}
          >
            <ChevronRight size={15} strokeWidth={2.4} className={`chevron ${expanded ? "is-open" : ""}`} />
          </button>
        )}
      </header>

      {slot.note && warn && (
        <p className="note note-warn">
          <TriangleAlert size={14} aria-hidden="true" />
          {slot.note}
        </p>
      )}

      <div id={bodyId} className="slot-body" hidden={!expanded}>
        <div className="slot-fields">
          {fields.map((field, i) => (
            <SecretField
              key={field}
              id={`slot-${slot.slot}-${field}`}
              label={slot.fieldLabels[i]}
              filled={field === "access" ? slot.accessFilled : slot.secretFilled}
              onCommit={(value) => act("set_slot_key", { slot: slot.slot, field, value })}
              onReveal={() => act("reveal_slot_key", { slot: slot.slot, field })}
            />
          ))}
        </div>

        {slot.rememberSupported && (
          <label className="check-row">
            <input
              type="checkbox"
              checked={slot.remembered}
              onChange={(e) => act("set_slot_remember", { slot: slot.slot, remember: e.target.checked })}
            />
            <span>이 기기에 키 기억하기 ({keyStoreName()}에 보관)</span>
          </label>
        )}
        {slot.note && !warn && <p className="note note-muted">{slot.note}</p>}
        {binanceNote && <p className="note note-muted">{binanceNote}</p>}
        <InUseNote count={slot.inUseBy} />

        {showChecks && (
          <div className="slot-checks">
            <CheckChips checks={slot.checks} />
            <button
              type="button"
              className="link-btn slot-recheck"
              onClick={() => act("check_slot", { slot: slot.slot })}
              disabled={!filled}
            >
              <RefreshCw size={13} aria-hidden="true" /> 다시 확인
            </button>
          </div>
        )}
        {slot.status === "unchecked" && filled && (
          <button type="button" className="link-btn" onClick={() => act("check_slot", { slot: slot.slot })}>
            <RefreshCw size={13} aria-hidden="true" /> 지금 확인
          </button>
        )}
      </div>
    </article>
  );
}
