// 알림 토스트 — 정보는 4초 뒤 저절로 사라지고, 실패한 명령(오류)은 닫을 때까지 남음(state/notices.ts).
import { Info, TriangleAlert, X } from "lucide-react";
import type { Toast } from "../state/notices";

export function Toasts({ toasts, onDismiss }: { toasts: Toast[]; onDismiss(id: number): void }) {
  return (
    <div className="toasts" aria-live="polite">
      {toasts.map((t) => (
        <div key={t.id} className={`toast toast-${t.tone}`} role={t.tone === "error" ? "alert" : "status"}>
          {t.tone === "error" ? <TriangleAlert size={16} aria-hidden="true" /> : <Info size={16} aria-hidden="true" />}
          <div className="toast-text">
            {t.title && <strong>{t.title}</strong>}
            <span>{t.message}</span>
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
