// mock 연결 도우미 — 단계 이동·공인 IP 조회·클립보드 자동 채우기·연결 확인 결과. 키는 그 칸(m.keys[slot])에 넣음.
// 바이낸스는 실행 환경(테스트넷·데모·실거래)마다 단계가 다름(Rust `wizard::flow`와 같은 순서).
import { FIELD_LABELS, SLOT_LABELS, type Model } from "./mockData";
import type { BinanceEnv, Exchange, KeyField, KeySlot, Market, WizardState } from "./types";
import { flowOf } from "./wizard/steps";

type Stage = WizardState["stage"];

export interface WizardTarget {
  exchange: Exchange;
  slot: KeySlot;
  env: BinanceEnv;
  market: Market;
}

interface Deps {
  later(ms: number, fn: () => void): ReturnType<typeof setTimeout>;
  info(message: string, title?: string): void;
  checkNow(slot: KeySlot): void;
}

/** '처음이에요'의 단계 순서 — 화면의 단계 점과 같은 표(Rust `wizard::flow`와 같은 순서). */
function flow(env: BinanceEnv): readonly Stage[] {
  return flowOf(env).stages;
}

/** 바이낸스 머리줄 '… 열기' 단추 이름. 국내는 거래소 이름. */
const HOME_LABELS: Record<Exclude<BinanceEnv, "">, string> = { testnet: "테스트넷", demo: "데모", live: "바이낸스" };

const NO_NOTES = { auth: "", ip: "", order: "" };

