// 정보 — 문의할 때 알려 줄 사용 환경. 앱 버전(업데이트를 한 줄로 합침)·서버 연동 레벨·OS·공인 IP와
// [문의용 정보 복사]. 서버 주소는 기본 운영 서버가 아니거나 개발자 로그를 켰을 때만 보여 줌.
import { Check, ClipboardCopy, LoaderCircle, RefreshCw } from "lucide-react";
import { useEffect, useState } from "react";
import { useRunnerContext } from "../../state/context";
import { Section } from "./Section";
import { UpdatePanel } from "./UpdatePanel";

function PublicIp() {
  const { state, act } = useRunnerContext();
  const ip = state.settings.publicIp;
  // 정보 화면을 열 때 한 번 조회(120초 안의 결과가 있으면 Rust가 다시 묻지 않음).
  useEffect(() => {
    act("refresh_public_ip", { force: false });
  }, [act]);
  const text =
    ip.status === "ok"
      ? ip.address
      : ip.status === "loading"
        ? "확인하는 중…"
        : ip.status === "error"
          ? "확인하지 못했어요"
          : "확인 전";
  return (
    <dd className="about-value">
      <span className={ip.status === "ok" ? "num" : "muted"}>{text}</span>
      {ip.status !== "loading" && (
        <button
          type="button"
          className="icon-btn icon-btn-sm"
          aria-label="공인 IP 다시 확인"
          title="공인 IP 다시 확인"
          onClick={() => act("refresh_public_ip", { force: true })}
        >
          <RefreshCw size={13} aria-hidden="true" />
        </button>
      )}
      {ip.status === "loading" && <LoaderCircle size={13} className="spin muted" aria-hidden="true" />}
    </dd>
  );
}

function CopySupport() {
  const { ok } = useRunnerContext();
  const [copied, setCopied] = useState(false);
  useEffect(() => {
    if (!copied) return;
    const t = setTimeout(() => setCopied(false), 2000);
    return () => clearTimeout(t);
  }, [copied]);
  return (
    <div className="about-copy">
      <p className="muted">문의할 때 이 정보를 붙여 넣어 주세요. 거래소 키·회원 키 값은 들어가지 않아요.</p>
      <button
        type="button"
        className="btn btn-ghost btn-sm"
        onClick={async () => (await ok("copy_support_info")) && setCopied(true)}
      >
        {copied ? <Check size={14} aria-hidden="true" /> : <ClipboardCopy size={14} aria-hidden="true" />}
        {copied ? "복사했어요" : "문의용 정보 복사"}
      </button>
    </div>
  );
}

export function AboutSection() {
  const { state } = useRunnerContext();
  const s = state.settings;
  const showServer = s.serverCustom || s.showDevLog;
  return (
    <Section id="about" title="정보">
      <div className="plain-card about-card">
        <UpdatePanel />
        <dl className="about-table">
          <div>
            <dt>서버 연동 레벨</dt>
            <dd className="num" title="서버가 기능을 열 때 확인하는 번호예요. 앱 버전 가운데 숫자와 같아요.">
              {s.serverLinkLevel}
            </dd>
          </div>
          <div>
            <dt>OS</dt>
            <dd>{s.os}</dd>
          </div>
          <div>
            <dt>공인 IP</dt>
            <PublicIp />
          </div>
          {showServer && (
            <div>
              <dt>서버 주소</dt>
              <dd>
                <code className="path">{s.serverBase}</code>
                {s.serverCustom && <span className="about-note">기본 서버가 아닌 곳에 연결돼 있어요.</span>}
              </dd>
            </div>
          )}
        </dl>
        <CopySupport />
      </div>
    </Section>
  );
}
