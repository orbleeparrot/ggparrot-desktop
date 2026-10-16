import { describe, expect, it } from "vitest";
import { isReloadKey, keepNativeMenu } from "./appShell";

describe("앱 창의 기본 메뉴·새로고침", () => {
  it("입력 칸·편집 영역·골라 둔 글자에서만 기본 메뉴를 둠", () => {
    const input = document.createElement("input");
    const editable = document.createElement("div");
    editable.setAttribute("contenteditable", "true");
    const inner = document.createElement("span");
    editable.appendChild(inner);
    const plain = document.createElement("div");
    expect(keepNativeMenu(input, "")).toBe(true);
    expect(keepNativeMenu(inner, "")).toBe(true);
    expect(keepNativeMenu(plain, "")).toBe(false);
    expect(keepNativeMenu(plain, "BTCUSDT")).toBe(true);
    expect(keepNativeMenu(null, "")).toBe(false);
  });

  it("새로고침 단축키를 알아봄", () => {
    expect(isReloadKey({ key: "r", metaKey: true, ctrlKey: false })).toBe(true);
    expect(isReloadKey({ key: "R", metaKey: false, ctrlKey: true })).toBe(true);
    expect(isReloadKey({ key: "F5", metaKey: false, ctrlKey: false })).toBe(true);
    expect(isReloadKey({ key: "r", metaKey: false, ctrlKey: false })).toBe(false);
    expect(isReloadKey({ key: "c", metaKey: true, ctrlKey: false })).toBe(false);
  });
});
