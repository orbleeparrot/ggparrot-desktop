// 실행 목록이 비었을 때 — 껄무새가 맞이하고 '이 순서로 실행해요' 세 단계(매크로 넣기 → 키 연결 → 시작).
// - 단계 이름 · 설명은 그대로 두고 번호를 세로 선으로 이음. 지금 할 단계(1)만 노란 번호 · 흰 칸.
// - 단계마다 그 자리에서 할 일: 1 [매크로 추가] · [웹에서 고르기], 2 회원 키 · 거래소 키 — 된 것은 체크 칩, 안 된 것은 단추.
// - 글자 규칙: 크기 22 · 15 · 13, 굵기 500 · 700, 글자색은 진한 색 · 회색 두 가지. 노랑 · 초록은 번호 · 체크 모양에만.
// - 껄무새는 단계 묶음의 세로 가운데. 창 크기에 따라 그림 크기 · 간격만 바뀜(run.css), 묶음은 창의 위아래 가운데.
import { Check, ChevronRight, ExternalLink, Plus } from "lucide-react";
import { useRunnerContext } from "../../state/context";

export function EmptyState() {
  const { openLibrary, openSettings, act, state } = useRunnerContext();
  const member = state.settings.memberKeyFilled;
  const exchange = state.settings.slots.some((s) => s.status === "connected");
  return (
    <section className="empty-state" aria-labelledby="empty-title">
      <div className="first-run">
        <img className="first-hero" src="/brand/ggparrot-sunglasses-hero-v2.svg" alt="" aria-hidden="true" />
        <div className="first-body">
          <h2 id="empty-title" className="empty-title">
            실행할 매크로를 추가하세요
          </h2>
          <p className="empty-lead">웹에서 받은 매크로 파일로 시작해요. 여러 개를 동시에 돌릴 수 있어요.</p>
          <h3 className="first-flow-title">이 순서로 실행해요</h3>
          <ol className="first-steps">
            <li className="first-step is-now">
              <span className="first-step-no" aria-hidden="true">
                1
              </span>
              <div className="first-step-body">
                <strong>매크로 파일 넣기</strong>
                <span>매크로 폴더에 .ggm.json 파일을 넣거나 웹에서 [실행기 열기]를 누르세요.</span>
                <div className="first-step-actions">
                  <button type="button" className="btn btn-primary" onClick={openLibrary}>
                    <Plus size={15} aria-hidden="true" />
                    매크로 추가
                  </button>
                  <button type="button" className="btn btn-ghost" onClick={() => void act("open_web")}>
                    <ExternalLink size={14} aria-hidden="true" />
                    웹에서 고르기
                  </button>
                </div>
              </div>
            </li>
            <li className="first-step">
              <span className="first-step-no" aria-hidden="true">
                2
              </span>
              <div className="first-step-body">
                <strong>설정에서 키 연결</strong>
                <span>모의는 거래소 키 없이도 돼요(회원 키는 필요해요).</span>
                <div className="first-step-actions">
                  {member ? (
                    <span className="first-chip">
                      <Check size={11} strokeWidth={3.2} aria-hidden="true" />
                      회원 키 입력됨
                    </span>
                  ) : (
                    <button
                      type="button"
                      className="btn btn-ghost btn-sm"
                      onClick={() => openSettings({ kind: "member" })}
                    >
                      회원 키 입력하기
                      <ChevronRight size={14} aria-hidden="true" />
                    </button>
                  )}
                  {exchange ? (
                    <span className="first-chip">
                      <Check size={11} strokeWidth={3.2} aria-hidden="true" />
                      거래소 키 연결됨
                    </span>
                  ) : (
                    <button
                      type="button"
                      className="btn btn-ghost btn-sm"
                      onClick={() => openSettings({ kind: "section", section: "exchanges" })}
                    >
                      거래소 연결 열기
                      <ChevronRight size={14} aria-hidden="true" />
                    </button>
                  )}
                </div>
              </div>
            </li>
            <li className="first-step">
              <span className="first-step-no" aria-hidden="true">
                3
              </span>
              <div className="first-step-body">
                <strong>시작</strong>
                <span>실전은 한 번 더 확인한 뒤 시작해요.</span>
              </div>
            </li>
          </ol>
        </div>
      </div>
    </section>
  );
}
