// 아직 시작하지 않은 슬롯 — 무엇이 여기에 나타날지 미리 보여 주는 빈 상태.
import { Sparkline } from "../Sparkline";

const PREVIEW = [42, 44, 43, 47, 46, 50, 49, 53, 52, 51, 55, 58, 57, 61];

export function IdleCard() {
  return (
    <section className="idle-card" aria-label="실행 상태">
      <div className="idle-figure" aria-hidden="true">
        <span className="idle-pct num">+0.00%</span>
        <Sparkline points={PREVIEW} width={200} height={44} />
      </div>
      <p className="idle-text">시작하면 여기에 현재가와 손익이 보여요</p>
    </section>
  );
}
