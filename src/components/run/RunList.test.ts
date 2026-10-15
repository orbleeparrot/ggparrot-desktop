import { describe, expect, it } from "vitest";
import { dropIndex } from "./RunList";

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
