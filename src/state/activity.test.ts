import { describe, expect, it } from "vitest";
import { act } from "../test/fixtures";
import { countByFilter, MAX_ACTIVITY, mergeActivity, visibleActivity } from "./activity";

describe("mergeActivity", () => {
  it("id 로 중복을 버리고 순서를 지킨다", () => {
    const a = act({ id: 1 });
    const b = act({ id: 2 });
    const c = act({ id: 3 });
    const first = mergeActivity([], [a, c]);
    expect(mergeActivity(first, [b, c]).map((x) => x.id)).toEqual([1, 2, 3]);
  });
  it("시각(at)이 같아도 id(Rust 일련번호) 순서를 지킨다", () => {
    const at = 1_760_000_000_000;
    const out = mergeActivity([act({ id: 7, at })], [act({ id: 9, at }), act({ id: 8, at })]);
    expect(out.map((x) => x.id)).toEqual([7, 8, 9]);
    const late = mergeActivity(out, [act({ id: 6, at: at + 5_000 })]);
    expect(late.map((x) => x.id)).toEqual([6, 7, 8, 9]);
  });
  it("바뀐 게 없으면 같은 배열", () => {
    const list = mergeActivity([], [act({ id: 10 })]);
    expect(mergeActivity(list, [list[0]])).toBe(list);
    expect(mergeActivity(list, [])).toBe(list);
  });
  it("최근 1,000개만 남긴다", () => {
    const many = Array.from({ length: MAX_ACTIVITY + 5 }, (_, i) => act({ id: 5000 + i }));
    const out = mergeActivity([], many);
    expect(out).toHaveLength(MAX_ACTIVITY);
    expect(out[0].id).toBe(5005);
  });
});

describe("visibleActivity · 필터와 개발자 로그", () => {
  const items = [
    act({ kind: "start" }),
    act({ kind: "info", dev: true, title: "heartbeat ok" }),
    act({ kind: "signal" }),
    act({ kind: "buy" }),
    act({ kind: "blocked" }),
    act({ kind: "sell" }),
    act({ kind: "stop_loss" }),
    act({ kind: "connection" }),
    act({ kind: "error" }),
    act({ kind: "buy", dev: true, title: "BUY 체결: id=x" }),
  ];
  const kinds = (f: Parameters<typeof visibleActivity>[1], dev: boolean) =>
    visibleActivity(items, f, dev).map((a) => `${a.kind}${a.dev ? "*" : ""}`);

  it("기본은 원문 줄(dev)을 숨긴다", () => {
    expect(kinds("all", false)).toEqual([
      "start",
      "signal",
      "buy",
      "blocked",
      "sell",
      "stop_loss",
      "connection",
      "error",
    ]);
  });
  it("개발자 로그 보기를 켜면 원문 줄도 보인다", () => {
    expect(kinds("all", true)).toContain("info*");
    expect(kinds("all", true)).toHaveLength(items.length);
  });
  it("거래 · 신호 · 문제", () => {
    expect(kinds("trade", false)).toEqual(["buy", "sell", "stop_loss"]);
    expect(kinds("trade", true)).toEqual(["buy", "sell", "stop_loss", "buy*"]);
    expect(kinds("signal", false)).toEqual(["signal", "blocked"]);
    expect(kinds("problem", false)).toEqual(["connection", "error"]);
  });
  it("칩 숫자는 원문 줄을 세지 않는다", () => {
    expect(countByFilter(items)).toEqual({ all: 8, trade: 3, signal: 2, problem: 2 });
  });
});
