import { describe, expect, it } from "vitest";
import { rangeIds } from "./useRunPicking";

describe("rangeIds", () => {
  const order = ["a", "b", "c", "d", "e"];
  it("양 끝 포함, 앞뒤 무관", () => {
    expect(rangeIds(order, "b", "d")).toEqual(["b", "c", "d"]);
    expect(rangeIds(order, "d", "b")).toEqual(["b", "c", "d"]);
    expect(rangeIds(order, "c", "c")).toEqual(["c"]);
  });
  it("시작이 차례에 없으면 누른 줄 하나, 누른 줄이 없으면 빈 목록", () => {
    expect(rangeIds(order, "x", "c")).toEqual(["c"]);
    expect(rangeIds(order, "a", "x")).toEqual([]);
  });
});
