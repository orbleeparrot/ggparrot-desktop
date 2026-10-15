// 껄무새 계정 — 회원 키 한 칸.
import { useRunnerContext } from "../../state/context";
import { SecretField } from "../SecretField";
import { InUseNote } from "./InUseNote";
import { Section } from "./Section";

export function MemberSection() {
  const { state, act } = useRunnerContext();
  const s = state.settings;
  return (
    <Section id="account" title="껄무새 계정" lead="웹에서 [실행기 열기]를 누르면 회원 키가 저절로 채워져요.">
      <div id="settings-member" className="plain-card" tabIndex={-1}>
        <div className="plain-card-head">
          <h4 className="slot-title">회원 키</h4>
          <span className={`slot-badge tone-${s.memberKeyFilled ? "ok" : "muted"}`}>
            <span aria-hidden="true">{s.memberKeyFilled ? "✓ " : "● "}</span>
            {s.memberKeyFilled ? "입력됨" : "입력 전"}
          </span>
        </div>
        <SecretField
          id="member-key"
          label="회원 키"
          filled={s.memberKeyFilled}
          onCommit={(value) => act("set_member_key", { value })}
          onReveal={() => act("reveal_member_key")}
        />
        <InUseNote count={s.memberInUseBy} />
        {s.memberKeyPending && (
          <div className="note note-warn member-pending" role="alert">
            <span>
              {s.memberKeyFilled
                ? `웹 연결에서 지금과 다른 껄무새 계정의 회원 키(${s.memberKeyPending})를 받았어요. 내 계정이 맞을 때만 바꾸세요. 바꾸기 전까지는 지금 회원 키를 써요.`
                : `웹 연결에서 껄무새 계정의 회원 키(${s.memberKeyPending})를 받았어요. 내 계정이 맞을 때만 쓰세요. 쓰기 전에는 매크로를 시작할 수 없어요.`}
            </span>
            <span className="member-pending-actions">
              <button
                type="button"
                className="btn btn-ghost btn-sm"
                onClick={() => act("resolve_member_key", { accept: false })}
              >
                {s.memberKeyFilled ? "그대로 두기" : "쓰지 않기"}
              </button>
              <button
                type="button"
                className="btn btn-primary btn-sm"
                onClick={() => act("resolve_member_key", { accept: true })}
              >
                {s.memberKeyFilled ? "이 키로 바꾸기" : "이 키 쓰기"}
              </button>
            </span>
          </div>
        )}
      </div>
    </Section>
  );
}
