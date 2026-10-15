// 상세 머리줄 — 뒤로(한 열)·거래소 로고·종목·방향·거래소, 출처·전략·시장 요약(파일 이름은 마우스를 올리면), 웹 실행 기록·거래소 종목 화면 단추.
import { ArrowLeft, ExternalLink, History } from "lucide-react";
import { useRunnerContext } from "../../state/context";
import { MARKET_LABELS, SIDE_LABELS } from "../../state/runs";
import type { RunSlot } from "../../types";
import { ExchangeLogo } from "../ExchangeLogo";

export function DetailHeader({ slot, onBack }: { slot: RunSlot; onBack?: () => void }) {
  const { ok } = useRunnerContext();
  const m = slot.macro;
  const facts = [
    // 서버가 알려 준 출처(서명 확인 등)가 있으면 그게 더 정확함.
    slot.run?.macroOrigin || (m.source === "web" ? "웹 매크로" : "파일 매크로"),
    m.strategy,
    MARKET_LABELS[m.market],
    m.leverage > 1 ? `${m.leverage}배` : "",
  ].filter(Boolean);

  return (
    <div className="detail-head">
      {onBack && (
        <button type="button" className="icon-btn detail-back" aria-label="내 매크로 목록으로" onClick={onBack}>
          <ArrowLeft size={18} />
        </button>
      )}
      <ExchangeLogo exchange={m.exchange} size={30} />
      <div className="detail-name">
        <h2 id="detail-title" className="detail-title">
          <span className="num">{m.symbol}</span>
          <span className={`side side-${m.side}`}>{SIDE_LABELS[m.side]}</span>
          <span className="detail-exchange">{m.exchangeLabel || m.exchangeName}</span>
        </h2>
        <p className="detail-sub" title={`매크로 파일: ${m.title}`}>
          {facts.join(" · ")}
        </p>
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
              <History size={13} aria-hidden="true" />
              웹에서 실행 기록 보기
            </button>
          )}
          {slot.marketPage && (
            <button
              type="button"
              className="btn btn-ghost btn-sm"
              title={`${m.symbol} 시세를 브라우저로 열어요`}
              onClick={() => void ok("open_market_page", { runId: slot.id })}
            >
              {slot.marketPage}
              <ExternalLink size={13} aria-hidden="true" />
            </button>
          )}
        </div>
      )}
    </div>
  );
}
