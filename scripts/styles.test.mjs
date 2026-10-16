import { readdirSync, readFileSync } from "node:fs";
import { describe, expect, it } from "vitest";

// src/styles의 CSS 규칙 검사(앱 tsconfig에는 Node 타입이 없어 scripts/에 둠).
// 테스트는 jsdom에서 돌아 import.meta.url이 file: 주소가 아님 — 저장소 루트(npm test 위치) 기준으로 읽음.
const DIR = "src/styles";
const sheets = readdirSync(DIR)
  .filter((f) => f.endsWith(".css"))
  .map((f) => ({ file: f, css: readFileSync(`${DIR}/${f}`, "utf8") }));

/** `선택자 { … }` 규칙들(@media 안쪽 포함, 중첩 없는 평평한 CSS 기준). */
function rules(css) {
  return [...css.replace(/\/\*[\s\S]*?\*\//g, "").matchAll(/([^{}]+)\{([^{}]*)\}/g)].map((m) => ({
    selector: m[1].trim(),
    body: m[2],
  }));
}

describe("스크롤 영역", () => {
  it("문서(html·body)는 스크롤하지 않음 — Windows 이중 스크롤·머리줄 가림 방지", () => {
    const base = sheets.find((s) => s.file === "base.css").css;
    // 넓은 창에서만 막음 — 좁은 창은 문서 하나로 스크롤하는 배치라 막으면 아래로 못 내려감.
    const wide = /@media \(min-width: 900px\)\s*\{\s*html,\s*body\s*\{[^}]*overflow:\s*clip/;
    expect(base, "base.css의 @media (min-width: 900px) 안에 html, body { overflow: clip }").toMatch(wide);
    const everywhere = rules(base).filter(
      (r) => /^html,\s*body$/.test(r.selector) && /overflow:\s*(clip|hidden)/.test(r.body),
    );
    expect(everywhere.length, "문서 스크롤 막기는 넓은 창 한 곳에만").toBe(1);
  });

  it("스크롤 영역은 기준 상자(position: relative) — 안의 .sr-only가 문서로 새지 않게", () => {
    const loose = sheets.flatMap(({ file, css }) =>
      rules(css)
        .filter(
          (r) => /overflow(-y)?:\s*auto/.test(r.body) && !/position:\s*(relative|absolute|fixed|sticky)/.test(r.body),
        )
        .map((r) => `${file}: ${r.selector}`),
    );
    expect(loose).toEqual([]);
  });
});
