// 상태에 따라 바뀌는 단추 글자([기록 복사] ↔ [복사했어요] · 접기 ↔ 펼치기 등) — 후보를 모두 같은 칸에 겹쳐 두고 지금 것만 보임.
// 단추 너비가 가장 긴 후보에 맞춰 고정되어, 글자가 바뀌어도 단추 크기 · 옆 요소 자리가 그대로.
// 안 보이는 후보는 visibility: hidden(base.css .swap-label) + aria-hidden 이라 화면 읽기 프로그램 · 단추 이름에 들어가지 않음.
import type { ReactNode } from "react";

export function SwapLabel({ items, active }: { items: ReactNode[]; active: number }) {
  return (
    <span className="swap-label">
      {items.map((item, i) => (
        <span key={i} className={i === active ? undefined : "is-off"} aria-hidden={i === active ? undefined : true}>
          {item}
        </span>
      ))}
    </span>
  );
}
