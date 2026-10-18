import { afterEach, describe, expect, it } from "vitest";
import { readThemePref, resolveTheme, THEME_KEY } from "./theme";

afterEach(() => window.localStorage.clear());

describe("화면 테마", () => {
  it("시스템이면 OS 설정을, 라이트 · 다크면 그대로", () => {
    expect(resolveTheme("system", true)).toBe("dark");
    expect(resolveTheme("system", false)).toBe("light");
    expect(resolveTheme("light", true)).toBe("light");
    expect(resolveTheme("dark", false)).toBe("dark");
  });
  it("저장된 값이 없거나 이상하면 시스템", () => {
    expect(readThemePref()).toBe("system");
    window.localStorage.setItem(THEME_KEY, "dark");
    expect(readThemePref()).toBe("dark");
    window.localStorage.setItem(THEME_KEY, "blue");
    expect(readThemePref()).toBe("system");
  });
});
