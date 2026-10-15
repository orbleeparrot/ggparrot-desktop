// 알림 토스트 — 정보는 4초 뒤 저절로 사라지고, 실패한 명령(오류)은 닫을 때까지 남음(state/notices.ts).
// 머리줄 아래 오른쪽 위에 밝은 카드로 뜸. 열린 대화상자보다 아래 층(layout.css .toasts).
// 앱 기록에 남은 알림(logId)은 [자세히]로 설정 › 앱 기록의 그 줄을 열고 토스트는 닫음.
import { Info, TriangleAlert, X, type LucideIcon } from "lucide-react";
import { createElement } from "react";
import type { Toast } from "../state/notices";

const ICONS: Record<Toast["tone"], LucideIcon> = { info: Info, error: TriangleAlert };

export function Toasts({
  toasts,
  onDismiss,
  onDetail,
}: {
  toasts: Toast[];
  onDismiss(id: number): void;
  onDetail(logId: number): void;
}) {
  const detail = (id: number, logId: number) => () => {
    onDetail(logId);
    onDismiss(id);
  };
  return (
    <div className="toasts" aria-live="polite">
      {toasts.map((t) => (
        <div key={t.id} className={`toast toast-${t.tone}`} role={t.tone === "error" ? "alert" : "status"}>
          <span className="toast-icon" aria-hidden="true">
            {createElement(ICONS[t.tone], { size: 15, strokeWidth: 2.4 })}
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
          <button
            type="button"
            className="icon-btn"
            aria-label="알림 닫기"
            title="닫기"
            onClick={() => onDismiss(t.id)}
          >
            <X size={14} />
          </button>
        </div>
      ))}
    </div>
  );
}
