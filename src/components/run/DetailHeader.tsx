// 상세 머리줄 — 뒤로(한 열)·거래소 로고·종목(실전·테스트넷이면 모드 배지), 아래 한 줄은 줄글 대신 칩
// (거래소 시장 · 방향 배수 · 전략 · 봉 간격 · 출처). 오른쪽 끝은 웹 실행 기록·거래소 종목 화면 단추.
import { ArrowLeft, Check, ExternalLink } from "lucide-react";
import { useRunnerContext } from "../../state/context";
import { MARKET_LABELS, MODE_LABELS, SIDE_LABELS } from "../../state/runs";
import type { MacroInfo, RunSlot } from "../../types";
import { ExchangeLogo } from "../ExchangeLogo";

/** 출처 칩 — 서버가 알려 준 출처(서명 확인 등)가 있으면 그게 더 정확함. 서명된 원본·웹 매크로는 체크. */
function originChip(slot: RunSlot): { text: string; tone: "ok" | "warn" | "" } {
  const m = slot.macro;
  const text = slot.run?.macroOrigin || (m.source === "web" ? "웹 매크로" : "파일 매크로");
  const tone: Record<MacroInfo["signature"], "ok" | "warn" | ""> = {
    signed: "ok",
    web: "ok",
    modified: "warn",
    unsigned: "",
  };
  return { text, tone: tone[m.signature] };
}

/** 거래소 종목 화면 단추 글자 — "업비트에서 보기" → "업비트"(전체 문장은 풍선 도움말). */
function shortMarket(label: string): string {
  return label.replace(/에서 보기$/, "").trim() || label;
}

export function DetailHeader({ slot, onBack }: { slot: RunSlot; onBack?: () => void }) {
  const { ok } = useRunnerContext();
  const m = slot.macro;
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
      <ExchangeLogo exchange={m.exchange} size={34} />
      <div className="detail-name">
        <h2 id="detail-title" className="detail-title">
          <span className="num">{m.symbol}</span>
          {slot.mode !== "mock" && <span className={`mode-badge mode-${slot.mode}`}>{MODE_LABELS[slot.mode]}</span>}
        </h2>
        <ul className="detail-chips" aria-label="매크로 정보" title={`매크로 파일: ${m.title}`}>
          {chips.map((c) => (
            <li key={c} className="detail-chip">
              {c}
            </li>
          ))}
          <li className={`detail-chip ${origin.tone ? `tone-${origin.tone}` : ""}`}>
            {origin.tone === "ok" && <Check size={12} strokeWidth={3} aria-hidden="true" />}
            {origin.text}
          </li>
        </ul>
      </div>
      {(slot.marketPage || slot.webRunPage) && (
        <div className="detail-links">
          {slot.webRunPage && (
            <button
              type="button"
              className="btn btn-ghost btn-sm"
              title="껄무새 웹의 내 에이전트에서 이 실행 기록을 열어요"
              onClick={() => void ok("open_run_web_page", { runId: slot.id })}
            >
              실행 기록
              <ExternalLink size={13} aria-hidden="true" />
            </button>
          )}
          {slot.marketPage && (
            <button
              type="button"
              className="btn btn-ghost btn-sm"
              title={`${slot.marketPage} — ${m.symbol} 시세를 브라우저로 열어요`}
              onClick={() => void ok("open_market_page", { runId: slot.id })}
            >
              {shortMarket(slot.marketPage)}
              <ExternalLink size={13} aria-hidden="true" />
            </button>
          )}
        </div>
      )}
    </div>
  );
}
