// 릴리스 본문 만들기 — CHANGELOG.md의 버전 칸에서 사용자용 내용만 뽑음.
// - 배포할 버전 칸부터 아래로, 배포하지 않은 칸('미배포'·'배포하지 않음')을 다음 배포된 칸 전까지 모두 합침.
// - 분류(### 추가 등)별로 모으고, `### 개발`은 뺌(개발자용이라 사용자에게 보이지 않게).
// - 분류 앞 소개 글(예: '첫 배포.')도 합친 칸 모두에서 모음.
// - 끝에 WEB_ONLY_MARKER 아래로 설치 파일 안내를 붙임. 앱의 '바뀐 점'은 이 표시 앞까지만 보임
//   (src/state/releaseNotes.ts의 WEB_ONLY_MARKER와 같아야 함 — 테스트가 확인).
import { INSTALLERS, assetName } from "./bundle-names.mjs";

export const ORDER = ["추가", "변경", "수정", "삭제", "보안"];
export const HIDDEN = new Set(["개발"]);
/** 웹 릴리스 페이지에만 보일 부분의 시작 표시 — 마크다운에서는 보이지 않는 HTML 주석. */
export const WEB_ONLY_MARKER = "<!-- web-only -->";

const unreleased = (header) => header.includes("미배포") || header.includes("배포하지 않음");

/** CHANGELOG 글을 버전 칸 목록으로 — [{ version, header, line, intro: string[], groups: Map<분류, string[]> }] */
export function parseChangelog(text) {
  const sections = [];
  let cur = null;
  let group = null;
  text.split("\n").forEach((line, i) => {
    const v = line.match(/^## \[([^\]]+)\](.*)$/);
    if (v) {
      cur = { version: v[1], header: line, line: i, intro: [], groups: new Map() };
      sections.push(cur);
      group = null;
      return;
    }
    if (!cur) return;
    const g = line.match(/^### (.+?)\s*$/);
    if (g) {
      group = g[1];
      if (!cur.groups.has(group)) cur.groups.set(group, []);
      return;
    }
    if (group) cur.groups.get(group).push(line);
    else cur.intro.push(line);
  });
  return sections;
}

function trimLines(lines) {
  const out = [...lines];
  while (out.length && !out[0].trim()) out.shift();
  while (out.length && !out[out.length - 1].trim()) out.pop();
  return out;
}

/** 배포할 버전 칸과, 그 아래로 이어지는 배포하지 않은 칸들. 문제가 있으면 Error. */
function pickSections(text, version, { preview = false } = {}) {
  const sections = parseChangelog(text);
  const start = sections.findIndex((s) => s.version === version);
  if (start < 0) {
    throw new Error(
      `CHANGELOG.md에 '## [${version}]' 칸이 없어요. '미배포' 칸을 이 버전으로 바꾼 뒤 태그를 다시 올리세요.`,
    );
  }
  if (!preview && unreleased(sections[start].header)) {
    throw new Error(
      `CHANGELOG.md의 '${sections[start].header}'에 배포 날짜를 적으세요(npm run release:mark -- v${version}).`,
    );
  }
  const picked = [sections[start]];
  for (const s of sections.slice(start + 1)) {
    if (!unreleased(s.header)) break;
    picked.push(s);
  }
  return picked;
}

/** 배포할 버전의 릴리스 본문. preview면 '미배포' 칸도 받음(태그 전에 미리 보기). 문제가 있으면 Error. */
export function buildNotes(text, version, options = {}) {
  const picked = pickSections(text, version, options);
  const intros = [];
  const groups = new Map();
  for (const s of picked) {
    const intro = trimLines(s.intro).join("\n");
    if (intro && !intros.includes(intro)) intros.push(intro);
    for (const [name, lines] of s.groups) {
      if (HIDDEN.has(name)) continue;
      const body = trimLines(lines);
      if (!body.length) continue;
      if (!groups.has(name)) groups.set(name, []);
      groups.get(name).push(...body);
    }
  }
  const names = [...ORDER.filter((n) => groups.has(n)), ...[...groups.keys()].filter((n) => !ORDER.includes(n))];
  const parts = [...intros];
  for (const n of names) parts.push(`### ${n}\n\n${groups.get(n).join("\n")}`);
  return parts.join("\n\n");
}

/** 웹 릴리스 페이지에만 보일 설치 안내 — WEB_ONLY_MARKER 아래(앱의 '바뀐 점'에는 보이지 않음). */
export function installFooter(binary, version) {
  const mac = assetName(binary, version, INSTALLERS.mac);
  const win = assetName(binary, version, INSTALLERS.windows);
  return (
    `\n\n${WEB_ONLY_MARKER}\n\n---\n\n설치: macOS \`${mac}\` · Windows \`${win}\`. ` +
    "이미 설치했다면 앱의 설정 › 정보 › 업데이트에서 바로 받을 수 있어요."
  );
}

/** 배포 표시 — 배포할 칸에 날짜를 달고, 함께 나가는 배포하지 않은 칸은 '배포하지 않음(<버전>에 포함)'으로 바꿈. */
export function markReleased(text, version, date) {
  if (!/^\d{4}-\d{2}-\d{2}$/.test(date)) throw new Error(`날짜는 YYYY-MM-DD로 적으세요: ${date}`);
  const picked = pickSections(text, version, { preview: true });
  if (!unreleased(picked[0].header)) throw new Error(`'${picked[0].header}'은 이미 배포 표시가 되어 있어요.`);
  const lines = text.split("\n");
  lines[picked[0].line] = `## [${version}] - ${date}`;
  for (const s of picked.slice(1)) lines[s.line] = `## [${s.version}] - 배포하지 않음(${version}에 포함)`;
  return lines.join("\n");
}
