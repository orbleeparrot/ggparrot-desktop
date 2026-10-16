// 연결 도우미(업비트·빗썸·바이낸스) — 지금 보이는 화면(실행·설정) 위에 뜨는 시트(한 열에서는 전체 화면).
// 포커스를 안에 가두고 Esc로 닫힘. 닫아도 매매는 시작하지 않음.
import { Check, ExternalLink, LoaderCircle, X } from "lucide-react";
import { useEffect, useRef, type KeyboardEvent } from "react";
import { ExchangeLogo } from "../components/ExchangeLogo";
import { trapTab, useModalFocus } from "../components/focus";
import { useRunnerContext } from "../state/context";
import type { WizardState } from "../types";
import { AskStage } from "./stages/AskStage";
import { GuideStage } from "./stages/GuideStage";
import { IpStage } from "./stages/IpStage";
import { ManualStage } from "./stages/ManualStage";
import { ResultStage } from "./stages/ResultStage";
import { StepDots } from "./StepDots";
import { footFor, nextStage, stageTitle, type FootButton } from "./steps";

function titleOf(w: WizardState): string {
  if (w.stage === "result_fail" || w.stage === "result_ok") return w.headline || stageTitle(w);
  return stageTitle(w);
}

function StageBody({ wizard }: { wizard: WizardState }) {
  switch (wizard.stage) {
    case "ask":
      return <AskStage wizard={wizard} />;
    case "ip":
      return <IpStage wizard={wizard} />;
    case "perm":
    case "keys":
    case "existing":
      return <GuideStage wizard={wizard} kind={wizard.stage} />;
    case "manual":
      return <ManualStage wizard={wizard} />;
    default:
      return <ResultStage wizard={wizard} />;
  }
}

export function Wizard({ wizard }: { wizard: WizardState }) {
  const { act } = useRunnerContext();
  const panel = useRef<HTMLElement>(null);
  const body = useRef<HTMLDivElement>(null);
  const foot = footFor(wizard);
  useModalFocus(panel);

  function onKeyDown(e: KeyboardEvent<HTMLElement>) {
    if (e.key === "Escape" && !e.defaultPrevented) {
      e.preventDefault();
      e.stopPropagation();
      void act("wizard_close");
      return;
    }
    trapTab(e, panel.current);
  }

  // 단계가 바뀌면 맨 위로 굴리고 제목에 포커스를 둠(화면 읽기 프로그램이 새 단계를 읽게).
  useEffect(() => {
    body.current?.scrollTo?.({ top: 0 });
    if (wizard.stage === "manual") return;
    body.current?.querySelector<HTMLElement>("[data-stage-title]")?.focus({ preventScroll: true });
  }, [wizard.stage]);

  function press(b: FootButton) {
    switch (b.action) {
      case "back":
        return act("wizard_back");
      case "close":
      case "done":
        return act("wizard_close");
      case "next": {
        const next = nextStage(wizard);
        return next ? act("wizard_go", { stage: next }) : undefined;
      }
      case "manual":
        return act("wizard_go", { stage: "manual" });
      case "check":
        return act("wizard_recheck");
    }
  }

  const resultIcon =
    wizard.stage === "checking" ? (
      <span className="wz-title-icon is-spin">
        <LoaderCircle size={18} className="spin" />
      </span>
    ) : wizard.stage === "result_ok" ? (
      <span className="wz-title-icon is-ok">
        <Check size={15} strokeWidth={3} />
      </span>
    ) : wizard.stage === "result_fail" ? (
      <span className="wz-title-icon is-bad">
        <X size={15} strokeWidth={3} />
      </span>
    ) : null;

  return (
    <div className="scrim scrim-wizard">
      <section
        ref={panel}
        className="wizard"
        role="dialog"
        aria-modal="true"
        aria-labelledby="wizard-name"
        tabIndex={-1}
        onKeyDown={onKeyDown}
      >
        <header className="wz-head">
          <div className="wz-name">
            <ExchangeLogo exchange={wizard.exchange} size={22} />
            <h2 id="wizard-name" className="card-title">
              {wizard.label} 연결 도우미
            </h2>
          </div>
          <div className="wz-links">
            <button type="button" className="btn btn-quiet btn-sm" onClick={() => act("wizard_open_home")}>
              {wizard.homeLabel} 열기 <ExternalLink size={13} aria-hidden="true" />
            </button>
            <button type="button" className="btn btn-quiet btn-sm" onClick={() => act("wizard_open_api_page")}>
              API 관리 열기 <ExternalLink size={13} aria-hidden="true" />
            </button>
            <button
              type="button"
              className="icon-btn wz-close"
              aria-label="연결 도우미 닫기"
              onClick={() => act("wizard_close")}
            >
              <X size={18} />
            </button>
          </div>
        </header>

        <div ref={body} className="wz-body">
          <div key={wizard.stage} className="wz-stage">
            <StepDots wizard={wizard} />
            {wizard.stage !== "ask" && (
              <h3 className="wz-title" data-stage-title tabIndex={-1} aria-live="polite">
                {resultIcon}
                {titleOf(wizard)}
              </h3>
            )}
            <StageBody wizard={wizard} />
          </div>
        </div>

        <footer className="wz-foot">
          {foot.left && (
            <button
              type="button"
              className="btn btn-ghost"
              onClick={() => foot.left && press(foot.left)}
              disabled={!foot.left.enabled}
            >
              {foot.left.label}
            </button>
          )}
          <span className="action-spacer" />
          {foot.right && (
            <button
              type="button"
              className={`btn ${foot.right.primary ? "btn-primary" : "btn-ghost"}`}
              onClick={() => foot.right && press(foot.right)}
              disabled={!foot.right.enabled}
            >
              {foot.right.label}
            </button>
          )}
        </footer>
      </section>
    </div>
  );
}
