// 거래소 연결 한 줄 — 왼쪽: 로고 · 이름 · 연결 상태, 그 아래 세 확인 띠(키 인증 → 허용 IP → 주문 권한)와 고칠 일 한 줄.
// 오른쪽 끝: 잔고(연결 확인 때 읽은 값 · 확인 시각), 막혔으면 그 자리에 노란 [연결 도우미]. 맨 끝 ▾로 키 칸을 열고 닫음.
// - 키 칸(두 칸 · 기억하기 · 안내 · [다시 확인] · [연결 도우미])은 처음엔 모두 닫혀 있음 — 화면이 키 칸으로 가득하지 않게.
// - 안내 색은 Rust가 주는 noteTone("warn"·"muted"). 없으면 막힘(blocked)·확인 실패·잔고·권한·허용 IP 문구로 짐작함.
// - 막힌 이유가 따로 없으면(국내 거래소) 세 확인의 빨간 칸 이유와 이 기기 공인 IP로 한 줄을 만듦.
import { ChevronDown, LoaderCircle, RefreshCw, TriangleAlert, Wand2 } from "lucide-react";
import { useRunnerContext } from "../../state/context";
import { formatClock } from "../../state/format";
import { noteToneOf } from "../../state/slots";
import type { SlotCard } from "../../types";
import { CheckStrip } from "../CheckStrip";
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

const BAD_TEXT = {
  auth: "키 인증 · 키가 맞지 않음",
  ip: "허용 IP · 등록된 IP와 다름",
  order: "주문 권한 · 주문하기 권한 없음",
};

function badgeTone(s: SlotCard): string {
  if (s.status === "connected") return "ok";
  if (s.status === "blocked") return "warn";
  return "muted";
}

/** "사용 가능 잔고 12,300 KRW · 보유 BTC 0.01" → 이름 · 값 · 나머지. 모양이 다르면 통째로 값. */
export function splitBalance(text: string): { label: string; value: string; rest: string } {
  const [first, ...rest] = text.split(" · ");
  const m = /^(사용 가능 (?:잔고|증거금)) (.+)$/.exec(first ?? "");
  if (!m) return { label: "잔고", value: text, rest: "" };
  return { label: m[1], value: m[2], rest: rest.join(" · ") };
}

interface Props {
  slot: SlotCard;
  /** 키 칸을 열었는지 — 설정 화면이 들고 있음([설정에서 입력]으로 데려오면 그 칸을 엶). */
  open: boolean;
  onToggle(open: boolean): void;
}

export function SlotCardView({ slot, open, onToggle }: Props) {
  const { act, state } = useRunnerContext();
  const filled = slot.accessFilled && slot.secretFilled;
  const id = `settings-slot-${slot.slot}`;
  const showChecks = slot.status === "connected" || slot.status === "blocked";
  const fields = ["access", "secret"] as const;
  const tag = PRACTICE_TAG[slot.slot];
  const binanceNote = BINANCE_NOTES[slot.slot];
  const warn = noteToneOf(slot) === "warn";
  const bodyId = `${id}-body`;
  const blocked = slot.status === "blocked";
  // 막힌 이유 — Rust가 준 안내가 없으면 빨간 칸의 이유(+ 허용 IP면 이 기기 주소)로.
  const badKey = (["auth", "ip", "order"] as const).find((k) => slot.checks[k] === "bad");
  const ip = state.settings.publicIp.status === "ok" ? state.settings.publicIp.address : "";
  const why =
    slot.note && warn
      ? slot.note
      : blocked && badKey
        ? `${BAD_TEXT[badKey]}${badKey === "ip" && ip ? ` — 이 기기 주소 ${ip}` : ""}`
        : "";
  const balance = slot.status === "connected" && slot.balance ? splitBalance(slot.balance) : null;
  const short = slot.status === "connected" && warn;
  const wizard = slot.wizardAvailable && (
    <button
      type="button"
      className={`btn btn-sm ${blocked ? "btn-primary" : "btn-ghost"} slot-wizard`}
      onClick={() => act("wizard_open", { exchange: slot.exchange, slot: slot.slot })}
    >
      <Wand2 size={13} aria-hidden="true" />
      연결 도우미
    </button>
  );

  return (
    <article
      id={id}
      className={`slot-card status-${slot.status} ${open ? "is-open" : ""} ${tag ? "is-practice" : ""}`}
      tabIndex={-1}
      aria-labelledby={`${id}-title`}
    >
      <span className="slot-logo">
        <ExchangeLogo exchange={slot.exchange} size={tag ? 28 : 34} />
        {tag && (
          <span className="slot-test" aria-hidden="true">
            {tag}
          </span>
        )}
      </span>
      <div className="slot-main">
        <div className="slot-head">
          <h4 id={`${id}-title`} className="slot-title">
            {slot.label}
          </h4>
          <span className={`slot-badge tone-${badgeTone(slot)}`} aria-live="polite">
            {slot.status === "checking" && <LoaderCircle size={12} className="spin" aria-hidden="true" />}
            {slot.headline}
          </span>
        </div>
        {showChecks && <CheckStrip checks={slot.checks} />}
        {why && (
          <p className={`slot-why ${blocked ? "is-bad" : ""}`}>
            <TriangleAlert size={13} aria-hidden="true" />
            {why}
          </p>
        )}
      </div>
      <div className="slot-side">
        {balance ? (
          <div className={`slot-balance ${short ? "is-short" : ""}`}>
            <span className="slot-balance-label">{balance.label}</span>
            <span className="slot-balance-value num">{balance.value}</span>
            <span className="slot-balance-time num">
              {slot.checkedAt > 0 && `${formatClock(slot.checkedAt)} 확인`}
              {balance.rest && `${slot.checkedAt > 0 ? " · " : ""}${balance.rest}`}
            </span>
          </div>
        ) : (
          !open && slot.status !== "connected" && wizard
        )}
      </div>
      <button
        type="button"
        className={`icon-btn slot-toggle ${open ? "is-open" : ""}`}
        aria-expanded={open}
        aria-controls={bodyId}
        aria-label={`${slot.label} 키 칸 ${open ? "접기" : "펼치기"}`}
        title={open ? "키 칸 접기" : "키 칸 열기"}
        onClick={() => onToggle(!open)}
      >
        <ChevronDown size={16} strokeWidth={2.4} aria-hidden="true" />
      </button>

      <div id={bodyId} className="slot-body" hidden={!open}>
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
        <div className="slot-actions">
          {showChecks && (
            <button
              type="button"
              className="link-btn slot-recheck"
              onClick={() => act("check_slot", { slot: slot.slot })}
              disabled={!filled}
            >
              <RefreshCw size={13} aria-hidden="true" /> 다시 확인
            </button>
          )}
          {slot.status === "unchecked" && filled && (
            <button type="button" className="link-btn" onClick={() => act("check_slot", { slot: slot.slot })}>
              <RefreshCw size={13} aria-hidden="true" /> 지금 확인
            </button>
          )}
          {wizard}
        </div>
      </div>
    </article>
  );
}
