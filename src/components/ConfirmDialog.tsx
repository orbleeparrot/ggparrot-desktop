// 묻는 대화상자 — [보조 단추(취소 등)] [주 단추] 두 개. 어느 단추든 먼저 닫고(onClose) 그 일을 함.
import type { ComponentProps, ReactNode } from "react";
import { Dialog } from "./Dialog";

/** 단추 하나. run이 없으면 닫기만 함. */
export interface ConfirmAction {
  label: string;
  run?(): void;
}

interface Props {
  title: string;
  children?: ReactNode;
  onClose(): void;
  /** 주 단추 — className은 색 클래스(btn-primary·btn-live·btn-danger-solid 등). */
  confirm: ConfirmAction & { className: string };
  /** 보조 단추(btn-ghost). 없으면 주 단추만. */
  cancel?: ConfirmAction;
  /** 열릴 때 포커스를 둘 단추. 되돌릴 수 없는 일은 보조 단추(취소)에 둠. */
  focus?: "cancel" | "confirm";
  /** 시트의 큰 단추(btn-lg). */
  large?: boolean;
  variant?: ComponentProps<typeof Dialog>["variant"];
  tone?: ComponentProps<typeof Dialog>["tone"];
  icon?: ReactNode;
}

export function ConfirmDialog({
  title,
  children,
  onClose,
  confirm,
  cancel,
  focus = "cancel",
  large = false,
  variant,
  tone,
  icon,
}: Props) {
  const size = large ? " btn-lg" : "";
  const press = (a: ConfirmAction) => () => {
    onClose();
    a.run?.();
  };
  return (
    <Dialog
      title={title}
      variant={variant}
      tone={tone}
      icon={icon}
      onClose={onClose}
      actions={
        <>
          {cancel && (
            <button
              type="button"
              className={`btn btn-ghost${size}`}
              data-autofocus={focus === "cancel" || undefined}
              onClick={press(cancel)}
            >
              {cancel.label}
            </button>
          )}
          <button
            type="button"
            className={`btn ${confirm.className}${size}`}
            data-autofocus={focus === "confirm" || undefined}
            onClick={press(confirm)}
          >
            {confirm.label}
          </button>
        </>
      }
    >
      {children}
    </Dialog>
  );
}
