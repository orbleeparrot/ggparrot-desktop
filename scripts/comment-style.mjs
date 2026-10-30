// 개조식 검사(docs/conventions.md 1.1) — 코드 주석·문서의 끝맺음·가운뎃점·조사 띄어쓰기.
// - 사람이든 AI든 같은 기준을 지키게 `npm test`(scripts/comment-style.test.mjs)에서 돌림.
// - 따옴표("…" '…' “…” ‘…’)·대괄호([…])·코드 표기(`…`) 안은 보지 않음 — 화면 문구(해요체)나 단추 이름을 옮겨 적는 자리.

/** 검사 대상 코드 파일 — `//`·블록 주석. */
const SLASH = /\.(rs|ts|tsx|mjs|js|swift|css)$/;
/** 검사 대상 설정 파일 — `#` 주석. */
const HASH = /\.(ya?ml|toml|sh)$/;
/** CHANGELOG `### 개발`은 이 버전부터 개조식(그 앞은 쓴 그대로 둔 기록). */
const CHANGELOG_SINCE = [0, 21, 0];

export const RULES = [
  {
    id: "끝맺음",
    why: '명사형(~함·~임·~됨·~음) 또는 명사로 끝냄 — "~한다·~했다·~이다·~합니다·~해요" 금지',
    // "~마다"는 조사, "필요·중요·주요·수요·강요·개요·소요"는 명사라 뺌.
    re: /(?<=[가-힣])(?:(?<!마)다|(?<![필중주수강개소])요|니다)(?=\s*(?:[.!?…,;:)\]—–-]|$))/u,
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

/** 한 줄 글자에서 어긴 규칙들. 끝맺음은 코드 표기 안도 지우고 봄(`x = 다` 같은 코드 조각). */
export function problems(text) {
  const plain = unquoted(text);
  if (!/[가-힣]/.test(plain)) return [];
  return RULES.filter((r) => r.re.test(r.id === "조사" ? plain : plain.replace(/`[^`]*`/g, "□"))).map((r) => r.id);
}

/** `//`·블록 주석 줄들 — 글자 리터럴 안은 건너뜀. Rust는 작은따옴표가 수명(`'a`)이라 큰따옴표만 글자로 봄. */
export function slashComments(src, quotes) {
  const out = [];
  let i = 0;
  let line = 1;
  const n = src.length;
  while (i < n) {
    const c = src[i];
    if (c === "\n") {
      line++;
      i++;
    } else if (quotes === '"' && src.startsWith("'\"'", i)) {
      i += 3; // Rust 문자 리터럴 '"'
    } else if (quotes.includes(c)) {
      let j = i + 1;
      while (j < n && src[j] !== c) {
        if (src[j] === "\\") j++;
        else if (src[j] === "\n") {
          if (c !== "`" && c !== '"') break;
          line++;
        }
        j++;
      }
      i = j + 1;
    } else if (src.startsWith("//", i)) {
      const end = src.indexOf("\n", i) < 0 ? n : src.indexOf("\n", i);
      out.push({ line, text: src.slice(i + 2, end) });
      i = end;
    } else if (src.startsWith("/*", i)) {
      const end = src.indexOf("*/", i + 2) < 0 ? n : src.indexOf("*/", i + 2);
      src
        .slice(i + 2, end)
        .split("\n")
        .forEach((text, k) => out.push({ line: line + k, text }));
      line += src.slice(i, end).split("\n").length - 1;
      i = end + 2;
    } else {
      i++;
    }
  }
  return out;
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

/** 마크다운 본문 줄들(코드 블록 제외). CHANGELOG는 머리말과 `### 개발`(CHANGELOG_SINCE부터)만 — 사용자 분류는 해요체. */
export function markdownLines(file, src) {
  const changelog = /(^|\/)CHANGELOG\.md$/.test(file);
  const out = [];
  let fence = false;
  let version = null;
  let section = null;
  src.split("\n").forEach((text, k) => {
    if (/^```/.test(text)) fence = !fence;
    if (fence || /^```/.test(text)) return;
    const v = text.match(/^## \[(\d+)\.(\d+)\.(\d+)\]/);
    if (v) [version, section] = [v.slice(1).map(Number), null];
    else if (/^### /.test(text)) section = text.slice(4).trim();
    if (changelog && version && (v || section !== "개발" || /^### /.test(text) || older(version, CHANGELOG_SINCE)))
      return;
    out.push({ line: k + 1, text });
  });
  return out;
}

/** 검사 대상 파일인지. */
export const isTarget = (file) => SLASH.test(file) || HASH.test(file) || file.endsWith(".md");

/** 파일 하나의 검사할 줄들. 대상이 아니면 빈 배열. */
export function linesOf(file, src) {
  if (SLASH.test(file)) return slashComments(src, file.endsWith(".rs") ? '"' : file.endsWith(".css") ? "\"'" : "\"'`");
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
