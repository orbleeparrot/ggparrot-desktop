import { describe, expect, it } from "vitest";
import { euro, iga } from "./particle";

describe("버전 뒤 조사", () => {
  it("이/가", () => {
    expect(iga("v0.18.3")).toBe("v0.18.3이");
    expect(iga("v0.18.2")).toBe("v0.18.2가");
    expect(iga("v0.18.10")).toBe("v0.18.10이");
  });
  it("으로/로 — ㄹ 받침(1 · 7 · 8)은 '로'", () => {
    expect(euro("v0.18.3")).toBe("v0.18.3으로");
    expect(euro("v0.18.2")).toBe("v0.18.2로");
    expect(euro("v0.18.1")).toBe("v0.18.1로");
    expect(euro("v0.18.6")).toBe("v0.18.6으로");
    expect(euro("v0.18.10")).toBe("v0.18.10으로");
  });
});
