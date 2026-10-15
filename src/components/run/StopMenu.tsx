// [■ 종료 ▾] — 종료 방법(stopChoices)을 고르는 메뉴. 고를 게 하나뿐이면 메뉴 없이 [■ 종료] 단추. 고르면 확인 대화상자로 감.
import { ChevronDown, Square } from "lucide-react";
import { useEffect, useId, useRef, useState, type KeyboardEvent } from "react";
import type { StopChoice } from "../../state/runs";
import type { StopMode } from "../../types";

export function StopMenu({
  label,
  choices,
  onChoose,
  size = "sm",
}: {
  label: string;
  choices: StopChoice[];
  onChoose(mode: StopMode): void;
  size?: "sm" | "md";
}) {
  const [open, setOpen] = useState(false);
  const menuId = useId();
  const trigger = useRef<HTMLButtonElement>(null);
  const menu = useRef<HTMLDivElement>(null);

  useEffect(() => {
    if (!open) return;
    menu.current?.querySelector<HTMLElement>('[role="menuitem"]')?.focus();
    function onDown(e: MouseEvent) {
      if (!menu.current?.contains(e.target as Node) && !trigger.current?.contains(e.target as Node)) setOpen(false);
    }
    document.addEventListener("mousedown", onDown);
    return () => document.removeEventListener("mousedown", onDown);
  }, [open]);

  function close(refocus = true) {
    setOpen(false);
    if (refocus) trigger.current?.focus();
  }

  function onMenuKey(e: KeyboardEvent<HTMLDivElement>) {
    const items = Array.from(menu.current?.querySelectorAll<HTMLElement>('[role="menuitem"]') ?? []);
    const i = items.indexOf(document.activeElement as HTMLElement);
    if (e.key === "Escape") {
      e.preventDefault();
      e.stopPropagation();
      close();
    } else if (e.key === "ArrowDown" || e.key === "ArrowUp") {
      e.preventDefault();
      const d = e.key === "ArrowDown" ? 1 : -1;
      items[(i + d + items.length) % items.length]?.focus();
    } else if (e.key === "Tab") {
      close(false);
    }
  }

  if (choices.length === 1)
    return (
      <button
        type="button"
        className={`btn btn-stop ${size === "sm" ? "btn-sm" : ""}`}
        aria-label={label}
        onClick={() => onChoose(choices[0].mode)}
      >
        <Square size={11} fill="currentColor" aria-hidden="true" />
        종료
      </button>
    );

  return (
    <div className="stop-menu">
      <button
        ref={trigger}
        type="button"
        className={`btn btn-stop ${size === "sm" ? "btn-sm" : ""}`}
        aria-haspopup="menu"
        aria-expanded={open}
        aria-controls={open ? menuId : undefined}
        aria-label={label}
        onClick={() => setOpen((v) => !v)}
        onKeyDown={(e) => {
          if (e.key === "ArrowDown") {
            e.preventDefault();
            setOpen(true);
          }
        }}
      >
        <Square size={11} fill="currentColor" aria-hidden="true" />
        종료
        <ChevronDown size={14} aria-hidden="true" />
      </button>
      {open && (
        <div ref={menu} id={menuId} className="menu" role="menu" aria-label="종료 방법" onKeyDown={onMenuKey}>
          {choices.map((c) => (
            <button
              key={c.mode}
              type="button"
              role="menuitem"
              tabIndex={-1}
              className={`menu-item ${c.danger ? "is-danger" : ""}`}
              onClick={() => {
                close(false);
                onChoose(c.mode);
              }}
            >
              {c.label}
            </button>
          ))}
        </div>
      )}
    </div>
  );
}
