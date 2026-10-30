// 구동 중 설정 바꾸기 — 입력란의 글자를 서버가 받는 모양으로 바꾸는 규칙.
//
// 가장 조심할 것은 **빈 칸의 뜻**임. 끌 수 있는 칸(손절)을 비우면 '끄는 것'이고, 끌 수
// 없는 칸을 비운 것은 '아직 덜 친 것'임. 둘을 같게 다루면 사용자가 칸을 지웠다가 투자
// 비율이 0이 되거나, 손절을 끄려 했는데 그대로 남음.
import { describe, expect, it } from "vitest";
import type { LiveSettingField } from "../../types";
import { badFields, buildChanges, hasChanges, touchesPosition } from "./LiveSettings";

const money: LiveSettingField = {
  path: "params.initial_capital",
  section: "params",
  key: "initial_capital",
  label: "투자 금액",
  value: 1_000_000,
  unit: "money",
  unit_text: "USDT",
  help: "다음에 살 때 쓸 금액이에요. 지금 들고 있는 건 그대로예요.",
  affects: "next_entry",
  clearable: false,
};
const stop: LiveSettingField = {
  path: "risk.stop_loss_pct",
  section: "risk",
  key: "stop_loss_pct",
  label: "손절",
  value: 3,
  unit: "pct",
  unit_text: "%",
  help: "이만큼 손해 나면 팔아요.",
  affects: "position",
  clearable: true,
};
const ratio: LiveSettingField = {
  path: "risk.invest_ratio",
  section: "risk",
  key: "invest_ratio",
  label: "투자 비율",
  value: 1,
  unit: "ratio",
  unit_text: "",
  help: "한 번에 투자 금액의 얼마를 쓸지예요(1 = 전부). 다음에 살 때부터 적용돼요.",
  affects: "next_entry",
  clearable: false,
};
const FIELDS = [money, stop, ratio];

describe("buildChanges", () => {
  it("바뀐 칸만 담는다", () => {
    expect(
      buildChanges(FIELDS, {
        "params.initial_capital": "2,000,000",
        "risk.stop_loss_pct": "3",
        "risk.invest_ratio": "1",
      }),
    ).toEqual({ params: { initial_capital: 2_000_000 } });
  });

  it("손 안 댄 칸은 보내지 않는다", () => {
    expect(buildChanges(FIELDS, {})).toEqual({});
  });

  it("끌 수 있는 칸을 비우면 끄는 것이다", () => {
    expect(buildChanges(FIELDS, { "risk.stop_loss_pct": "" })).toEqual({ risk: { stop_loss_pct: null } });
  });

  it("이미 꺼져 있던 칸을 비운 것은 변경이 아니다", () => {
    expect(buildChanges([{ ...stop, value: null }], { "risk.stop_loss_pct": "" })).toEqual({});
  });

  it("비울 수 없는 칸을 비운 것은 0 으로 보내지 않는다", () => {
    expect(buildChanges(FIELDS, { "risk.invest_ratio": "" })).toEqual({});
  });

  it("숫자로 안 읽히는 글자는 보내지 않는다", () => {
    expect(buildChanges(FIELDS, { "risk.stop_loss_pct": "많이" })).toEqual({});
  });
});

describe("badFields", () => {
  it("손댄 칸만 본다 — 열어 두기만 해도 빨개지면 안 된다", () => {
    expect(badFields(FIELDS, {})).toEqual([]);
  });

  it("숫자가 아닌 글자와, 비울 수 없는데 비운 칸을 집는다", () => {
    expect(badFields(FIELDS, { "risk.stop_loss_pct": "많이", "risk.invest_ratio": "" }).map((f) => f.path)).toEqual([
      "risk.stop_loss_pct",
      "risk.invest_ratio",
    ]);
  });

  it("끌 수 있는 칸을 비운 것은 잘못이 아니다", () => {
    expect(badFields(FIELDS, { "risk.stop_loss_pct": "" })).toEqual([]);
  });
});

describe("hasChanges", () => {
  it("보낼 게 있는지로 단추를 가른다", () => {
    expect(hasChanges(FIELDS, { "params.initial_capital": "1,000,000" })).toBe(false);
    expect(hasChanges(FIELDS, { "params.initial_capital": "2,000,000" })).toBe(true);
  });
});

// 적용 시점을 묻는 자리가 '어느 쪽을 골라도 같다'고 말할 수 있어야 함. 무엇이 지금
// 포지션을 건드리는지는 서버가 정함(affects) — 앱이 짐작하면 두 벌이 됨.
describe("touchesPosition", () => {
  it("손절을 바꾸면 들고 있는 포지션의 청산 조건이 바뀐다", () => {
    expect(touchesPosition(FIELDS, { "risk.stop_loss_pct": "2" })).toBe(true);
  });

  it("금액·비율만 바꾸면 지금 포지션과 상관없다", () => {
    expect(touchesPosition(FIELDS, { "params.initial_capital": "2,000,000" })).toBe(false);
    expect(touchesPosition(FIELDS, { "risk.invest_ratio": "0.5" })).toBe(false);
  });

  it("손 안 댄 칸과 같은 값은 세지 않는다", () => {
    expect(touchesPosition(FIELDS, {})).toBe(false);
    expect(touchesPosition(FIELDS, { "risk.stop_loss_pct": "3" })).toBe(false);
  });

  it("둘을 같이 바꾸면 묻는다", () => {
    expect(touchesPosition(FIELDS, { "params.initial_capital": "2,000,000", "risk.stop_loss_pct": "2" })).toBe(true);
  });
});
