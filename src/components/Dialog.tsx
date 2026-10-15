// 모달 대화상자·아래에서 올라오는 시트. 포커스를 안에 가두고 Esc로 닫음. 닫히면 이전 포커스로 돌아감.
import { useId, useRef, type KeyboardEvent, type ReactNode } from "react";
import { trapTab, useModalFocus } from "./focus";

interface Props {
  title: string;
  children?: ReactNode;
  actions: ReactNode;
  onClose(): void;
  variant?: "dialog" | "sheet";
  tone?: "default" | "warn" | "danger" | "live";
  icon?: ReactNode;
}

export function Dialog({ title, children, actions, onClose, variant = "dialog", tone = "default", icon }: Props) {
  const panel = useRef<HTMLDivElement>(null);
  const titleId = useId();
  const bodyId = useId();

  useModalFocus(panel);

  function onKeyDown(e: KeyboardEvent<HTMLDivElement>) {
    if (e.key === "Escape") {
      e.stopPropagation();
      e.preventDefault();
      onClose();
      return;
    }
    trapTab(e, panel.current);
  }

  return (
    <div className={`scrim scrim-${variant}`} onMouseDown={(e) => e.target === e.currentTarget && onClose()}>
      <div
        ref={panel}
        className={`dialog dialog-${variant} tone-${tone}`}
        role="dialog"
        aria-modal="true"
        aria-labelledby={titleId}
        aria-describedby={children ? bodyId : undefined}
        tabIndex={-1}
        onKeyDown={onKeyDown}
      >
        <div className="dialog-head">
          {icon && (
            <span className="dialog-icon" aria-hidden="true">
              {icon}
            </span>
          )}
          <h2 id={titleId} className="dialog-title">
            {title}
          </h2>
        </div>
        {children && (
          <div id={bodyId} className="dialog-body">
            {children}
          </div>
        )}
        <div className="dialog-actions">{actions}</div>
      </div>
    </div>
  );
}
