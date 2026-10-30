// 돈·안전 설정 칸 — 투자 금액·한 번에 최대·손절·하루 최대 손실·최대 보유(있는 것만).
// 이름·값 낱말은 Rust가 준 그대로(riskItems — 한 줄 요약 risk와 같은 말). 시작 전 화면과 실전 시작 확인 시트가 함께 씀.
// - 칸은 한 줄에 다 들어가면 한 줄, 모자라면 다음 줄로(남는 칸 없이 줄마다 꽉 채움).
import type { ReactNode } from "react";
import { formatAmount } from "../../state/format";
import type { MacroInfo } from "../../types";

interface Props {
  macro: MacroInfo;
  /** 투자 금액 칸 끝에 붙일 것(바꾸기·원본으로 단추). */
  capitalExtra?: ReactNode;
  /** 투자 금액을 바꾸는 중이면 값 대신 넣을 입력 칸 — 그 칸이 한 줄을 다 씀. */
  capitalEditor?: ReactNode;
  /** 시트처럼 좁은 곳 — 두 칸씩. */
  compact?: boolean;
}

export function MoneyCells({ macro, capitalExtra, capitalEditor, compact = false }: Props) {
  const q = macro.quote;
  return (
    <dl className={`money-cells ${compact ? "is-compact" : ""}`}>
      {macro.capital !== null && (
        <div className={`money-cell is-capital ${capitalEditor ? "is-editing" : ""}`}>
          <dt>투자 금액</dt>
          {capitalEditor ? (
            <dd>{capitalEditor}</dd>
          ) : (
            <dd>
              <span className="num">{formatAmount(macro.capital, q)}</span>
              {macro.capitalOriginal !== null && (
                <span className="money-original num">원본 {formatAmount(macro.capitalOriginal, q)}</span>
              )}
              {capitalExtra}
            </dd>
          )}
        </div>
      )}
      <div className="money-cell">
        <dt>한 번에 최대</dt>
        <dd className="num">{formatAmount(macro.orderCap, q)}</dd>
      </div>
      {macro.riskItems.map((r) => (
        <div key={r.label} className="money-cell">
          <dt>{r.label}</dt>
          <dd className="num">{r.value}</dd>
        </div>
      ))}
    </dl>
  );
}
