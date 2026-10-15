// mock 연결 도우미 — 단계 이동·공인 IP 조회·클립보드 자동 채우기·연결 확인 결과. 키는 그 거래소 칸(m.keys)에 넣음.
import { EXCHANGE_LABELS, FIELD_LABELS, type Model } from "./mockData";
import type { KeyField, KeySlot, WizardState } from "./types";

type Stage = WizardState["stage"];
type Domestic = "upbit" | "bithumb";

interface Deps {
  later(ms: number, fn: () => void): ReturnType<typeof setTimeout>;
  info(message: string, title?: string): void;
  checkNow(slot: KeySlot): void;
}

const back: Partial<Record<Stage, (w: WizardState) => Stage>> = {
  ip: () => "ask",
  perm: () => "ip",
  keys: () => "perm",
  existing: () => "ask",
  manual: (w) => (w.path === "existing" ? "existing" : "keys"),
  checking: (w) => (w.path === "existing" ? "existing" : "keys"),
  result_ok: (w) => (w.path === "existing" ? "existing" : "keys"),
  result_fail: (w) => (w.path === "existing" ? "existing" : "keys"),
};

export function createWizard(m: Model, { later, info, checkNow }: Deps) {
  function open(exchange: Domestic, stage: Stage = "ask") {
    m.wizard = {
      exchange,
      label: EXCHANGE_LABELS[exchange],
      stage,
      path: stage === "ip" || stage === "perm" || stage === "keys" ? "new" : stage === "existing" ? "existing" : "",
      ip: { status: "idle", address: "", note: "", changed: false },
      slots: { access: false, secret: false },
      fieldLabels: FIELD_LABELS[exchange],
      checks: { auth: "wait", ip: "wait", order: "wait" },
      headline: "",
      reason: "",
      problem: "",
      watchingClipboard: false,
      remembered: m.keys[exchange].remembered,
      rememberSupported: m.storage.status !== "unsupported",
    };
    go(stage);
  }

  function go(stage: Stage) {
    const w = m.wizard;
    if (!w) return;
    w.stage = stage;
    if (stage === "ip" || stage === "perm" || stage === "keys") w.path = "new";
    if (stage === "existing") w.path = "existing";
    w.watchingClipboard = stage === "perm" || stage === "keys" || stage === "existing";
    if (stage === "ip" && w.ip.status !== "ok") refreshIp();
    if (stage === "checking") checkNow(w.exchange);
    if (stage === "keys" && !m.keys[w.exchange].access) simulateClipboard();
  }

  function refreshIp() {
    const w = m.wizard;
    if (!w) return;
    w.ip = { ...w.ip, status: "loading" };
    later(900, () => {
      if (m.wizard) m.wizard.ip = { status: "ok", address: "211.234.101.57", note: "", changed: false };
    });
  }

  /** 거래소 발급 창에서 [복사] 두 번을 누른 것처럼 칸을 채움. */
  function simulateClipboard() {
    later(2600, () => {
      const w = m.wizard;
      if (!w || w.stage !== "keys") return;
      m.keys[w.exchange].access = "a8Kq3Zt0mock4Rk2Lp9WqZx7";
      later(1600, () => {
        const w2 = m.wizard;
        if (!w2 || w2.stage !== "keys") return;
        m.keys[w2.exchange].secret = "Wm2xP7LbmockSecretValue1234";
        go("checking");
      });
    });
  }

  /** 칸 확인이 끝났을 때 — 도우미가 그 거래소를 검사 중이면 결과 단계로. */
  function onChecked(slot: KeySlot) {
    const w = m.wizard;
    if (!w || w.exchange !== slot || w.stage !== "checking") return;
    const k = m.keys[slot];
    const ok = k.status === "connected";
    w.stage = ok ? "result_ok" : "result_fail";
    w.checks = ok
      ? { auth: "ok", ip: "ok", order: slot === "upbit" ? "ok" : "na" }
      : k.problem === "ip"
        ? { auth: "ok", ip: "bad", order: "wait" }
        : { auth: "bad", ip: "wait", order: "wait" };
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
    wizard_open: ({ exchange, stage }) => {
      if (exchange !== "upbit" && exchange !== "bithumb")
        throw "연결 도우미는 업비트 · 빗썸용이에요. 바이낸스는 설정에서 키를 직접 입력해요.";
      open(exchange, (stage as Stage) || "ask");
    },
    wizard_choose: ({ path }) => {
      if (!m.wizard) return;
      m.wizard.path = path as "new" | "existing";
      go(path === "new" ? "ip" : "existing");
      if (path === "existing") info("(미리보기) 거래소 API 관리 페이지를 열었어요.", "API 관리");
    },
    wizard_go: ({ stage }) => go(stage as Stage),
    wizard_back: () => {
      if (m.wizard) go(back[m.wizard.stage]?.(m.wizard) ?? "ask");
    },
    wizard_close: () => {
      m.wizard = null;
    },
    wizard_refresh_ip: () => refreshIp(),
    wizard_copy_ip: () => undefined,
    wizard_copy_and_open: () => info("(미리보기) 주소를 복사하고 API 관리 페이지를 열었어요.", "API 관리"),
    wizard_open_api_page: () => info("(미리보기) 거래소 API 관리 페이지를 열었어요.", "API 관리"),
    wizard_open_home: () => info("(미리보기) 거래소 첫 화면을 열었어요.", "거래소"),
    wizard_recheck: () => go("checking"),
    wizard_recopy: () => {
      const w = m.wizard;
      if (!w) return;
      Object.assign(m.keys[w.exchange], { access: "", secret: "", status: "empty" });
      go(w.path === "existing" ? "existing" : "keys");
    },
    wizard_set_key: ({ field, value }) => {
      const w = m.wizard;
      if (!w) return;
      const k = m.keys[w.exchange];
      k[field as KeyField] = String(value ?? "").trim();
      k.status = k.access && k.secret ? "unchecked" : "empty";
    },
    wizard_set_remember: ({ remember }) => {
      if (m.wizard) m.keys[m.wizard.exchange].remembered = Boolean(remember);
    },
  };

  return { open, onChecked, commands };
}
