// 연결 확인 중·연결 완료·연결 확인 필요 — 세 줄 상태와 문제별 고치는 길.
import { ExternalLink, RefreshCw } from "lucide-react";
import { useRunnerContext } from "../../state/context";
import type { WizardState } from "../../types";
import { CheckRows } from "../CheckRows";
import { ExchangeScreen, siteOf } from "../screens";
import { keyStoreName } from "../../platform";
import { restrictLead } from "./IpStage";

export function ResultStage({ wizard }: { wizard: WizardState }) {
  const { act } = useRunnerContext();
  const ip = wizard.ip.status === "ok" ? wizard.ip.address : "";

  if (wizard.stage === "checking") {
    return <CheckRows checks={wizard.checks} notes={wizard.notes} ip={ip} spinning />;
  }

  if (wizard.stage === "result_ok") {
    return (
      <>
        <CheckRows checks={wizard.checks} notes={wizard.notes} ip={ip} />
        {wizard.problem === "order_size" && wizard.reason && <p className="wz-reason">{wizard.reason}</p>}
        {wizard.rememberSupported && (
          <label className="check-row">
            <input
              type="checkbox"
              checked={wizard.remembered}
              onChange={(e) => act("wizard_set_remember", { remember: e.target.checked })}
            />
            <span>이 기기에 키 기억하기 ({keyStoreName()}에 보관)</span>
          </label>
        )}
      </>
    );
  }

  const problem = wizard.problem;
  const binance = wizard.exchange === "binance";
  // 바이낸스는 주문 권한이 막혔을 때 어느 권한을 켤지(선물은 키를 새로 만들어야 하는 경우까지)가 이유에 있음.
  const showReason = problem === "order" ? binance : problem !== "ip" && problem !== "auth";
  return (
    <>
      <CheckRows checks={wizard.checks} notes={wizard.notes} ip={ip} />
      {problem === "ip" && binance && <p className="wz-lead">{restrictLead(wizard.market)}</p>}
      {problem === "ip" && (
        <ExchangeScreen
          site={siteOf(wizard)}
          kind={binance ? "restrict" : "fix"}
          ip={ip}
          label={wizard.label}
          market={wizard.market}
        />
      )}
      {wizard.reason && showReason && <p className="wz-reason">{wizard.reason}</p>}
      <div className="wz-fix-actions">
        {problem === "ip" && (
          <button type="button" className="btn btn-primary" onClick={() => act("wizard_copy_and_open")}>
            복사하고 API 관리 열기 <ExternalLink size={14} aria-hidden="true" />
          </button>
        )}
        {problem === "auth" && (
          <button type="button" className="btn btn-primary" onClick={() => act("wizard_recopy")}>
            키 다시 복사
          </button>
        )}
        {problem === "order" && (
          <button type="button" className="btn btn-primary" onClick={() => act("wizard_open_api_page")}>
            API 관리 열기 <ExternalLink size={14} aria-hidden="true" />
          </button>
        )}
        <button type="button" className="btn btn-ghost" onClick={() => act("wizard_recheck")}>
          <RefreshCw size={14} aria-hidden="true" /> 다시 확인
        </button>
      </div>
    </>
  );
}
