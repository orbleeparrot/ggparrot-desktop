import { describe, expect, it } from "vitest";
import type { WizardState } from "../types";
import { stillFrame } from "./screens/ScenePlayer";
import { SCENES } from "./screens";
import { footFor, stepStates } from "./steps";

function wizard(
  stage: WizardState["stage"],
  path: WizardState["path"] = "new",
  slots = { access: false, secret: false },
): WizardState {
  return {
    exchange: "upbit",
    label: "업비트",
    stage,
    path,
    ip: { status: "ok", address: "1.2.3.4", note: "", changed: false },
    slots,
    fieldLabels: ["Access Key", "Secret Key"],
    checks: { auth: "wait", ip: "wait", order: "wait" },
    headline: "",
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
    for (const exchange of ["upbit", "bithumb"] as const) {
      for (const scene of Object.values(SCENES[exchange])) {
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
});
