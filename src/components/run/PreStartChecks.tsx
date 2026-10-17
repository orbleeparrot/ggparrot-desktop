// 시작 전 확인 — 가짜 그래프 대신, 시작 전에 알아야 할 것만 앱이 아는 값으로.
// - 막힌 이유(키 · 회원 키 말고): 지원하지 않는 매크로 · 같은 종목 실행 중 · 웹 연결 중 — readiness.blocker 그대로
// - 회원 키: 설정의 '입력됨 · 입력 전'과 같은 값(memberKeyFilled)
// - 거래소 키(모의 빼고): 고른 모드의 칸(실전 · 테스트넷 · 데모가 따로) — 칸 상태 글자 · 세 확인
// - 바로 사지 않아요(기다리는 매크로, Rust is_waiting_macro): 평소 기다린 시간은 서버에 미리 물음(답이 없으면 그 부분만 빠짐)
import { Check, Hourglass, TriangleAlert } from "lucide-react";
import type { ReactNode } from "react";
import { useRunnerContext } from "../../state/context";
import type { RunSlot, SlotCard } from "../../types";
import { CheckStrip } from "../CheckStrip";
import { rough } from "./EntryWait";
import { useEntryPreview } from "./useEntryPreview";

type Mark = "ok" | "warn" | "todo" | "wait";

function Row({ mark, title, sub, right }: { mark: Mark; title: string; sub?: ReactNode; right?: ReactNode }) {
  return (
    <li className={`pre-row is-${mark}`}>
      <span className="pre-mark" aria-hidden="true">
        {mark === "ok" && <Check size={12} strokeWidth={3.4} />}
        {mark === "warn" && <TriangleAlert size={12} strokeWidth={2.6} />}
        {mark === "wait" && <Hourglass size={12} strokeWidth={2.4} />}
      </span>
      <span className="pre-main">
        <span className="pre-title">{title}</span>
        {sub && <span className="pre-sub">{sub}</span>}
      </span>
      {right && <span className="pre-right">{right}</span>}
    </li>
  );
}

/** 키 칸 상태 → 줄 표시. 머리 글자("✓ 연결됨"·"● 확인 전")의 앞 기호는 빼고 표시로 대신함. */
function keyMark(card: SlotCard): Mark {
  if (card.status === "connected") return "ok";
  if (card.status === "blocked") return "warn";
  return "todo";
}

export function PreStartChecks({ slot }: { slot: RunSlot }) {
  const { state } = useRunnerContext();
  const preview = useEntryPreview(slot);
  const r = slot.readiness;
  const other = !r.canStart && !r.needs ? r.blocker : "";
  const member = state.settings.memberKeyFilled;
  const card =
    slot.mode !== "mock" && slot.keySlot ? state.settings.slots.find((s) => s.slot === slot.keySlot) : undefined;
  const typical = preview?.typical ?? null;

  return (
    <section className="pre-start" aria-label="시작 전 확인">
      <h3 className="eyebrow">시작 전 확인</h3>
      <ul className="pre-rows">
        {other && <Row mark="warn" title="지금은 시작할 수 없어요" sub={other} />}
        <Row mark={member ? "ok" : "todo"} title="회원 키" right={member ? "입력됨" : "입력 전"} />
        {card && (
          <Row
            mark={keyMark(card)}
            title={`${card.label} 거래소 키`}
            sub={
              card.status === "connected" || card.status === "blocked" ? (
                <CheckStrip checks={card.checks} small />
              ) : undefined
            }
            right={card.headline.replace(/^[✓●]\s*/, "")}
          />
        )}
        {slot.waits && (
          <Row
            mark="wait"
            title="바로 사지 않아요"
            sub={
              typical?.avg_ms
                ? `조건이 맞을 때만 사요 · 평소 기다린 시간 ${rough(typical.avg_ms)}`
                : "조건이 맞을 때만 사요"
            }
          />
        )}
      </ul>
    </section>
  );
}
