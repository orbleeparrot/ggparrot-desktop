// 허용 IP 등록 — 이 기기의 공인 IPv4·[복사]·[복사하고 API 관리 열기]·주의 문구·거래소 화면 그림.
// 국내는 첫 단계, 바이낸스 실거래는 키를 만든 뒤 마지막 단계(키 카드의 Edit restrictions에서 IP·주문 권한).
import { Check, Copy, ExternalLink, RefreshCw } from "lucide-react";
import { useCopiedFlag } from "../../components/useCopiedFlag";
import { useRunnerContext } from "../../state/context";
import type { WizardState } from "../../types";
import { ExchangeScreen, siteOf } from "../screens";

const PUBLIC_IP_CAVEAT =
  "이 주소는 방금 조회한 이 기기의 공인 IP예요. 고정 IP인지, 거래소에 허용 IP로 등록됐는지는 확인하지 않아요. " +
  "VPN이나 프록시를 쓰면 거래소가 보는 주소가 다를 수 있어요. " +
  "주소나 네트워크가 바뀌었다면 주소를 다시 확인해 등록한 뒤 연결을 확인하세요.";

/** 바이낸스 실거래 — 만든 키에 허용 IP와 주문 권한을 거는 길. 선물은 켤 권한 이름이 다름. */
export function restrictLead(market: WizardState["market"]): string {
  const permission = market === "futures" ? "Enable Futures" : "Enable Spot & Margin & Stock Trading";
  return (
    "API Management의 키 카드에서 Edit restrictions를 누르고 Restrict access to trusted IPs only를 고르면 " +
    "나오는 칸에 이 기기 주소를 넣고 Confirm을 누르세요. " +
    `주문 권한(${permission})도 켜고 Save를 누르면 돌아와 바로 연결을 확인해요.`
  );
}

function ipShown(ip: WizardState["ip"]): { text: string; tone: "ink" | "muted" | "danger" } {
  if (ip.status === "ok" && ip.address) return { text: ip.address, tone: "ink" };
  if (ip.status === "error") return { text: "확인 실패", tone: "danger" };
  return { text: "확인 중", tone: "muted" };
}

export function IpStage({ wizard }: { wizard: WizardState }) {
  const { act, ok } = useRunnerContext();
  const [copied, markCopied] = useCopiedFlag(1500);
  const ip = wizard.ip;
  const ready = ip.status === "ok" && !!ip.address;
  const shown = ipShown(ip);
  const canRetry = ip.status === "error" || ip.status === "expired" || ip.status === "idle";

  const binance = wizard.exchange === "binance";
  return (
    <>
      {binance && <p className="wz-lead">{restrictLead(wizard.market)}</p>}
      <ExchangeScreen
        site={siteOf(wizard)}
        kind={binance ? "restrict" : "ip"}
        ip={ready ? ip.address : ""}
        label={wizard.label}
        market={wizard.market}
      />
      <div className="wz-ip">
        <div className="wz-ip-text">
          <span className="eyebrow">이 기기 주소</span>
          <span className={`wz-ip-value num tone-${shown.tone}`} aria-live="polite">
            {shown.text}
          </span>
          {ip.note && <span className={`wz-ip-note ${ip.changed ? "is-changed" : ""}`}>{ip.note}</span>}
        </div>
        <div className="wz-ip-actions">
          {ready && (
            <button
              type="button"
              className="btn btn-ghost btn-sm"
              onClick={async () => (await ok("wizard_copy_ip")) && markCopied()}
            >
              {copied ? <Check size={14} aria-hidden="true" /> : <Copy size={14} aria-hidden="true" />}
              {copied ? "복사됨" : "복사"}
            </button>
          )}
          <button
            type="button"
            className={`btn btn-sm ${canRetry ? "btn-ghost" : "btn-quiet"}`}
            onClick={() => act("wizard_refresh_ip")}
            disabled={ip.status === "loading"}
            aria-label={canRetry ? undefined : "주소 다시 확인"}
            title={canRetry ? undefined : "주소 다시 확인"}
          >
            <RefreshCw size={14} aria-hidden="true" className={ip.status === "loading" ? "spin" : ""} />
            {canRetry ? "다시 확인" : null}
          </button>
        </div>
      </div>
      <button
        type="button"
        className="btn btn-primary btn-lg btn-block"
        disabled={!ready}
        onClick={() => act("wizard_copy_and_open")}
      >
        복사하고 API 관리 열기 <ExternalLink size={15} aria-hidden="true" />
      </button>
      <p className="wz-caveat">{PUBLIC_IP_CAVEAT}</p>
    </>
  );
}
