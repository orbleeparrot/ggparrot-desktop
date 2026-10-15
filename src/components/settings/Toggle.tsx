import { useId } from "react";

/** 켜고 끄는 스위치 한 줄 — 제목·설명·스위치(role=switch). */
export function Toggle({
  label,
  hint,
  checked,
  onChange,
}: {
  label: string;
  hint?: string;
  checked: boolean;
  onChange(next: boolean): void;
}) {
  const id = useId();
  return (
    <div className="toggle-row">
      <div className="toggle-text">
        <span id={`${id}-label`} className="strong">
          {label}
        </span>
        {hint && (
          <span id={`${id}-hint`} className="muted">
            {hint}
          </span>
        )}
      </div>
      <button
        type="button"
        role="switch"
        className="switch"
        aria-checked={checked}
        aria-labelledby={`${id}-label`}
        aria-describedby={hint ? `${id}-hint` : undefined}
        onClick={() => onChange(!checked)}
      >
        <span className="switch-knob" aria-hidden="true" />
      </button>
    </div>
  );
}
