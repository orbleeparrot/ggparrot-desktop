import { describe, expect, it } from "vitest";
import { dropIndex, edgeScrollSpeed } from "./RunList";

describe("dropIndex", () => {
  it("놓을 자리 → 옮긴 뒤 번호(자기 자리를 빼고 셈)", () => {
    // [A,B,C,D]에서 A(0)를 C 앞(2)에 놓으면 [B,A,C,D] → 1
    expect(dropIndex(0, 2)).toBe(1);
    // 맨 뒤(4)에 놓으면 3
    expect(dropIndex(0, 4)).toBe(3);
    // D(3)를 맨 앞(0)에 놓으면 0
    expect(dropIndex(3, 0)).toBe(0);
    // 제자리(앞이나 바로 뒤)는 그대로
    expect(dropIndex(1, 1)).toBe(1);
    expect(dropIndex(1, 2)).toBe(1);
  });
});

describe("edgeScrollSpeed", () => {
  it("가운데는 0, 가장자리 안쪽으로 갈수록 빨라짐", () => {
    // 보이는 영역 100~500, 가장자리 48px, 최대 14
    expect(edgeScrollSpeed(300, 100, 500)).toBe(0);
    expect(edgeScrollSpeed(148, 100, 500)).toBe(0);
    expect(edgeScrollSpeed(124, 100, 500)).toBe(-7);
    expect(edgeScrollSpeed(100, 100, 500)).toBe(-14);
    expect(edgeScrollSpeed(476, 100, 500)).toBe(7);
    expect(edgeScrollSpeed(500, 100, 500)).toBe(14);
  });
  it("영역 밖으로 나가도 최대 속도를 넘지 않음", () => {
    expect(edgeScrollSpeed(20, 100, 500)).toBe(-14);
    expect(edgeScrollSpeed(900, 100, 500)).toBe(14);
  });
});
