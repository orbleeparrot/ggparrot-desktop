import { describe, expect, it } from "vitest";
import { parseReleaseNotes, splitCode } from "./releaseNotes";

describe("parseReleaseNotes", () => {
  it("제목·목록·들여쓰기를 읽고 마크다운 기호를 빼며, 웹 전용 표시 아래 설치 안내는 읽지 않는다", () => {
    const body =
      "첫 배포.\n\n### 추가\n\n- 껄무새 웹 [열기](https://x)\n  - 머리줄 **단추**\n\n---\n\n- 구분선 뒤도 읽음\n\n<!-- web-only -->\n\n---\n\n설치: macOS `a.dmg`";
    expect(parseReleaseNotes(body)).toEqual([
      { kind: "text", text: "첫 배포." },
      { kind: "heading", text: "추가" },
      { kind: "item", text: "껄무새 웹 열기", level: 0 },
      { kind: "item", text: "머리줄 단추", level: 1 },
      { kind: "item", text: "구분선 뒤도 읽음", level: 0 },
    ]);
  });

  it("코드 표기는 홀수 조각", () => {
    expect(splitCode("파일 `a.dmg` 받기")).toEqual(["파일 ", "a.dmg", " 받기"]);
  });
});
