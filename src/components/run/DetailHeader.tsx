// 상세 머리줄 — 뒤로(한 열)·코인 표시(코인 로고 + 모서리 거래소 로고)·종목(실전·테스트넷이면 모드 배지)·실행 중이면 상태("● 실행 중 06:02"),
// 아래 한 줄은 줄글 대신 칩(거래소 시장·방향 배수·전략·봉 간격·출처 주의). 실행 중이면 오른쪽 끝에 끝내는 단추.
// - 종목은 기초 자산(B3)만 크게 — 시장 쌍(KRW-B3)은 빼고, 거래소·현물/선물은 아래 칩이 말함(runs.ts coinTicker).
// - 제목(상세 영역의 이름)은 "업비트 B3"로 읽힘 — 거래소 이름은 제목 안 가려진 글자로, 코인 표시는 장식으로 둠(두 번 읽히지 않게).
// - 웹 실행 기록·거래소 종목 화면 단추는 앱 머리줄(Header)에 있음.
import type { ReactNode } from "react";
import { ArrowLeft } from "lucide-react";
import { useRunnerContext } from "../../state/context";
import { coinTicker, MARKET_LABELS, MODE_LABELS, SIDE_LABELS } from "../../state/runs";
import type { MacroInfo, RunSlot } from "../../types";
import { CoinMark } from "../CoinMark";

const MODIFIED = "수정된 파일";
const UNSIGNED = "서명 없는 파일";

/** 출처 칩 — 확인할 게 있을 때만(수정된 파일·서명 없는 파일). 웹 매크로·서명 확인된 원본은 칩 없음(2026-10-19:
 * '웹에서 바로 실행'이 늘 붙어 있어 정보가 없었음). 서버가 알려 준 출처(서명을 비밀키로 확인)가 있으면 그걸 따름. */
export function originChip(slot: RunSlot): { text: string; tone: "warn" | "" } | null {
  const server = slot.run?.macroOrigin || "";
  const signature: MacroInfo["signature"] = server
    ? server === MODIFIED
      ? "modified"
      : server === UNSIGNED
        ? "unsigned"
        : "signed"
    : slot.macro.signature;
  if (signature === "modified") return { text: MODIFIED, tone: "warn" };
  if (signature === "unsigned") return { text: UNSIGNED, tone: "" };
  return null;
}

interface Props {
  slot: RunSlot;
  onBack?: () => void;
  /** 제목 옆 상태(실행 중일 때 RunState). */
  runState?: ReactNode;
  /** 오른쪽 끝 단추(실행 중일 때 끝내는 단추). */
  actions?: ReactNode;
}

export function DetailHeader({ slot, onBack, runState, actions }: Props) {
  const { state } = useRunnerContext();
  const m = slot.macro;
  const ticker = coinTicker(m);
  const origin = originChip(slot);
  const chips = [
    `${m.exchangeLabel || m.exchangeName} ${MARKET_LABELS[m.market]}`,
    m.leverage > 1 ? `${SIDE_LABELS[m.side]} ${m.leverage}배` : SIDE_LABELS[m.side],
    m.strategy,
    m.interval ? `${m.interval} 봉` : "",
  ].filter(Boolean);

  return (
    <div className="detail-head">
      {onBack && (
        <button type="button" className="icon-btn detail-back" aria-label="내 매크로 목록으로" onClick={onBack}>
          <ArrowLeft size={18} />
        </button>
      )}
      <CoinMark
        base={ticker}
        exchange={m.exchange}
        exchangeLabel=""
        size={44}
        badge={18}
        serverBase={state.settings.serverBase}
      />
      <div className="detail-name">
        <div className="detail-title-row">
          <h2 id="detail-title" className="detail-title">
            <span className="sr-only">{m.exchangeLabel || m.exchangeName}</span>{" "}
            <span className="detail-ticker num">{ticker}</span>
            {slot.mode !== "mock" && <span className={`mode-badge mode-${slot.mode}`}>{MODE_LABELS[slot.mode]}</span>}
          </h2>
          {runState}
        </div>
        <ul className="detail-chips" aria-label="매크로 정보" title={`매크로 파일: ${m.title}`}>
          {chips.map((c) => (
            <li key={c} className="detail-chip">
              {c}
            </li>
          ))}
          {origin && <li className={`detail-chip ${origin.tone ? `tone-${origin.tone}` : ""}`}>{origin.text}</li>}
        </ul>
      </div>
      {actions && <div className="detail-actions">{actions}</div>}
    </div>
  );
}
