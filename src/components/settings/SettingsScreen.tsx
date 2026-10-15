// 설정 — 실행 화면을 바꿔 끼우는 전체 화면. Esc·[← 실행 화면으로]를 누르면 돌아감.
// target이 오면 그 칸으로 굴리고 첫 입력칸에 포커스를 둠([설정에서 입력]·업데이트 칩).
import { ArrowLeft } from "lucide-react";
import { useEffect, useRef } from "react";
import { useRunnerContext } from "../../state/context";
import { settingsElementId, type SettingsSection, type SettingsTarget } from "../../state/runs";
import { reducedMotion } from "../focus";
import { AboutSection } from "./AboutSection";
import { MemberSection } from "./MemberSection";
import { DisplaySection } from "./DisplaySection";
import { LibrarySection } from "./LibrarySection";
import { Section } from "./Section";
import { SlotCardView } from "./SlotCardView";
import { StorageSection } from "./StorageSection";

const NAV: { id: SettingsSection; label: string }[] = [
  { id: "exchanges", label: "거래소 연결" },
  { id: "account", label: "껄무새 계정" },
  { id: "library", label: "매크로 폴더" },
  { id: "display", label: "창 · 화면" },
  { id: "storage", label: "기억한 키" },
  { id: "about", label: "정보" },
];

function goTo(target: SettingsTarget) {
  const el = document.getElementById(settingsElementId(target));
  if (!el) return;
  el.scrollIntoView?.({
    behavior: reducedMotion() ? "auto" : "smooth",
    block: target.kind === "section" ? "start" : "center",
  });
  const input = target.kind === "section" ? null : el.querySelector<HTMLElement>("input:not([disabled])");
  (input ?? el).focus({ preventScroll: true });
  el.classList.remove("is-target");
  void el.offsetWidth;
  el.classList.add("is-target");
}

export function SettingsScreen({
  target,
  onBack,
}: {
  target: { target: SettingsTarget; seq: number } | null;
  onBack(): void;
}) {
  const { state } = useRunnerContext();
  const title = useRef<HTMLHeadingElement>(null);

  useEffect(() => {
    if (!target) {
      title.current?.focus({ preventScroll: true });
      return;
    }
    const id = requestAnimationFrame(() => goTo(target.target));
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
              onClick={() => goTo({ kind: "section", section: n.id })}
            >
              {n.label}
            </button>
          ))}
        </nav>
        <div className="settings-content">
          <Section
            id="exchanges"
            title="거래소 연결"
            lead="모의는 거래소 키 없이 시작할 수 있어요(회원 키는 필요해요). 테스트넷 · 실전은 해당 거래소의 API 키가 있어야 시작해요. 키는 언제든 바꿀 수 있어요."
          >
            <div className="slot-grid">
              {state.settings.slots.map((s) => (
                <SlotCardView key={s.slot} slot={s} />
              ))}
            </div>
          </Section>
          <MemberSection />
          <LibrarySection />
          <DisplaySection />
          <StorageSection />
          <AboutSection />
        </div>
      </div>
    </div>
  );
}
