// 키 확인 결과 — 키 인증·허용 IP·주문 권한. ok ✓ 초록·bad ✕ 빨강·wait … 회색·na 확인 불가.
// notes가 있으면(바이낸스 '필요 없음'·'IP 제한 없음' 등) bad·na의 설명 글자를 그것으로 바꿈.
// - CheckRows: 세 줄(연결 도우미 결과 — 자리가 넉넉하고 설명이 중요)
// - CheckChips: 한 줄 칩(설정의 거래소 연결 카드 — 세로 자리를 아낌, 설명은 칩의 title)
import { Check, LoaderCircle, Minus, X } from "lucide-react";
import type { Check as CheckState, WizardState } from "../types";

const ROWS = [
  ["auth", "키 인증"],
  ["ip", "허용 IP"],
  ["order", "주문 권한"],
] as const;

const BAD_TEXT = { auth: "키가 맞지 않음", ip: "등록된 IP와 다름", order: "주문하기 권한 없음" } as const;

function Icon({ state, spinning }: { state: CheckState; spinning: boolean }) {
  if (spinning) return <LoaderCircle size={16} className="spin" />;
  if (state === "ok") return <Check size={13} strokeWidth={3} />;
  if (state === "bad") return <X size={13} strokeWidth={3} />;
  if (state === "na") return <Minus size={13} strokeWidth={3} />;
  return <span className="wz-wait">…</span>;
}

function explain(key: (typeof ROWS)[number][0], state: CheckState, notes?: WizardState["notes"]): string {
  if (state === "bad") return notes?.[key] || BAD_TEXT[key];
  if (state === "na") return notes?.[key] || "확인 불가";
  if (state === "wait") return "대기";
  return "정상";
}

export function CheckChips({ checks, notes }: { checks: WizardState["checks"]; notes?: WizardState["notes"] }) {
  return (
    <ul className="check-chips" aria-label="키 확인 결과">
      {ROWS.map(([key, label]) => {
        const state = checks[key];
        const why = explain(key, state, notes);
        return (
          <li key={key} className={`check-chip check-${state}`} title={`${label}: ${why}`}>
            <span className="check-chip-icon" aria-hidden="true">
              <Icon state={state} spinning={false} />
            </span>
            {label}
            <span className="sr-only">: {why}</span>
          </li>
        );
      })}
    </ul>
  );
}

export function CheckRows({
  checks,
  notes,
  ip,
  spinning = false,
}: {
  checks: WizardState["checks"];
  notes?: WizardState["notes"];
  ip: string;
  spinning?: boolean;
}) {
  return (
    <ul className="wz-rows">
      {ROWS.map(([key, label]) => {
        const state = checks[key];
        let right = "";
        if (spinning) right = "확인 중";
        else if (state === "bad") right = notes?.[key] || BAD_TEXT[key];
        else if (state === "na") right = notes?.[key] || "확인 불가";
        else if (state === "wait") right = "대기";
        else if (key === "ip" && ip) right = ip;
        return (
          <li key={key} className={`wz-row check-${spinning ? "spin" : state}`}>
            <span className="wz-row-icon" aria-hidden="true">
              <Icon state={state} spinning={spinning} />
            </span>
            <span className="wz-row-label">{label}</span>
            <span className={`wz-row-value ${key === "ip" && state === "ok" ? "num strong" : ""}`}>
              {right || (state === "ok" && !spinning ? <span className="sr-only">정상</span> : null)}
            </span>
          </li>
        );
      })}
    </ul>
  );
}
