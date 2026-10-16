// 매크로 파일 알림 한 줄 — 지원하지 않는 거래소·웹 원본과 다른 설정·서명 없는 예전 파일.
import { TriangleAlert } from "lucide-react";
import type { RunSlot } from "../../types";

/**
 * blockerShown: 상세가 막힌 이유 상자를 보이는 중인지 여부.
 * - 지원하지 않는 거래소면 막힌 이유가 곧 같은 문장이라 상자가 보일 때는 다시 적지 않음.
 */
export function MacroNotes({ slot, blockerShown }: { slot: RunSlot; blockerShown: boolean }) {
  const m = slot.macro;
  if (!m.exchange) {
    if (blockerShown) return null;
    return (
      <p className="note note-danger">
        이 앱이 지원하지 않는 거래소({m.exchangeName || "알 수 없음"})예요. 바이낸스 · 업비트 · 빗썸 매크로만 실행할 수
        있어요.
      </p>
    );
  }
  if (m.signature === "modified")
    return (
      <p className="note note-warn">
        <TriangleAlert size={14} aria-hidden="true" />이 매크로는 웹에서 받은 원본과 설정이 달라요(투자 금액 등). 이
        기기에서 바꾼 설정으로 실행돼요.
      </p>
    );
  if (m.signature === "unsigned")
    return <p className="note note-muted">서명이 없는 예전 파일이에요. 실행은 되지만 원본인지 확인할 수 없어요.</p>;
  return null;
}
