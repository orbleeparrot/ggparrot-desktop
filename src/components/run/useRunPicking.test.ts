import { describe, expect, it } from "vitest";
import { hitIds, pickArea, rangeIds } from "./useRunPicking";

const box = (id: string, top: number, left = 0, width = 300, height = 34) => ({
  id,
  rect: { left, top, right: left + width, bottom: top + height },
});

describe("pickArea", () => {
  it("누른 자리와 지금 자리로 사각형 — 위로 끌어도 같음", () => {
    expect(pickArea({ x: 10, y: 200 }, 0, { x: 40, y: 100 })).toEqual({ left: 10, right: 40, top: 100, bottom: 200 });
  });
  it("끄는 사이 스크롤한 만큼 누른 자리가 위로 올라감", () => {
    // 200에서 눌렀고 목록이 120 내려감 → 누른 줄은 지금 화면 80
    expect(pickArea({ x: 10, y: 200 }, 120, { x: 10, y: 500 })).toEqual({ left: 10, right: 10, top: 80, bottom: 500 });
  });
});

describe("hitIds", () => {
  const rows = [box("a", 0), box("b", 40), box("c", 80), box("d", 120)];
  it("한 줄 목록 — 위아래로 끈 범위의 줄", () => {
    expect(hitIds(rows, { left: 50, right: 50, top: 50, bottom: 95 })).toEqual(["b", "c"]);
  });
  it("가장자리만 닿아도 셈", () => {
    expect(hitIds(rows, { left: 50, right: 50, top: 34, bottom: 34 })).toEqual(["a"]);
  });
  it("두 칸 격자 — 끈 모양 그대로의 덩어리만", () => {
    const grid = [box("l1", 0, 0, 150), box("r1", 0, 160, 150), box("l2", 40, 0, 150), box("r2", 40, 160, 150)];
    // 왼쪽 칸만 위아래로 끌면 왼쪽 둘
    expect(hitIds(grid, { left: 20, right: 30, top: 10, bottom: 50 })).toEqual(["l1", "l2"]);
    // 대각선으로 끌면 넷
    expect(hitIds(grid, { left: 20, right: 200, top: 10, bottom: 50 })).toEqual(["l1", "r1", "l2", "r2"]);
  });
});

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
