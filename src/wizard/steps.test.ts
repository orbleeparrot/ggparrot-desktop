import { describe, expect, it } from "vitest";
import type { BinanceEnv, WizardState } from "../types";
import { stillFrame } from "./screens/ScenePlayer";
import { SCENES } from "./screens";
import { footFor, nextStage, stageTitle, stepStates } from "./steps";

function wizard(
  stage: WizardState["stage"],
  path: WizardState["path"] = "new",
  slots = { access: false, secret: false },
  env: BinanceEnv = "",
): WizardState {
  return {
    exchange: env ? "binance" : "upbit",
    slot: ({ "": "upbit", testnet: "binance_testnet", demo: "binance_demo", live: "binance" } as const)[env],
    env,
    market: env === "demo" ? "futures" : "spot",
    label: "업비트",
    homeLabel: "업비트",
    stage,
    path,
    ip: { status: "ok", address: "1.2.3.4", note: "", changed: false },
    slots,
    fieldLabels: ["Access Key", "Secret Key"],
    checks: { auth: "wait", ip: "wait", order: "wait" },
    headline: "",
    notes: { auth: "", ip: "", order: "" },
    keysReady: slots.access && slots.secret,
    reason: "",
    problem: "",
    watchingClipboard: false,
    remembered: false,
    rememberSupported: true,
  };
}

describe("stepStates", () => {
  it("처음 발급은 네 칸", () => {
    expect(stepStates(wizard("perm")).map((s) => s.state)).toEqual(["done", "on", "todo", "todo"]);
    expect(stepStates(wizard("perm")).map((s) => s.label)).toEqual(["허용 IP", "권한·발급", "키 복사", "연결 확인"]);
    expect(stepStates(wizard("result_ok")).every((s) => s.state === "done")).toBe(true);
  });
  it("기존 키는 두 칸, 질문 화면은 점 없음", () => {
    expect(stepStates(wizard("existing", "existing")).map((s) => [s.label, s.state])).toEqual([
      ["기존 키 넣기", "on"],
      ["연결 확인", "todo"],
    ]);
    expect(stepStates(wizard("ask", ""))).toEqual([]);
  });
});

describe("바이낸스 단계", () => {
  it("테스트넷 · 데모는 키 만들기 · 키 복사 두 단계, 허용 IP 단계가 없음", () => {
    for (const env of ["testnet", "demo"] as const) {
      expect(stepStates(wizard("perm", "new", undefined, env)).map((s) => [s.label, s.state])).toEqual([
        ["키 만들기", "on"],
        ["키 복사", "todo"],
        ["연결 확인", "todo"],
      ]);
      expect(nextStage(wizard("keys", "new", undefined, env))).toBeUndefined();
    }
  });
  it("실거래는 키를 만든 뒤 허용 IP · 주문 권한, 그 단계에서 바로 연결 확인", () => {
    expect(stepStates(wizard("ip", "new", undefined, "live")).map((s) => s.state)).toEqual([
      "done",
      "done",
      "on",
      "todo",
    ]);
    expect(nextStage(wizard("keys", "new", undefined, "live"))).toBe("ip");
    expect(stageTitle(wizard("ip", "new", undefined, "live"))).toBe("허용 IP · 주문 권한");
    expect(stageTitle(wizard("perm", "new", undefined, "demo"))).toBe("키 만들기");
    expect(footFor(wizard("ip", "new", undefined, "live")).right).toMatchObject({ action: "check", enabled: false });
    const ready = wizard("ip", "new", { access: true, secret: true }, "live");
    expect(footFor(ready).right).toMatchObject({ label: "연결 확인", enabled: true });
  });
  it("국내는 그대로 허용 IP부터", () => {
    expect(nextStage(wizard("ip"))).toBe("perm");
    expect(footFor(wizard("ip")).right).toMatchObject({ action: "next" });
    expect(stageTitle(wizard("perm"))).toBe("권한 선택 · 키 발급");
  });
});

describe("footFor", () => {
  it("질문 화면은 닫기만", () => {
    expect(footFor(wizard("ask", ""))).toEqual({ left: { label: "닫기", action: "close", enabled: true } });
  });
  it("직접 입력의 검사는 두 칸이 차야 켜진다", () => {
    expect(footFor(wizard("manual")).right?.enabled).toBe(false);
    expect(footFor(wizard("manual", "new", { access: true, secret: true })).right).toMatchObject({
      label: "연결 확인",
      enabled: true,
    });
  });
  it("완료는 연결 완료일 때만 켜진다", () => {
    expect(footFor(wizard("result_fail")).right).toMatchObject({ label: "닫기", action: "close", enabled: true });
    expect(footFor(wizard("result_ok")).right).toMatchObject({ label: "완료", enabled: true });
  });
});

describe("거래소 화면 그림", () => {
  it("모든 장면의 움직임은 있는 목표만 가리킨다", () => {
    for (const site of Object.keys(SCENES) as (keyof typeof SCENES)[]) {
      const exchange = site;
      for (const scene of Object.values(SCENES[site])) {
        for (const step of scene.steps) {
          if (step.t === "move") expect(scene.targets[step.to], `${exchange} ${step.to}`).toBeDefined();
        }
      }
    }
  });
  it("움직임 줄이기 한 장은 겹친 창을 연 채로 IP 를 찍어 둔다", () => {
    const s = stillFrame(SCENES.upbit.fix, "211.234.101.57");
    expect(s.layers.has("ovl")).toBe(true);
    expect(s.typed.in).toBe("211.234.101.57");
    expect(stillFrame(SCENES.bithumb.perm, "").on.has("t6")).toBe(true);
  });
  it("바이낸스 그림은 사이트마다 있고, 연습 환경에는 허용 IP 그림이 없다", () => {
    expect(Object.keys(SCENES["binance:testnet"]).sort()).toEqual(["existing", "keys", "perm"]);
    expect(Object.keys(SCENES["binance:demo"]).sort()).toEqual(["existing", "keys", "perm"]);
    expect(Object.keys(SCENES["binance:live"]).sort()).toEqual(["existing", "keys", "perm", "restrict"]);
    // 이름 칸에는 IP가 아니라 정한 글자를 찍음.
    expect(stillFrame(SCENES["binance:testnet"].perm, "1.2.3.4").typed.desc).toBe("gg_parrot");
    const restrict = stillFrame(SCENES["binance:live"].restrict, "1.2.3.4");
    expect([restrict.layers.has("edit"), restrict.on.has("restrict"), restrict.on.has("trade")]).toEqual([
      true,
      true,
      true,
    ]);
  });
});
