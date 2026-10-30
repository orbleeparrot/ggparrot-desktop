// 개조식 검사(docs/conventions.md 1.1) — 코드 주석·문서의 끝맺음·가운뎃점·조사 띄어쓰기.
// - 사람이든 AI든 같은 기준을 지키게 `npm test`(scripts/comment-style.test.mjs)에서 돌림.
// - 따옴표("…" '…' “…” ‘…’)·대괄호([…])·코드 표기(`…`) 안은 보지 않음 — 화면 문구(해요체)나 단추 이름을 옮겨 적는 자리.
// - 주석은 언어마다 제대로 읽음: JS·TS는 oxc 파서(정규식 리터럴·JSX까지), Rust·Swift는 문자열 종류를 아는 토크나이저,
//   CSS는 블록 주석만. 줄 번호는 글자 위치에서 셈 — 문자열 안 줄바꿈·이스케이프에 밀리지 않음.
import { parseSync } from "oxc-parser";

/** JS·TS — oxc 파서로 주석을 읽음. */
const JS = /\.(ts|tsx|mts|cts|js|jsx|mjs|cjs)$/;
/** Rust·Swift — `//`·블록 주석(중첩 가능). */
const RUST_LIKE = /\.(rs|swift)$/;
/** 검사 대상 설정 파일 — `#` 주석. */
const HASH = /\.(ya?ml|toml|sh)$/;
/** CHANGELOG `### 개발`은 이 버전부터 개조식(그 앞은 쓴 그대로 둔 기록). */
const CHANGELOG_SINCE = [0, 21, 0];

export const RULES = [
  {
    id: "끝맺음",
    why: '명사형(~함·~임·~됨·~음) 또는 명사로 끝냄 — "~한다·~했다·~이다·~합니다·~해요" 금지',
    // 조사 "~마다"·앞말에 붙은 비교 조사 "~보다"(10px보다), 명사 "필요·중요·주요·수요·강요·개요·소요"는 뺌.
    re: /(?<=[가-힣])(?:(?<!마)(?<!\S보)다|(?<![필중주수강개소])요|니다)(?=\s*(?:[.!?…,;:)\]—–-]|$))/u,
  },
  {
    id: "가운뎃점",
    why: "가운뎃점은 앞뒤를 붙임(그리드·마틴게일)",
    re: / · /,
  },
  {
    id: "조사",
    why: "조사는 앞말에 붙임 — 코드 표기·영문·숫자 뒤도 같음(`exit_id`를, README에, 18임)",
    re: /(?:`|□|[A-Za-z0-9_])\s+(?:을|를|이|가|은|는|의|로|으로|에|에서|와|과|이면|부터|까지|처럼|보다|으로는|에는|이나|이라|이고)(?=[\s.,)—:]|$)/,
  },
];

/** 따옴표·대괄호 안을 □ 하나로 바꾼 글자 — 옮겨 적은 화면 문구는 검사하지 않되 앞뒤 띄어쓰기는 봄. */
export function unquoted(text) {
  return text.replace(/"[^"]*"|“[^”]*”|‘[^’]*’|'[^'\n]*'|\[[^\]\n]*\]/g, "□");
}

/** 한 줄 글자에서 어긴 규칙들. 끝맺음·가운뎃점은 코드 표기 안도 지우고 봄(`x = 다` 같은 코드 조각). */
export function problems(text) {
  const plain = unquoted(text);
  if (!/[가-힣]/.test(plain)) return [];
  return RULES.filter((r) => r.re.test(r.id === "조사" ? plain : plain.replace(/`[^`]*`/g, "□"))).map((r) => r.id);
}

/** 글자 위치 → 줄 번호(1부터). */
function lineIndex(src) {
  const starts = [0];
  for (let i = 0; i < src.length; i++) if (src[i] === "\n") starts.push(i + 1);
  return (offset) => {
    let lo = 0;
    let hi = starts.length - 1;
    while (lo < hi) {
      const mid = (lo + hi + 1) >> 1;
      if (starts[mid] <= offset) lo = mid;
      else hi = mid - 1;
    }
    return lo + 1;
  };
}

/** 주석 본문들(`{at, body}` — body가 시작하는 글자 위치)을 줄 단위로 펼침. */
function toLines(src, bodies) {
  const lineOf = lineIndex(src);
  return bodies.flatMap(({ at, body }) => body.split("\n").map((text, k) => ({ line: lineOf(at) + k, text })));
}

/** JS·TS 주석 — oxc 파서가 정규식 리터럴·템플릿·JSX를 구분함. */
export function jsComments(file, src) {
  const { comments } = parseSync(file, src);
  return toLines(
    src,
    comments.map((c) => ({ at: c.start + 2, body: c.value })),
  );
}

/** CSS 주석 — 블록 주석만(CSS에는 `//` 주석이 없음, `url(https://…)`을 주석으로 읽지 않음). */
export function cssComments(src) {
  const bodies = [];
  let i = 0;
  while (i < src.length) {
    const c = src[i];
    if (c === '"' || c === "'") {
      let j = i + 1;
      while (j < src.length && src[j] !== c && src[j] !== "\n") j += src[j] === "\\" ? 2 : 1;
      i = j + 1;
    } else if (src.startsWith("/*", i)) {
      const end = src.indexOf("*/", i + 2);
      const stop = end < 0 ? src.length : end;
      bodies.push({ at: i + 2, body: src.slice(i + 2, stop) });
      i = stop + 2;
    } else {
      i++;
    }
  }
  return toLines(src, bodies);
}

