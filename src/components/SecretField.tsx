// 비밀 칸 — 값은 이 컴포넌트의 로컬 초안에만 있음. 전역 상태에는 '찼는가'만 옴.
// 찼고 손대지 않았으면 점(••••••••)을 보이고, 입력하면 새 값으로 바뀜. 눈을 누르면 reveal_key로 한 번 받아 보여 주고,
// 다시 가리면 받은 값을 지움. 입력은 150ms 디바운스로 보냄.
import { Eye, EyeOff } from "lucide-react";
import { forwardRef, useCallback, useEffect, useRef, useState } from "react";

export const SECRET_DEBOUNCE_MS = 150;
export const FILLED_MASK = "••••••••";

interface Props {
  id: string;
  label: string;
  filled: boolean;
  disabled?: boolean;
  placeholder?: string;
  onCommit(value: string): void;
  onReveal(): Promise<string | undefined>;
}

export const SecretField = forwardRef<HTMLInputElement, Props>(function SecretField(
  { id, label, filled, disabled, placeholder = "붙여넣기", onCommit, onReveal },
  ref,
) {
  const [draft, setDraft] = useState("");
  const [touched, setTouched] = useState(false);
  const [visible, setVisible] = useState(false);
  const [revealing, setRevealing] = useState(false);
  const timer = useRef<ReturnType<typeof setTimeout> | undefined>(undefined);
  const pending = useRef<string | null>(null);
  const commitRef = useRef(onCommit);
  useEffect(() => {
    commitRef.current = onCommit;
  }, [onCommit]);

  const flush = useCallback(() => {
    clearTimeout(timer.current);
    if (pending.current !== null) {
      const v = pending.current;
      pending.current = null;
      commitRef.current(v);
    }
  }, []);

  useEffect(() => flush, [flush]);

  function change(value: string) {
    setDraft(value);
    setTouched(true);
    pending.current = value;
    clearTimeout(timer.current);
    timer.current = setTimeout(flush, SECRET_DEBOUNCE_MS);
  }

  async function toggle() {
    if (visible) {
      flush();
      setVisible(false);
      setDraft("");
      setTouched(false);
      return;
    }
    if (!touched && filled) {
      setRevealing(true);
      const value = await onReveal();
      setRevealing(false);
      if (value === undefined) return;
      setDraft(value);
      setTouched(true);
    }
    setVisible(true);
  }

  return (
    <div className="secret-row">
      <label className="field-label" htmlFor={id}>
        {label}
      </label>
      <div className={`secret-field ${filled ? "is-filled" : ""}`}>
        <input
          ref={ref}
          id={id}
          className="input"
          type={visible ? "text" : "password"}
          value={touched ? draft : ""}
          placeholder={filled && !touched ? FILLED_MASK : placeholder}
          onChange={(e) => change(e.target.value)}
          onBlur={flush}
          disabled={disabled}
          autoComplete="off"
          maxLength={200}
          autoCapitalize="off"
          autoCorrect="off"
          spellCheck={false}
          data-filled={filled ? "true" : "false"}
        />
        <button
          type="button"
          className="eye-btn"
          onClick={toggle}
          disabled={disabled || revealing || (!filled && !touched)}
          aria-label={visible ? `${label} 가리기` : `${label} 보기`}
          aria-pressed={visible}
          aria-controls={id}
        >
          {visible ? <EyeOff size={16} /> : <Eye size={16} />}
        </button>
      </div>
    </div>
  );
});
