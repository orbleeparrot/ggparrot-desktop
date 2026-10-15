// 설정 — 실행 화면을 바꿔 끼우는 전체 화면. Esc·[← 실행 화면으로]를 누르면 돌아감.
// target이 오면 그 칸으로 굴리고 첫 입력칸에 포커스를 둠([설정에서 입력]·업데이트 칩).
// 앱 기록의 한 줄(토스트 [자세히])은 기록을 읽은 뒤 AppLogSection이 직접 데려감.
// - 왼쪽 목차는 지금 보이는 묶음을 밝힘(IntersectionObserver로 본문 위쪽 띠에 걸린 묶음, aria-current="true").
// - 거래소 칸은 쓰는 칸을 앞에 둠 — 순서는 설정을 열 때 한 번 정해 키를 넣는 동안 자리가 튀지 않게 함.
import { ArrowLeft } from "lucide-react";
import { useEffect, useMemo, useRef, useState } from "react";
import { keyStoreName } from "../../platform";
import { useRunnerContext } from "../../state/context";
import type { SettingsSection, SettingsTarget } from "../../state/runs";
import { arrangeSlots, slotOrder } from "../../state/slots";
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
  const body = useRef<HTMLDivElement>(null);
  const [active, setActive] = useState<SettingsSection>("exchanges");
  // 칸 순서는 처음 그릴 때의 '쓰는 칸 먼저'로 고정함.
  const [order] = useState(() => slotOrder(state.settings.slots));
  const slots = arrangeSlots(state.settings.slots, order);
  const slotTarget = target?.target.kind === "slot" ? { slot: target.target.slot, seq: target.seq } : null;
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

  // 목차 밝히기 — 본문 위쪽 35% 띠에 걸린 묶음 중 맨 위. 걸린 게 없으면 앞의 값을 둠.
  // - 맨 아래까지 굴리면 마지막 묶음(띠까지 올라오지 못할 수 있음).
  // - 목차를 누르면 사용자가 직접 굴릴 때(휠·터치·스크롤바·키)까지 누른 묶음을 지킴 — 굴러가는 동안 지나는 묶음이나,
  //   맨 아래라 띠까지 못 올라온 짧은 묶음으로 바뀌지 않게.
  const pinned = useRef(false);
  useEffect(() => {
    const root = body.current;
    if (!root || typeof IntersectionObserver === "undefined") return;
    const sections = NAV.map((n) => document.getElementById(`settings-${n.id}`)).filter(
      (el): el is HTMLElement => !!el,
    );
    const hits = new Set<string>();
    const pick = () => {
      if (pinned.current) return;
      const atEnd = root.scrollTop > 0 && root.scrollHeight - root.scrollTop - root.clientHeight < 2;
      const first = atEnd ? sections[sections.length - 1] : sections.find((el) => hits.has(el.id));
      if (first) setActive(first.id.replace("settings-", "") as SettingsSection);
    };
    const io = new IntersectionObserver(
      (entries) => {
        for (const e of entries) {
          if (e.isIntersecting) hits.add(e.target.id);
          else hits.delete(e.target.id);
        }
        pick();
      },
      { root, rootMargin: "0px 0px -65% 0px" },
    );
    sections.forEach((el) => io.observe(el));
    const unpin = () => {
      pinned.current = false;
    };
    const userInput = ["wheel", "touchstart", "pointerdown", "keydown"] as const;
    root.addEventListener("scroll", pick, { passive: true });
    userInput.forEach((t) => root.addEventListener(t, unpin, { passive: true }));
    return () => {
      io.disconnect();
      root.removeEventListener("scroll", pick);
      userInput.forEach((t) => root.removeEventListener(t, unpin));
    };
  }, []);

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
      <div ref={body} className="settings-body">
        <nav className="settings-nav" aria-label="설정 목차">
          {NAV.map((n) => (
            <button
              key={n.id}
              type="button"
              className="settings-nav-item"
              aria-current={active === n.id ? "true" : undefined}
              onClick={() => {
                pinned.current = true;
                setActive(n.id);
                goToSettings({ kind: "section", section: n.id });
              }}
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
              {slots.map((s) => (
                <SlotCardView
                  key={s.slot}
                  slot={s}
                  openSeq={slotTarget?.slot === s.slot ? slotTarget.seq : undefined}
                />
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