/**
 * Rust·Swift 주석 — 문자열 종류를 알고 건너뜀.
 * - Rust: `"…"`(여러 줄 가능)·`r#"…"#`·`b"…"`·문자 리터럴(`'"'`, `'\''`, `'\u{1F600}'`)과 수명(`'a`)을 가름.
 * - Swift: `"""…"""` 여러 줄 문자열.
 * - 블록 주석은 안에 블록 주석을 품을 수 있음(중첩) — 두 언어 모두.
 */
export function rustLikeComments(src) {
  const bodies = [];
  const n = src.length;
  const ident = (ch) => ch !== undefined && /[A-Za-z0-9_]/.test(ch);
  let i = 0;
  while (i < n) {
    const c = src[i];
    if (src.startsWith('"""', i)) {
      const end = src.indexOf('"""', i + 3);
      i = end < 0 ? n : end + 3;
    } else if ((c === "r" || (c === "b" && src[i + 1] === "r")) && !ident(src[i - 1])) {
      const m = src.slice(i).match(/^b?r(#*)"/);
      if (m) {
        const close = `"${m[1]}`;
        const end = src.indexOf(close, i + m[0].length);
        i = end < 0 ? n : end + close.length;
      } else {
        i++;
      }
    } else if (c === '"') {
      let j = i + 1;
      while (j < n && src[j] !== '"') j += src[j] === "\\" ? 2 : 1;
      i = j + 1;
    } else if (c === "'") {
      // 문자 리터럴이면 통째로 건너뛰고, 아니면 수명(`'a`)이라 따옴표 하나만 넘김.
      const m = src.slice(i, i + 12).match(/^'(?:\\u\{[0-9A-Fa-f]+\}|\\.|[^\\'\n])'/);
      i += m ? m[0].length : 1;
    } else if (src.startsWith("//", i)) {
      const end = src.indexOf("\n", i);
      const stop = end < 0 ? n : end;
      bodies.push({ at: i + 2, body: src.slice(i + 2, stop) });
      i = stop;
    } else if (src.startsWith("/*", i)) {
      let depth = 1;
      let j = i + 2;
      while (j < n && depth > 0) {
        if (src.startsWith("/*", j)) [depth, j] = [depth + 1, j + 2];
        else if (src.startsWith("*/", j)) [depth, j] = [depth - 1, j + 2];
        else j++;
      }
      bodies.push({ at: i + 2, body: src.slice(i + 2, depth > 0 ? n : j - 2) });
      i = j;
    } else {
      i++;
    }
  }
  return toLines(src, bodies);
}

/** `#` 주석 줄들. 줄머리이거나 앞이 빈칸인 `#`만 주석으로 봄. */
export function hashComments(src) {
  return src.split("\n").flatMap((l, k) => {
    const m = l.match(/(?:^|\s)#(.*)$/);
    return m ? [{ line: k + 1, text: m[1] }] : [];
  });
}

const older = (v, since) => {
  for (let k = 0; k < 3; k++) if (v[k] !== since[k]) return v[k] < since[k];
  return false;
};

/**
 * 마크다운 본문 줄들(코드 블록 제외 — 목록 안에 들여쓴 것도).
 * CHANGELOG는 머리말과 `### 개발`(CHANGELOG_SINCE부터)만 — 사용자 분류는 해요체.
 * `## [Unreleased]`처럼 숫자가 아닌 칸은 가장 새 버전으로 봄.
 */
export function markdownLines(file, src) {
  const changelog = /(^|\/)CHANGELOG\.md$/.test(file);
  const out = [];
  let fence = null;
  let version = null;
  let section = null;
  src.split("\n").forEach((text, k) => {
    const mark = text.match(/^\s*(`{3,}|~{3,})/)?.[1];
    if (mark && (fence === null || mark[0] === fence[0])) {
      fence = fence === null ? mark : null;
      return;
    }
    if (fence !== null) return;
    const head = text.match(/^## \[([^\]]+)\]/);
    if (head) {
      const v = head[1].match(/^(\d+)\.(\d+)\.(\d+)$/);
      [version, section] = [v ? v.slice(1).map(Number) : [Infinity, 0, 0], null];
    } else if (/^### /.test(text)) section = text.slice(4).trim();
    if (changelog && version && (head || section !== "개발" || /^### /.test(text) || older(version, CHANGELOG_SINCE)))
      return;
    out.push({ line: k + 1, text });
  });
  return out;
}

/** 검사 대상 파일인지. */
export const isTarget = (file) =>
  JS.test(file) || RUST_LIKE.test(file) || file.endsWith(".css") || HASH.test(file) || file.endsWith(".md");

/** 파일 하나의 검사할 줄들. 대상이 아니면 빈 배열. */
export function linesOf(file, src) {
  if (JS.test(file)) return jsComments(file, src);
  if (RUST_LIKE.test(file)) return rustLikeComments(src);
  if (file.endsWith(".css")) return cssComments(src);
  if (HASH.test(file)) return hashComments(src);
  if (file.endsWith(".md")) return markdownLines(file, src);
  return [];
}

/** 파일들을 검사해 어긴 곳 목록 — `{file, line, rule, text}`. */
export function scan(files, read) {
  return files.flatMap((file) =>
    linesOf(file, read(file)).flatMap(({ line, text }) =>
      problems(text).map((rule) => ({ file, line, rule, text: text.trim() })),
    ),
  );
}
