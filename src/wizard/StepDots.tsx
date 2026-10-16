import type { WizardState } from "../types";
import { stepStates } from "./steps";

export function StepDots({ wizard }: { wizard: WizardState }) {
  const steps = stepStates(wizard);
  if (steps.length === 0) return null;
  return (
    <ol className="wz-steps" aria-label="진행 단계">
      {steps.map((s) => (
        <li key={s.label} className={`wz-step is-${s.state}`} aria-current={s.state === "on" ? "step" : undefined}>
          <span className="wz-step-label">{s.label}</span>
        </li>
      ))}
    </ol>
  );
}
