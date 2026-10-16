// 권한 선택·키 발급(바이낸스는 키 만들기) / 키 복사 / 기존 키 넣기 — 거래소 화면 그림 + 받은 칸 표시(클립보드 자동 채우기).
import { Check, ClipboardCheck } from "lucide-react";
import { useRunnerContext } from "../../state/context";
import type { WizardState } from "../../types";
import { ExchangeScreen, siteOf, type Site } from "../screens";

type Kind = "perm" | "keys" | "existing";

const DOMESTIC_LEAD: Record<Kind, string> = {
  perm: "자산조회 · 주문조회 · 주문하기만 체크하고 키를 발급하세요. 출금은 켜지 마세요.",
  keys: "발급 창의 [복사]를 차례로 누르면 아래 칸이 저절로 채워지고 바로 연결을 확인해요.",
  existing: "발급할 때 따로 보관해 둔 키를 복사하세요. 두 칸이 차면 바로 연결을 확인해요.",
};
const BINANCE_EXISTING =
  "Secret Key는 만들 때 한 번만 보여요. 따로 보관해 둔 키를 복사하세요. 두 칸이 차면 바로 연결을 확인해요.";

// 사이트마다 키를 만드는 길이 다름 — 그림과 같은 순서로 적음.
const LEAD: Record<Site, Record<Kind, string>> = {
  upbit: DOMESTIC_LEAD,
  bithumb: DOMESTIC_LEAD,
  "binance:testnet": {
    perm: "GitHub로 로그인한 뒤 Generate HMAC-SHA-256 Key를 누르고 이름을 넣어 키를 만드세요. 권한은 기본값(TRADE · USER_DATA · USER_STREAM) 그대로 두세요.",
    keys: "이 화면에는 [복사]가 없어요. API Key와 Secret Key 값을 차례로 골라 복사하면 아래 칸이 저절로 채워지고 바로 연결을 확인해요.",
    existing: BINANCE_EXISTING,
  },
  "binance:demo": {
    perm: "바이낸스 계정으로 로그인한 뒤 Create API → System generated를 고르고 이름을 넣어 키를 만드세요.",
    keys: "키 카드의 Copy를 차례로 누르면 아래 칸이 저절로 채워지고 바로 연결을 확인해요.",
    existing: BINANCE_EXISTING,
  },
  "binance:live": {
    perm: "Create API → System generated를 고르고 이름을 넣어 키를 만드세요. 본인 확인을 마치면 키가 보여요. 출금 권한은 켜지 마세요.",
    keys: "키 카드의 Copy를 차례로 누르면 아래 칸이 저절로 채워져요. 다음 단계에서 이 키에 허용 IP와 주문 권한을 걸어요.",
    existing: BINANCE_EXISTING,
  },
};

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

export function GuideStage({ wizard, kind }: { wizard: WizardState; kind: Kind }) {
  const { act } = useRunnerContext();
  const site = siteOf(wizard);
  return (
    <>
      <p className="wz-lead">{LEAD[site][kind]}</p>
      <ExchangeScreen site={site} kind={kind} ip={wizard.ip.address} label={wizard.label} market={wizard.market} />
      {wizard.watchingClipboard && (
        <p className="wz-watch" role="status">
          <ClipboardCheck size={14} aria-hidden="true" />
          거래소에서 복사한 키를 기다리는 중
        </p>
      )}
      {kind !== "perm" && <Slots wizard={wizard} />}
      {kind === "existing" && (
        <button type="button" className="link-btn wz-new-link" onClick={() => act("wizard_choose", { path: "new" })}>
          Secret Key가 없어요 · {wizard.exchange === "binance" ? "새로 만들기" : "새로 발급"}
        </button>
      )}
    </>
  );
}
