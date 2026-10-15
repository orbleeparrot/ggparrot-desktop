// 정보 — 앱 버전·서버 연동(계약) 버전·서버 주소(읽기 전용)·업데이트.
import { useRunnerContext } from "../../state/context";
import { APP_NAME } from "../Header";
import { Section } from "./Section";
import { UpdatePanel } from "./UpdatePanel";

export function AboutSection() {
  const { state } = useRunnerContext();
  return (
    <Section id="about" title="정보">
      <div className="plain-card">
        <dl className="about-table">
          <div>
            <dt>앱</dt>
            <dd>
              {APP_NAME} <span className="num">v{state.version}</span>
            </dd>
          </div>
          <div>
            <dt>서버 연동 버전</dt>
            <dd className="num">{state.settings.protocolVersion}</dd>
          </div>
          <div>
            <dt>서버 주소</dt>
            <dd>
              <code className="path">{state.settings.serverBase}</code>
            </dd>
          </div>
        </dl>
      </div>
      <h4 className="settings-sub">업데이트</h4>
      <div className="plain-card">
        <UpdatePanel />
      </div>
    </Section>
  );
}
