// 거래소 연결 칸 하나 — 로고·이름·상태 배지·키 두 칸·기억하기·안내·세 줄 확인·[다시 확인]·[연결 도우미].
import { LoaderCircle, RefreshCw, Wand2 } from "lucide-react";
import { useRunnerContext } from "../../state/context";
import type { SlotCard } from "../../types";
import { CheckRows } from "../../wizard/CheckRows";
import { ExchangeLogo } from "../ExchangeLogo";
import { SecretField } from "../SecretField";
import { InUseNote } from "./InUseNote";
import { keyStoreName } from "../../platform";

function badgeTone(s: SlotCard): string {
  if (s.status === "connected") return "ok";
  if (s.status === "blocked") return "warn";
  return "muted";
}

export function SlotCardView({ slot }: { slot: SlotCard }) {
  const { act } = useRunnerContext();
  const filled = slot.accessFilled && slot.secretFilled;
  const id = `settings-slot-${slot.slot}`;
  const showChecks = slot.status === "connected" || slot.status === "blocked";
  const fields = ["access", "secret"] as const;

  return (
    <article id={id} className={`slot-card status-${slot.status}`} tabIndex={-1} aria-labelledby={`${id}-title`}>
      <header className="slot-head">
        <span className="slot-logo">
          <ExchangeLogo exchange={slot.exchange} size={26} />
          {slot.slot === "binance_testnet" && (
            <span className="slot-test" aria-hidden="true">
              TEST
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
            className={`btn btn-sm ${slot.status === "connected" ? "btn-ghost" : "btn-primary"} slot-wizard`}
            onClick={() => act("wizard_open", { exchange: slot.exchange as "upbit" | "bithumb" })}
          >
            <Wand2 size={13} aria-hidden="true" />
            연결 도우미
          </button>
        )}
      </header>

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
      {slot.note && <p className={`note ${slot.status === "blocked" ? "note-warn" : "note-muted"}`}>{slot.note}</p>}
      {slot.exchange === "binance" && (
        // 칸의 연결 확인은 현물(BTCUSDT)로만 함 — 선물 키·선물 테스트넷 키는 시작할 때 따로 확인됨.
        <p className="note note-muted">
          연결 확인은 현물(BTCUSDT) 기준이에요. 선물 매크로는 시작할 때 다시 확인해요.
          {slot.slot === "binance_testnet" && " 현물 테스트넷과 선물 테스트넷의 키는 서로 달라요."}
        </p>
      )}
      <InUseNote count={slot.inUseBy} />

      {showChecks && (
        <div className="slot-checks">
          <CheckRows checks={slot.checks} ip="" />
          <button
            type="button"
            className="link-btn"
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
    </article>
  );
}
