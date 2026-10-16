// 직접 입력 — 키 두 칸. 값은 이 화면 로컬 초안에만 있고 wizard_set_key로 보냄.
import { useEffect, useRef } from "react";
import { SecretField } from "../../components/SecretField";
import { useRunnerContext } from "../../state/context";
import type { WizardState } from "../../types";

export function ManualStage({ wizard }: { wizard: WizardState }) {
  const { act } = useRunnerContext();
  const first = useRef<HTMLInputElement>(null);
  useEffect(() => first.current?.focus(), []);
  return (
    <div className="wz-manual">
      {(["access", "secret"] as const).map((field, i) => (
        <SecretField
          key={field}
          ref={i === 0 ? first : undefined}
          id={`wizard-${field}`}
          label={wizard.fieldLabels[i]}
          filled={wizard.slots[field]}
          onCommit={(value) => act("wizard_set_key", { field, value })}
          onReveal={() => act("reveal_slot_key", { slot: wizard.slot, field })}
        />
      ))}
    </div>
  );
}
