// 알림 토스트 — 정보는 4초 뒤 저절로 사라지고, 실패한 명령(오류)은 닫을 때까지 남음(state/notices.ts).
// 머리줄 아래 오른쪽 위에 흰 카드 + 왼쪽 색 줄로 뜸(정보 파랑 · 오류 빨강) — 노란 바탕은 경고처럼 보여 쓰지 않음.
// 대화상자 · 연결 도우미가 열려 있으면 어두운 막 위로 띄움(layout.css) — 막 뒤에 깔려 반쯤 가려지지 않게.
// - 패널 위에 토스트가 들어갈 틈이 없고 아래에 있으면 아래 오른쪽으로 내림(.at-bottom). 둘 다 없으면 위 오른쪽에 겹쳐 뜸.
// - 대화상자가 열린 채 토스트를 닫으면 포커스를 대화상자로 돌려줌(포커스가 막 뒤 화면으로 빠지지 않게).
// 앱 기록에 남은 알림(logId)은 [자세히]로 설정 › 앱 기록의 그 줄을 열고 토스트는 닫음.
import { Info, TriangleAlert, X, type LucideIcon } from "lucide-react";
import { createElement, useLayoutEffect, useRef, useState, type RefObject } from "react";
import type { Toast } from "../state/notices";

const ICONS: Record<Toast["tone"], LucideIcon> = { info: Info, error: TriangleAlert };

/** 토스트 묶음의 위 끝 — 머리줄 높이(theme.css --header-h 54) + 10. */
const TOP_PX = 64;

/** 열린 대화상자 · 도우미 패널(막 바로 아래 자식). */
function modalPanel(): HTMLElement | null {
  return document.querySelector<HTMLElement>(".scrim > .dialog, .scrim > .wizard");
}

/** 패널 위가 모자라고 아래에 틈이 있으면 true — 토스트를 아래로 내림. 토스트가 떠 있는 동안만 잼. */
function useBottom(box: RefObject<HTMLDivElement | null>, count: number): boolean {
  const [bottom, setBottom] = useState(false);
  useLayoutEffect(() => {
    if (count === 0) return;
    let frame = 0;
    const measure = () => {
      cancelAnimationFrame(frame);
      frame = requestAnimationFrame(() => {
        const panel = modalPanel();
        const h = box.current?.offsetHeight ?? 0;
        // offsetTop — 막(창 전체에 고정)에서 잰 자리. 떠오르는 효과(transform) 중에도 흔들리지 않음.
        const top = panel?.offsetTop ?? 0;
        const end = top + (panel?.offsetHeight ?? 0);
        setBottom(!!panel && top - TOP_PX < h + 8 && window.innerHeight - end >= h + 24);
      });
    };
    measure();
    // 대화상자가 열리고 닫히는 것 · 도우미 단계가 바뀌는 것 · 창 크기를 따라감.
    const mo = new MutationObserver(measure);
    mo.observe(document.body, { childList: true, subtree: true });
    window.addEventListener("resize", measure);
    return () => {
      cancelAnimationFrame(frame);
      mo.disconnect();
      window.removeEventListener("resize", measure);
    };
  }, [box, count]);
  return bottom;
}

export function Toasts({
  toasts,
  onDismiss,
  onDetail,
}: {
  toasts: Toast[];
  onDismiss(id: number): void;
  onDetail(logId: number): void;
}) {
  const box = useRef<HTMLDivElement>(null);
  const bottom = useBottom(box, toasts.length);
  const dismiss = (id: number) => {
    onDismiss(id);
    if (box.current?.contains(document.activeElement)) modalPanel()?.focus();
  };
  const detail = (id: number, logId: number) => () => {
    onDetail(logId);
    dismiss(id);
  };
  return (
    <div ref={box} className={`toasts ${bottom ? "at-bottom" : ""}`} aria-live="polite">
      {toasts.map((t) => (
        <div key={t.id} className={`toast toast-${t.tone}`} role={t.tone === "error" ? "alert" : "status"}>
          <span className="toast-icon" aria-hidden="true">
            {createElement(ICONS[t.tone], { size: 16, strokeWidth: 2.4 })}
          </span>
          <div className="toast-text">
            {t.title && <strong>{t.title}</strong>}
            <span>{t.message}</span>
            {t.logId !== undefined && (
              <button
                type="button"
                className="link-btn toast-detail"
                title="설정 › 앱 기록에서 자세히 보기"
                onClick={detail(t.id, t.logId)}
              >
                자세히
              </button>
            )}
          </div>
          <button type="button" className="icon-btn" aria-label="알림 닫기" title="닫기" onClick={() => dismiss(t.id)}>
            <X size={14} />
          </button>
        </div>
      ))}
    </div>
  );
}
