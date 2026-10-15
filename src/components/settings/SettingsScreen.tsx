// 설정 — 실행 화면을 바꿔 끼우는 전체 화면. Esc·[← 실행 화면으로]를 누르면 돌아감.
// target이 오면 그 칸으로 굴리고 첫 입력칸에 포커스를 둠([설정에서 입력]·업데이트 칩).
// 앱 기록의 한 줄(토스트 [자세히])은 기록을 읽은 뒤 AppLogSection이 직접 데려감.
import { ArrowLeft } from "lucide-react";
import { useEffect, useMemo, useRef } from "react";
import { keyStoreName } from "../../platform";
import { useRunnerContext } from "../../state/context";
import type { SettingsSection, SettingsTarget } from "../../state/runs";
import { AboutSection } from "./AboutSection";
import { AppLogSection } from "./AppLogSection";
import { MemberSection } from "./MemberSection";
import { DisplaySection } from "./DisplaySection";
import { goToSettings } from "./goTo";
import { LibrarySection } from "./LibrarySection";
import { Section } from "./Section";
import { SlotCardView } from "./SlotCardView";
import { StorageWarning } from "./StorageWarning";

const NAV: { id: SettingsSection; label: string }[] = [
  { id: "exchanges", label: "거래소 연결" },
  { id: "account", label: "껄무새 계정" },
  { id: "library", label: "매크로 폴더" },
  { id: "display", label: "창 · 화면" },
  { id: "about", label: "정보" },
  { id: "log", label: "앱 기록" },
];

const EXCHANGES_LEAD =
  "모의는 거래소 키 없이 시작할 수 있어요(회원 키는 필요해요). 테스트넷 · 실전은 해당 거래소의 API 키가 있어야 시작해요. 키는 언제든 바꿀 수 있어요.";

export function SettingsScreen({
  target,
  onBack,
}: {
  target: { target: SettingsTarget; seq: number } | null;
  onBack(): void;
}) {
  const { state } = useRunnerContext();
  const title = useRef<HTMLHeadingElement>(null);
  const logFocus = useMemo(
    () => (target?.target.kind === "log" ? { id: target.target.id, seq: target.seq } : null),
    [target],
  );

  useEffect(() => {
    if (!target) {
      title.current?.focus({ preventScroll: true });
      return;
    }
    if (target.target.kind === "log") return;
    const id = requestAnimationFrame(() => goToSettings(target.target));
    return () => cancelAnimationFrame(id);
  }, [target]);

  useEffect(() => {
    function onKey(e: KeyboardEvent) {
      if (e.key !== "Escape" || e.defaultPrevented || state.wizard) return;
      if (document.querySelector('[aria-modal="true"]')) return;
      onBack();
    }
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [onBack, state.wizard]);

  return (
    <div className="settings">
      <div className="settings-bar">
        <button type="button" className="btn btn-quiet" onClick={onBack}>
          <ArrowLeft size={16} aria-hidden="true" />
          실행 화면으로
        </button>
        <span className="settings-divider" aria-hidden="true" />
        <h2 ref={title} className="settings-title" tabIndex={-1}>
          설정
        </h2>
      </div>
      <div className="settings-body">
        <nav className="settings-nav" aria-label="설정 목차">
          {NAV.map((n) => (
            <button
              key={n.id}
              type="button"
              className="settings-nav-item"
              onClick={() => goToSettings({ kind: "section", section: n.id })}
            >
              {n.label}
            </button>
          ))}
        </nav>
        <div className="settings-content">
          <Section id="exchanges" title="거래소 연결" lead={EXCHANGES_LEAD}>
            <p className="settings-lead-note">
              {state.settings.storage.status === "unsupported"
                ? "이 OS에서는 키를 이번 실행 동안만 기억해요."
                : `기억한 키는 이 기기의 ${keyStoreName()}에 보관해요.`}
            </p>
            <StorageWarning />
            <div className="slot-grid">
              {state.settings.slots.map((s) => (
                <SlotCardView key={s.slot} slot={s} />
              ))}
            </div>
          </Section>
          <MemberSection />
          <LibrarySection />
          <DisplaySection />
          <AboutSection />
          <AppLogSection focus={logFocus} />
        </div>
      </div>
    </div>
  );
}
