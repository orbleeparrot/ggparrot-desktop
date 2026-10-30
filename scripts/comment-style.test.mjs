import { execFileSync } from "node:child_process";
import { existsSync, readFileSync } from "node:fs";
import { describe, expect, it } from "vitest";
import {
  RULES,
  cssComments,
  isTarget,
  jsComments,
  markdownLines,
  problems,
  rustLikeComments,
  scan,
} from "./comment-style.mjs";

// 주석·문서 문체(docs/conventions.md 1.1) 검사.
// - 테스트는 jsdom에서 돌아 import.meta.url이 file: 주소가 아님 — 저장소 루트(npm test 위치) 기준으로 읽음.
// - 앱 tsconfig에는 Node 타입이 없어 scripts/에 둠.

describe("규칙", () => {
  it.each([
    ["실행 슬롯에 반영한다.", ["끝맺음"]],
    ["서버가 정했다 — 그래서 막음.", ["끝맺음"]],
    ["값이 null이면 끔. 그 규칙을 끈다", ["끝맺음"]],
    ["바뀐 칸만 보내요.", ["끝맺음"]],
    ["그리드 · 마틴게일", ["가운뎃점"]],
    ["`fields` 가 바뀔 때마다 돎.", ["조사"]],
    ["heartbeat 의 settings로", ["조사"]],
    ["직접 해 보다", ["끝맺음"]],
  ])("%s → %j", (text, ids) => {
    expect(problems(text)).toEqual(ids);
  });

  it.each([
    "실행 슬롯에 반영함.",
    "트레일이 켜졌는지.",
    "heartbeat마다 새 배열이 옴.",
    '화면에는 "바뀐 칸만 보내요"라고 보임.',
    "단추 글자([복사했어요] ↔ 접기)를 겹쳐 둠.",
    "그리드·마틴게일, `exit_id`를, README에, 18임.",
    "레벨 19 이상만 엶.",
    "'되돌릴 수 없어요' 줄을 보임.",
    "머리 줄 간격 10px보다",
  ])("통과: %s", (text) => {
    expect(problems(text)).toEqual([]);
  });

  const texts = (lines) => lines.map((l) => [l.line, l.text.trim()]);

  it("JS·TS — 정규식 리터럴 안의 따옴표에 밀리지 않음", () => {
    const src = [
      "const re = /\"[^\"]*\"|'[^']*'/g; // 첫째임",
      'const url = "https://a.b"; /* 둘째',
      "   줄임 */",
    ].join("\n");
    expect(texts(jsComments("x.ts", src))).toEqual([
      [1, "첫째임"],
      [2, "둘째"],
      [3, "줄임"],
    ]);
  });

  it("Rust — 문자 리터럴·수명·raw 문자열·중첩 블록 주석·이스케이프 줄바꿈", () => {
    const src = [
      "let q = '\\\"'; let r = '\"'; // 하나임",
      "fn f<'a>(x: &'a str) {} // 둘임",
      'let raw = r#"// 주석 아님 "# ; // 셋임',
      'let s = "이어지는 \\',
      '  글자"; // 넷임',
      "/* 바깥 /* 안 */ 여전히 주석임 */ let z = 1; // 다섯임",
    ].join("\n");
    expect(texts(rustLikeComments(src))).toEqual([
      [1, "하나임"],
      [2, "둘임"],
      [3, "셋임"],
      [5, "넷임"],
      [6, "바깥 /* 안 */ 여전히 주석임"],
      [6, "다섯임"],
    ]);
  });

  it("CSS — 블록 주석만(url 안의 // 는 주석이 아님)", () => {
    const src = '.a { background: url(https://a.b/c.png); content: "/* 아님 */"; } /* 주석임 */';
    expect(texts(cssComments(src))).toEqual([[1, "주석임"]]);
  });

  it("마크다운 — 목록 안에 들여쓴 코드 블록은 건너뜀", () => {
    const md = ["- 설치함:", "  ```bash", "  # 받는다", "  ```", "- 끝임."].join("\n");
    expect(markdownLines("docs/a.md", md).map((l) => l.text)).toEqual(["- 설치함:", "- 끝임."]);
  });

  it("CHANGELOG는 머리말과 정해진 버전부터의 `### 개발`만 봄", () => {
    const md = [
      "- 머리말임.",
      "## [0.22.1]",
      "### 수정",
      "- 화면을 고쳤어요.",
      "### 개발",
      "- 검사를 더함.",
      "## [0.20.9]",
      "### 개발",
      "- 예전 기록은 그대로예요.",
    ].join("\n");
    expect(markdownLines("CHANGELOG.md", md).map((l) => l.text)).toEqual(["- 머리말임.", "- 검사를 더함."]);
  });

  it("CHANGELOG `## [Unreleased]`는 가장 새 버전 — 사용자 분류는 보지 않음", () => {
    const md = ["## [Unreleased]", "### 추가", "- 새 단추가 생겼어요.", "### 개발", "- 단추를 더함."].join("\n");
    expect(markdownLines("CHANGELOG.md", md).map((l) => l.text)).toEqual(["- 단추를 더함."]);
  });

  it("규칙마다 이유가 있음 — 실패 문구에 그대로 나감", () => {
    for (const r of RULES) expect(r.why.length).toBeGreaterThan(10);
  });
});

describe("저장소", () => {
  it("코드 주석·문서가 개조식임(docs/conventions.md 1.1)", () => {
    // 아직 add 안 한 새 파일도 봄(무시 목록 제외). 지웠지만 아직 git rm 안 한 파일은 건너뜀.
    const files = execFileSync("git", ["ls-files", "--cached", "--others", "--exclude-standard"], { encoding: "utf8" })
      .split("\n")
      .filter((f) => isTarget(f) && existsSync(f));
    expect(files.length, "git ls-files로 파일을 못 읽음").toBeGreaterThan(100);
    const found = scan(files, (f) => readFileSync(f, "utf8"));
    const why = Object.fromEntries(RULES.map((r) => [r.id, r.why]));
    const report = found.map((p) => `${p.file}:${p.line} [${p.rule}] ${p.text}\n    → ${why[p.rule]}`);
    expect(report, "화면 문구를 옮겨 적는 자리면 따옴표로 감쌈").toEqual([]);
  });
});
