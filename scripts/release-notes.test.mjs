import { readFileSync } from "node:fs";
import { describe, expect, it } from "vitest";
import { parseReleaseNotes, WEB_ONLY_MARKER as APP_MARKER } from "../src/state/releaseNotes";
import { ASSET_PATTERN, INSTALLERS, assetName } from "./bundle-names.mjs";
import { WEB_ONLY_MARKER, buildNotes, compareVersions, installFooter, releasedVersions } from "./release-notes-lib.mjs";

const log = `# 변경 기록

## [0.2.2]

### 추가

- 새 기능 B

### 개발

- CI 정리

## [0.2.1]

첫 배포.

### 수정

- 버그 A

### 알림

- 기타 분류

### 추가

- 새 기능 A

## [0.2.0]

### 추가

- 지난 배포
`;

describe("buildNotes — 배포 여부는 태그로", () => {
  it("지난 배포(태그) 뒤의 칸을 분류별로 합치고, 소개 글은 모으고, 개발은 뺀다", () => {
    expect(buildNotes(log, "0.2.2", ["0.2.0"])).toBe(
      "첫 배포.\n\n### 추가\n\n- 새 기능 B\n- 새 기능 A\n\n### 수정\n\n- 버그 A\n\n### 알림\n\n- 기타 분류",
    );
  });

  it("공개한 버전이 없으면 처음부터 다 합치고, 바로 앞 버전이 공개됐으면 이 칸만", () => {
    expect(buildNotes(log, "0.2.2", [])).toContain("- 지난 배포");
    expect(buildNotes(log, "0.2.2", ["0.2.0", "0.2.1"])).toBe("### 추가\n\n- 새 기능 B");
  });

  it("이미 공개한 버전도 다시 만들 수 있고, 더 높은 공개 버전은 상관없다", () => {
    expect(buildNotes(log, "0.2.1", ["0.2.0", "0.2.1", "0.2.2"])).toContain("- 버그 A");
    expect(buildNotes(log, "0.2.1", ["0.2.0", "0.2.1", "0.2.2"])).not.toContain("새 기능 B");
  });

  it("버전은 숫자로 비교한다(0.10.0 > 0.9.9)", () => {
    expect(compareVersions("0.10.0", "0.9.9")).toBeGreaterThan(0);
    const many = "## [0.10.0]\n\n### 추가\n\n- 열\n\n## [0.9.9]\n\n### 추가\n\n- 아홉\n";
    expect(buildNotes(many, "0.10.0", ["0.9.9"])).toBe("### 추가\n\n- 열");
  });

  it("없는 버전은 막는다", () => {
    expect(() => buildNotes(log, "9.9.9", [])).toThrow("칸이 없어요");
  });

  it("태그에서 v<주>.<부>.<수>만 공개 버전으로 읽는다", () => {
    expect(releasedVersions(["v0.20.1", "v0.20.0-rc1", "runner-v20", "", "v1.2.3 "])).toEqual(["0.20.1", "1.2.3"]);
  });
});

describe("설치 안내와 앱의 '바뀐 점'", () => {
  it("앱은 웹 전용 표시 앞까지만 읽는다(표시가 양쪽에서 같음)", () => {
    expect(APP_MARKER).toBe(WEB_ONLY_MARKER);
    const body = buildNotes(log, "0.2.2", ["0.2.0"]) + installFooter("ggparrot-desktop", "0.2.2");
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
