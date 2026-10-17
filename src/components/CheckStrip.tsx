// 세 확인(키 인증 → 허용 IP → 주문 권한)을 이어 붙인 한 줄 — 칸마다 원 아이콘 + 이름.
// ok ✓ 초록 원 · bad ✕ 빨간 칸 · wait … 빈 원(아직 못 봄) · na – 빈 원(확인할 길이 없음).
// 설명(bad·na의 이유)은 칸의 풍선 도움말과 가려진 글자로 — 줄을 늘리지 않음.
import { Check, Minus, X } from "lucide-react";
import type { Check as CheckState } from "../types";

const ROWS = [
  ["auth", "키 인증"],
  ["ip", "허용 IP"],
  ["order", "주문 권한"],
] as const;

const BAD_TEXT = { auth: "키가 맞지 않음", ip: "등록된 IP와 다름", order: "주문하기 권한 없음" } as const;

type Key = (typeof ROWS)[number][0];
type Checks = Record<Key, CheckState>;
type Notes = Partial<Record<Key, string>>;

function why(key: Key, state: CheckState, notes?: Notes): string {
  if (state === "bad") return notes?.[key] || BAD_TEXT[key];
  if (state === "na") return notes?.[key] || "확인 불가";
  if (state === "wait") return "대기";
  return "정상";
}

export function CheckStrip({ checks, notes, small = false }: { checks: Checks; notes?: Notes; small?: boolean }) {
  return (
    <ul className={`check-strip ${small ? "is-small" : ""}`} aria-label="키 확인 결과">
      {ROWS.map(([key, label]) => {
        const state = checks[key];
        const text = why(key, state, notes);
        return (
          <li key={key} className={`check-cell check-${state}`} title={`${label}: ${text}`}>
            <span className="check-cell-icon" aria-hidden="true">
              {state === "ok" && <Check size={10} strokeWidth={3.6} />}
              {state === "bad" && <X size={10} strokeWidth={3.6} />}
              {state === "na" && <Minus size={10} strokeWidth={3.6} />}
              {state === "wait" && "…"}
            </span>
            {label}
            <span className="sr-only">: {text}</span>
          </li>
        );
      })}
    </ul>
  );
}
