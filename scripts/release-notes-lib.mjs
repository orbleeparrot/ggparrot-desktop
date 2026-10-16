// 릴리스 본문 만들기 — CHANGELOG.md의 버전 칸에서 사용자용 내용만 뽑음.
// - 배포 여부는 CHANGELOG에 적지 않음 — 공개한 릴리스의 태그(v<버전>)로 앎. 태그는 [Publish release] 때만 생김.
// - 배포할 버전 칸과, 그 아래로 지난 배포(그보다 낮은 태그 가운데 가장 높은 버전) 뒤의 칸을 모두 합침.
// - 분류(### 추가 등)별로 모으고, `### 개발`은 뺌(개발자용이라 사용자에게 보이지 않게).
// - 분류 앞 소개 글(예: '첫 배포.')도 합친 칸 모두에서 모음.
// - 끝에 WEB_ONLY_MARKER 아래로 설치 파일 안내를 붙임. 앱의 '바뀐 점'은 이 표시 앞까지만 보임
//   (src/state/releaseNotes.ts의 WEB_ONLY_MARKER와 같아야 함 — 테스트가 확인).
import { INSTALLERS, assetName } from "./bundle-names.mjs";

export const ORDER = ["추가", "변경", "수정", "삭제", "보안"];
export const HIDDEN = new Set(["개발"]);
/** 웹 릴리스 페이지에만 보일 부분의 시작 표시 — 마크다운에서는 보이지 않는 HTML 주석. */
export const WEB_ONLY_MARKER = "<!-- web-only -->";

/** "0.20.1" → [0, 20, 1]. 모양이 다르면 null. */
function parseVersion(v) {
  const m = /^(\d+)\.(\d+)\.(\d+)$/.exec(String(v).replace(/^v/, ""));
  return m ? m.slice(1).map(Number) : null;
}

/** 두 버전 비교 — a가 낮으면 음수, 같으면 0, 높으면 양수. */
export function compareVersions(a, b) {
  const x = parseVersion(a);
  const y = parseVersion(b);
  if (!x || !y) throw new Error(`버전 모양이 아니에요: ${!x ? a : b}`);
  for (let i = 0; i < 3; i += 1) if (x[i] !== y[i]) return x[i] - y[i];
  return 0;
}

/** 태그 목록에서 공개한 버전들 — v<주>.<부>.<수>만(그 밖의 태그는 무시). */
export function releasedVersions(tags) {
  return tags
    .map((t) => t.trim())
    .filter((t) => /^v\d+\.\d+\.\d+$/.test(t))
    .map((t) => t.slice(1));
}

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

/** 배포할 버전 칸과, 지난 배포 뒤의 칸들(버전이 높은 순). released: 공개한 버전 목록. 문제가 있으면 Error. */
function pickSections(text, version, released) {
  const sections = parseChangelog(text);
  if (!sections.some((s) => s.version === version)) {
    throw new Error(`CHANGELOG.md에 '## [${version}]' 칸이 없어요. 이 버전의 변경을 적은 칸을 먼저 만드세요.`);
  }
  // 지난 배포 — 이 버전보다 낮은 공개 버전 가운데 가장 높은 것. 없으면 첫 배포라 처음부터 다 합침.
  const last = released
    .filter((r) => compareVersions(r, version) < 0)
    .sort(compareVersions)
    .at(-1);
  return sections
    .filter((s) => parseVersion(s.version))
    .filter((s) => compareVersions(s.version, version) <= 0 && (!last || compareVersions(s.version, last) > 0))
    .sort((a, b) => compareVersions(b.version, a.version));
}

/** 배포할 버전의 릴리스 본문. released: 공개한 버전 목록(태그에서, releasedVersions). 문제가 있으면 Error. */
export function buildNotes(text, version, released = []) {
  const picked = pickSections(text, version, released);
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
