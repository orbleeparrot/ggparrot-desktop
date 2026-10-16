import { ChevronRight } from "lucide-react";
import { useRunnerContext } from "../../state/context";
import type { BinanceEnv, WizardState } from "../../types";

// '처음이에요'를 고르면 할 일 — 국내는 허용 IP부터, 바이낸스는 그 사이트에 로그인해 키 만들기부터.
const NEW_HINTS: Record<BinanceEnv, string> = {
  "": "허용 IP 등록부터 차례로",
  testnet: "GitHub 로그인 뒤 키 만들기부터",
  demo: "바이낸스 로그인 뒤 키 만들기부터",
  live: "키 만들기 · 허용 IP까지 차례로",
};

export function AskStage({ wizard }: { wizard: WizardState }) {
  const { act } = useRunnerContext();
  const options = [
    { path: "new" as const, title: "처음이에요", sub: NEW_HINTS[wizard.env] },
    { path: "existing" as const, title: "이미 발급받았어요", sub: "보관해 둔 키를 넣고 바로 연결 확인" },
  ];
  return (
    <div className="wz-ask">
      <h3 className="wz-title wz-question" data-stage-title tabIndex={-1}>
        {wizard.label} API 키를 발급받은 적 있나요?
      </h3>
      <div className="wz-options">
        {options.map((o) => (
          <button
            key={o.path}
            type="button"
            className="wz-option"
            onClick={() => act("wizard_choose", { path: o.path })}
          >
            <span className="wz-option-text">
              <strong>{o.title}</strong>
              <span>{o.sub}</span>
            </span>
            <ChevronRight size={18} aria-hidden="true" />
          </button>
        ))}
      </div>
    </div>
  );
}
