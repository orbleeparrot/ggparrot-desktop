// 릴리스 본문(CHANGELOG의 한 버전 칸, 마크다운) → 화면에 그릴 줄 목록.
// 제목(`### 추가`)·목록(`- `, 들여쓰기 2칸마다 한 단계)·문단만 읽음. 나머지 마크다운 기호는 글자에서 뺌.
// WEB_ONLY_MARKER 아래는 웹 릴리스 페이지용 설치 안내라 읽지 않음(scripts/release-notes-lib.mjs와 같은 표시).

/** 웹 릴리스 페이지에만 보일 부분의 시작 표시 — scripts/release-notes-lib.mjs의 WEB_ONLY_MARKER와 같음. */
export const WEB_ONLY_MARKER = "<!-- web-only -->";

export type NoteLine =
  { kind: "heading"; text: string } | { kind: "item"; text: string; level: number } | { kind: "text"; text: string };

/** 인라인 마크다운을 읽기 좋은 글자로 — 링크는 글자만, 굵게·기울임 기호는 뺌. 코드 표기(`)는 남겨 화면이 가름. */
function plainInline(text: string): string {
  return text
    .replace(/\[([^\]]+)\]\([^)]*\)/g, "$1")
    .replace(/\*\*([^*]+)\*\*/g, "$1")
    .replace(/__([^_]+)__/g, "$1")
    .trim();
}

export function parseReleaseNotes(body: string): NoteLine[] {
  const out: NoteLine[] = [];
  for (const raw of body.replace(/\r\n/g, "\n").split("\n")) {
    if (raw.trim() === WEB_ONLY_MARKER) break;
    if (!raw.trim() || /^\s*(-{3,}|\*{3,})\s*$/.test(raw)) continue;
    const heading = raw.match(/^\s*#{1,6}\s+(.*)$/);
    if (heading) {
      out.push({ kind: "heading", text: plainInline(heading[1]) });
      continue;
    }
    const item = raw.match(/^(\s*)[-*]\s+(.*)$/);
    if (item) {
      out.push({ kind: "item", text: plainInline(item[2]), level: Math.min(Math.floor(item[1].length / 2), 2) });
      continue;
    }
    out.push({ kind: "text", text: plainInline(raw) });
  }
  return out;
}

/** 글자를 코드 표기(`…`) 기준으로 나눔 — 홀수 번째가 코드. */
export function splitCode(text: string): string[] {
  return text.split("`");
}
