import { readFileSync } from "node:fs";
import { describe, expect, it } from "vitest";
import { parseReleaseNotes, WEB_ONLY_MARKER as APP_MARKER } from "../src/state/releaseNotes";
import { ASSET_PATTERN, INSTALLERS, assetName } from "./bundle-names.mjs";
import { WEB_ONLY_MARKER, buildNotes, installFooter, markReleased } from "./release-notes-lib.mjs";

const log = `# 변경 기록

## [0.2.2] - 2026-11-03

### 추가

- 새 기능 B

### 개발

- CI 정리

## [0.2.1] - 미배포

첫 배포.

### 수정

- 버그 A

### 알림

- 기타 분류

### 추가

- 새 기능 A

## [0.2.0] - 2026-11-01

### 추가

- 지난 배포
`;

describe("buildNotes", () => {
  it("배포하지 않은 칸을 분류별로 합치고, 소개 글은 모으고, 개발은 뺀다", () => {
    expect(buildNotes(log, "0.2.2")).toBe(
      "첫 배포.\n\n### 추가\n\n- 새 기능 B\n- 새 기능 A\n\n### 수정\n\n- 버그 A\n\n### 알림\n\n- 기타 분류",
    );
  });

  it("날짜가 없으면 배포를 막고, preview는 받는다", () => {
    expect(() => buildNotes(log, "0.2.1")).toThrow("배포 날짜");
    expect(buildNotes(log, "0.2.1", { preview: true })).toContain("### 수정\n\n- 버그 A");
  });

  it("없는 버전은 막는다", () => {
    expect(() => buildNotes(log, "9.9.9")).toThrow("칸이 없어요");
  });
});

describe("markReleased", () => {
  const draft = log.replace("## [0.2.2] - 2026-11-03", "## [0.2.2] - 미배포");

  it("배포할 칸에 날짜를 달고 함께 나가는 칸은 '배포하지 않음'으로", () => {
    const marked = markReleased(draft, "0.2.2", "2026-11-05");
    expect(marked).toContain("## [0.2.2] - 2026-11-05");
    expect(marked).toContain("## [0.2.1] - 배포하지 않음(0.2.2에 포함)");
    expect(marked).toContain("## [0.2.0] - 2026-11-01");
    // 다음 배포는 0.2.2 아래로 내려가지 않음
    const next = marked.replace("# 변경 기록\n", "# 변경 기록\n\n## [0.2.3] - 2026-11-09\n\n### 수정\n\n- 버그 C\n");
    expect(buildNotes(next, "0.2.3")).toBe("### 수정\n\n- 버그 C");
  });

  it("이미 배포 표시가 있거나 날짜 모양이 틀리면 막는다", () => {
    expect(() => markReleased(log, "0.2.2", "2026-11-05")).toThrow("이미 배포");
    expect(() => markReleased(draft, "0.2.2", "11/05")).toThrow("YYYY-MM-DD");
  });
});

describe("설치 안내와 앱의 '바뀐 점'", () => {
  it("앱은 웹 전용 표시 앞까지만 읽는다(표시가 양쪽에서 같음)", () => {
    expect(APP_MARKER).toBe(WEB_ONLY_MARKER);
    const body = buildNotes(log, "0.2.2") + installFooter("ggparrot-desktop", "0.2.2");
    const text = parseReleaseNotes(body)
      .map((l) => l.text)
      .join("\n");
    expect(text).toContain("새 기능 B");
    expect(text).not.toContain("설치:");
  });

  it("설치 파일 이름이 release.yml의 패턴과 같다", () => {
    // 테스트는 jsdom에서 돌아 import.meta.url이 file: 주소가 아님 — 저장소 루트(npm test 위치) 기준으로 읽음.
    const yml = readFileSync(".github/workflows/release.yml", "utf8");
    expect(yml).toContain(`releaseAssetNamePattern: "${ASSET_PATTERN}"`);
    expect(assetName("ggparrot-desktop", "0.2.2", INSTALLERS.mac)).toBe("ggparrot-desktop_0.2.2_darwin_universal.dmg");
    expect(assetName("ggparrot-desktop", "0.2.2", INSTALLERS.windows)).toBe(
      "ggparrot-desktop_0.2.2_windows_x64-setup.exe",
    );
    expect(installFooter("ggparrot-desktop", "0.2.2")).toContain("ggparrot-desktop_0.2.2_windows_x64-setup.exe");
  });
});