export function createWizard(m: Model, { later, info, checkNow }: Deps) {
  function open(t: WizardTarget, stage: Stage = "ask") {
    // 칸이 곧 사이트 — 사이트 이름은 칸 이름과 같음.
    const site = SLOT_LABELS[t.slot];
    const home = t.env ? HOME_LABELS[t.env] : site;
    m.wizard = {
      exchange: t.exchange,
      slot: t.slot,
      env: t.env,
      market: t.market,
      label: site,
      homeLabel: home,
      stage,
      path: flow(t.env).includes(stage) ? "new" : stage === "existing" ? "existing" : "",
      ip: { status: "idle", address: "", note: "", changed: false },
      slots: { access: false, secret: false },
      fieldLabels: FIELD_LABELS[t.exchange],
      checks: { auth: "wait", ip: "wait", order: "wait" },
      headline: "",
      notes: NO_NOTES,
      keysReady: false,
      reason: "",
      problem: "",
      watchingClipboard: false,
      remembered: m.keys[t.slot].remembered,
      rememberSupported: m.storage.status !== "unsupported",
    };
    go(stage);
  }

  function go(stage: Stage) {
    const w = m.wizard;
    if (!w) return;
    if (stage === "ip" && !flow(w.env).includes("ip")) return;
    w.stage = stage;
    if (flow(w.env).includes(stage)) w.path = "new";
    if (stage === "existing") w.path = "existing";
    w.watchingClipboard = stage === "perm" || stage === "keys" || stage === "existing";
    if (stage === "ip" && w.ip.status !== "ok") refreshIp();
    if (stage === "checking") checkNow(w.slot);
    if (stage === "keys" && !m.keys[w.slot].access) simulateClipboard();
  }

  /** Rust `wizard::flow::back_stage`와 같음 — 확인·결과에서 '처음이에요'면 그 사이트의 마지막 단계로. */
  function back(w: WizardState): Stage {
    const steps = flow(w.env);
    const i = steps.indexOf(w.stage);
    if (i > 0) return steps[i - 1];
    if (i === 0 || w.stage === "existing") return "ask";
    const result = w.stage === "checking" || w.stage === "result_ok" || w.stage === "result_fail";
    if (result && w.path === "new") return steps[steps.length - 1];
    return w.path === "existing" ? "existing" : "keys";
  }

  function refreshIp() {
    const w = m.wizard;
    if (!w) return;
    w.ip = { ...w.ip, status: "loading" };
    later(900, () => {
      if (m.wizard) m.wizard.ip = { status: "ok", address: "211.234.101.57", note: "", changed: false };
    });
  }

  /** 거래소 발급 창에서 [복사] 두 번을 누른 것처럼 칸을 채움. 실거래 바이낸스는 그 뒤 허용 IP 단계로. */
  function simulateClipboard() {
    later(2600, () => {
      const w = m.wizard;
      if (!w || w.stage !== "keys") return;
      m.keys[w.slot].access = "a8Kq3Zt0mock4Rk2Lp9WqZx7";
      later(1600, () => {
        const w2 = m.wizard;
        if (!w2 || w2.stage !== "keys") return;
        m.keys[w2.slot].secret = "Wm2xP7LbmockSecretValue1234";
        go(w2.env === "live" ? "ip" : "checking");
      });
    });
  }

  /** 칸 확인이 끝났을 때 — 도우미가 그 칸을 검사 중이면 결과 단계로. */
  function onChecked(slot: KeySlot) {
    const w = m.wizard;
    if (!w || w.slot !== slot || w.stage !== "checking") return;
    const k = m.keys[slot];
    const ok = k.status === "connected";
    const practice = w.env === "testnet" || w.env === "demo";
    w.stage = ok ? "result_ok" : "result_fail";
    w.notes = NO_NOTES;
    if (ok) {
      w.checks = practice
        ? { auth: "ok", ip: "na", order: "na" }
        : { auth: "ok", ip: "ok", order: slot === "bithumb" ? "na" : "ok" };
      if (practice) w.notes = { ...NO_NOTES, ip: "필요 없음" };
    } else if (k.problem === "ip") {
      w.checks =
        w.env === "live" ? { auth: "wait", ip: "bad", order: "wait" } : { auth: "ok", ip: "bad", order: "wait" };
      if (w.env === "live") w.notes = { ...NO_NOTES, ip: "IP 또는 키 불일치" };
    } else {
      w.checks = { auth: "bad", ip: practice ? "na" : "wait", order: "wait" };
      if (practice) w.notes = { ...NO_NOTES, ip: "필요 없음" };
    }
    w.problem = ok ? "" : k.problem;
    w.headline = ok ? "연결 완료" : k.problem === "ip" ? "허용 IP 확인 필요" : "키 확인 필요";
    w.reason = ok
      ? ""
      : k.problem === "ip"
        ? "이 기기 주소가 거래소에 등록된 허용 IP와 달라요."
        : "키가 맞지 않아요. 다시 복사해 넣어 주세요.";
    w.watchingClipboard = false;
  }

  const commands: Record<string, (args: Record<string, unknown>) => unknown> = {
    wizard_choose: ({ path }) => {
      if (!m.wizard) return;
      m.wizard.path = path as "new" | "existing";
      go(path === "new" ? flow(m.wizard.env)[0] : "existing");
      if (path === "existing") info("(미리보기) 거래소 API 관리 페이지를 열었어요.", "API 관리");
    },
    wizard_go: ({ stage }) => go(stage as Stage),
    wizard_back: () => {
      if (m.wizard) go(m.wizard.stage === "ask" ? "ask" : back(m.wizard));
    },
    wizard_close: () => {
      m.wizard = null;
    },
    wizard_refresh_ip: () => refreshIp(),
    wizard_copy_ip: () => undefined,
    wizard_copy_and_open: () => {
      info("(미리보기) 주소를 복사하고 API 관리 페이지를 열었어요.", "API 관리");
      // 바이낸스 실거래: 허용 IP·주문 권한을 걸고 돌아온 것처럼 바로 확인함.
      const w = m.wizard;
      if (w?.env === "live" && w.stage === "ip") later(1200, () => m.wizard?.stage === "ip" && go("checking"));
    },
    wizard_open_api_page: () => info("(미리보기) 거래소 API 관리 페이지를 열었어요.", "API 관리"),
    wizard_open_home: () => info("(미리보기) 거래소 첫 화면을 열었어요.", "거래소"),
    wizard_recheck: () => go("checking"),
    wizard_recopy: () => {
      const w = m.wizard;
      if (!w) return;
      Object.assign(m.keys[w.slot], { access: "", secret: "", status: "empty" });
      go(w.path === "existing" ? "existing" : "keys");
    },
    wizard_set_key: ({ field, value }) => {
      const w = m.wizard;
      if (!w) return;
      const k = m.keys[w.slot];
      k[field as KeyField] = String(value ?? "").trim();
      k.status = k.access && k.secret ? "unchecked" : "empty";
    },
    wizard_set_remember: ({ remember }) => {
      if (m.wizard) m.keys[m.wizard.slot].remembered = Boolean(remember);
    },
  };

  return { open, onChecked, commands };
}
