// 실행 목록이 비었을 때 — 첫 실행 안내(매크로 넣기 → 키 연결 → 시작)와 [매크로 추가].
import { Plus } from "lucide-react";
import { useRunnerContext } from "../../state/context";

function Illustration() {
  return (
    <svg className="empty-art" viewBox="0 0 200 120" aria-hidden="true">
      <rect x="44" y="18" width="112" height="30" rx="8" className="empty-art-card is-back" />
      <rect x="32" y="34" width="136" height="36" rx="9" className="empty-art-card is-mid" />
      <rect x="20" y="52" width="160" height="50" rx="10" className="empty-art-card" />
      <circle cx="44" cy="77" r="9" className="empty-art-dot" />
      <rect x="60" y="70" width="58" height="6" rx="3" className="empty-art-line" />
      <rect x="60" y="81" width="36" height="5" rx="2.5" className="empty-art-line is-faint" />
      <rect x="132" y="68" width="36" height="18" rx="6" className="empty-art-btn" />
      <path d="M146 72.5 L153 77 L146 81.5 Z" className="empty-art-play" />
    </svg>
  );
}

export function EmptyState() {
  const { openLibrary, openSettings } = useRunnerContext();
  return (
    <section className="empty-state" aria-labelledby="empty-title">
      <Illustration />
      <h2 id="empty-title" className="empty-title">
        실행할 매크로를 추가하세요
      </h2>
      <p className="empty-lead">웹에서 받은 매크로 파일로 시작해요. 여러 개를 동시에 돌릴 수 있어요.</p>
      <ol className="first-steps">
        <li>
          <span className="first-step-no" aria-hidden="true">
            1
          </span>
          <span>
            <strong>매크로 파일 넣기</strong>
            <span>매크로 폴더에 .ggm.json 파일을 넣거나 웹에서 [실행기 열기]를 누르세요.</span>
          </span>
        </li>
        <li>
          <span className="first-step-no" aria-hidden="true">
            2
          </span>
          <span>
            <strong>설정에서 키 연결</strong>
            <span>
              모의는 거래소 키 없이도 돼요(회원 키는 필요해요).{" "}
              <button
                type="button"
                className="link-btn"
                onClick={() => openSettings({ kind: "section", section: "exchanges" })}
              >
                거래소 연결 열기
              </button>
            </span>
          </span>
        </li>
        <li>
          <span className="first-step-no" aria-hidden="true">
            3
          </span>
          <span>
            <strong>시작</strong>
            <span>실전은 한 번 더 확인한 뒤 시작해요.</span>
          </span>
        </li>
      </ol>
      <button type="button" className="btn btn-primary btn-lg" onClick={openLibrary}>
        <Plus size={16} aria-hidden="true" />
        매크로 추가
      </button>
    </section>
  );
}
