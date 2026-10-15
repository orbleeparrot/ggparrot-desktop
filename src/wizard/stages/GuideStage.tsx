// 2단계 권한 선택·키 발급 / 3단계 키 복사 / 기존 키 넣기 — 거래소 화면 그림 + 받은 칸 표시(클립보드 자동 채우기).
import { Check, ClipboardCheck } from "lucide-react";
import { useRunnerContext } from "../../state/context";
import type { WizardState } from "../../types";
import { ExchangeScreen } from "../screens";

const LEAD = {
  perm: "자산조회 · 주문조회 · 주문하기만 체크하고 키를 발급하세요. 출금은 켜지 마세요.",
  keys: "발급 창의 [복사]를 차례로 누르면 아래 칸이 저절로 채워지고 바로 연결을 확인해요.",
  existing: "발급할 때 따로 보관해 둔 키를 복사하세요. 두 칸이 차면 바로 연결을 확인해요.",
} as const;

function Slots({ wizard }: { wizard: WizardState }) {
  const rows = [
    ["access", wizard.fieldLabels[0]],
    ["secret", wizard.fieldLabels[1]],
  ] as const;
  return (
    <ul className="wz-rows">
      {rows.map(([slot, label]) => {
        const got = wizard.slots[slot];
        return (
          <li key={slot} className={`wz-row check-${got ? "ok" : "wait"}`}>
            <span className="wz-row-icon" aria-hidden="true">
              {got ? <Check size={13} strokeWidth={3} /> : <span className="wz-wait">…</span>}
            </span>
            <span className="wz-row-label">{label}</span>
            <span className={`wz-row-value ${got ? "is-got" : ""}`}>{got ? "받음" : "복사 대기"}</span>
          </li>
        );
      })}
    </ul>
  );
}

export function GuideStage({ wizard, kind }: { wizard: WizardState; kind: "perm" | "keys" | "existing" }) {
  const { act } = useRunnerContext();
  return (
    <>
      <p className="wz-lead">{LEAD[kind]}</p>
      <ExchangeScreen exchange={wizard.exchange} kind={kind} ip={wizard.ip.address} />
      {wizard.watchingClipboard && (
        <p className="wz-watch" role="status">
          <ClipboardCheck size={14} aria-hidden="true" />
          거래소에서 복사한 키를 기다리는 중
        </p>
      )}
      {kind !== "perm" && <Slots wizard={wizard} />}
      {kind === "existing" && (
        <button type="button" className="link-btn wz-new-link" onClick={() => act("wizard_choose", { path: "new" })}>
          Secret Key가 없어요 · 새로 발급
        </button>
      )}
    </>
  );
}
