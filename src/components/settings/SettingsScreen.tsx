// 설정 — 실행 화면을 바꿔 끼우는 전체 화면. Esc·[← 실행 화면으로]를 누르면 돌아감.
// target이 오면 그 칸으로 굴리고 첫 입력칸에 포커스를 둠([설정에서 입력]·업데이트 칩).
// 앱 기록의 한 줄(토스트 [자세히])은 기록을 읽은 뒤 AppLogSection이 직접 데려감.
// - 왼쪽 목차는 지금 읽는 묶음을 밝힘(제목이 본문 위쪽 35% 선을 넘은 마지막 묶음, aria-current="true").
// - 거래소 칸은 쓰는 칸을 앞에 둠 — 순서는 설정을 열 때 한 번 정해 키를 넣는 동안 자리가 튀지 않게 함.
//   펼친 칸은 두 칸씩 놓고, 짝 없는 칸은 한 줄을 다 씀(옆이 비지 않게).
import { ArrowLeft } from "lucide-react";
import { useEffect, useMemo, useRef, useState } from "react";
import { keyStoreName } from "../../platform";
import { useRunnerContext } from "../../state/context";
import type { SettingsSection, SettingsTarget } from "../../state/runs";
import { arrangeSlots, isPractice, slotInUse, slotOrder } from "../../state/slots";
import type { SlotCard } from "../../types";
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
  // 키 칸을 연 거래소. 처음엔 모두 닫힘. [설정에서 입력]으로 데려오면(번호가 바뀌면) 그 칸을 엶 — 렌더 중에 맞춰
  // 같은 커밋에서 키 칸이 생기게 함(goToSettings가 바로 찾음).
  const [opened, setOpened] = useState<ReadonlySet<SlotCard["slot"]>>(() => new Set());
  const [seenSeq, setSeenSeq] = useState<number | undefined>(undefined);
  if (slotTarget && slotTarget.seq !== seenSeq) {
    setSeenSeq(slotTarget.seq);
    if (!opened.has(slotTarget.slot)) setOpened(new Set(opened).add(slotTarget.slot));
  }
  // 쓰지 않는 연습 칸(바이낸스 테스트넷·데모)은 아래 '연습 칸'에 낮게.
  const main = slots.filter((s) => !isPractice(s) || slotInUse(s));
  const practice = slots.filter((s) => isPractice(s) && !slotInUse(s));
  const toggleSlot = (slot: SlotCard["slot"], open: boolean) => {
    const next = new Set(opened);
    if (open) next.add(slot);
    else next.delete(slot);
    setOpened(next);
  };
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

  // 목차 밝히기 — 제목이 본문 위쪽 35% 선을 넘어 올라온 묶음 가운데 마지막(지금 읽는 묶음).
  // - 맨 아래까지 굴리면 선을 절반 높이로 내림 — 짧은 마지막 묶음은 위쪽 절반에 들어올 때만 밝힘
  //   (그 위 묶음이 화면 대부분을 차지하는데 마지막 묶음이 밝혀지지 않게).
  // - 목차를 누르면 사용자가 직접 굴릴 때(휠·터치·스크롤바·키)까지 누른 묶음을 지킴 — 굴러가는 동안 지나는 묶음이나,
  //   맨 아래라 선까지 못 올라온 짧은 묶음으로 바뀌지 않게.
  const pinned = useRef(false);
  useEffect(() => {
    const root = body.current;
    if (!root) return;
    const sections = NAV.map((n) => document.getElementById(`settings-${n.id}`)).filter(
      (el): el is HTMLElement => !!el,
    );
    const pick = () => {
      if (pinned.current || sections.length === 0) return;
      const atEnd = root.scrollTop > 0 && root.scrollHeight - root.scrollTop - root.clientHeight < 2;
      const line = root.getBoundingClientRect().top + root.clientHeight * (atEnd ? 0.5 : 0.35);
      let current = sections[0];
      for (const el of sections) if (el.getBoundingClientRect().top <= line) current = el;
      setActive(current.id.replace("settings-", "") as SettingsSection);
    };
    const unpin = () => {
      pinned.current = false;
    };
    const userInput = ["wheel", "touchstart", "pointerdown", "keydown"] as const;
    root.addEventListener("scroll", pick, { passive: true });
    window.addEventListener("resize", pick);
    userInput.forEach((t) => root.addEventListener(t, unpin, { passive: true }));
    pick();
    return () => {
      root.removeEventListener("scroll", pick);
      window.removeEventListener("resize", pick);
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
            <div className="slot-list">
              {main.map((s) => (
                <SlotCardView
                  key={s.slot}
                  slot={s}
                  open={opened.has(s.slot)}
                  onToggle={(open) => toggleSlot(s.slot, open)}
                />
              ))}
            </div>
            {practice.length > 0 && (
              <>
                <h4 className="slot-list-title">연습 칸</h4>
                <div className="slot-list is-practice">
                  {practice.map((s) => (
                    <SlotCardView
                      key={s.slot}
                      slot={s}
                      open={opened.has(s.slot)}
                      onToggle={(open) => toggleSlot(s.slot, open)}
                    />
                  ))}
                </div>
              </>
            )}
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
