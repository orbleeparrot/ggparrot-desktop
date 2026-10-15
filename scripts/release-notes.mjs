// CHANGELOG.md에서 릴리스 본문을 만듦 — 배포 워크플로가 GitHub 릴리스와 앱 업데이트의 '바뀐 점'에 씀.
//   node scripts/release-notes.mjs v0.18.22                  → 배포할 본문(그 버전 칸에 날짜가 있어야 함)
//   node scripts/release-notes.mjs v0.18.22 --preview        → 태그 전에 미리 보기('미배포'도 받음)
//   node scripts/release-notes.mjs v0.18.22 --mark [날짜]    → CHANGELOG에 배포 표시(날짜 기본값은 오늘)
//   npm run release:notes -- v0.18.22 --preview · npm run release:mark -- v0.18.22
// 규칙은 scripts/release-notes-lib.mjs.
import { readFileSync, writeFileSync } from "node:fs";
import { readAppInfo } from "./bundle-names.mjs";
import { buildNotes, installFooter, markReleased } from "./release-notes-lib.mjs";

// 순서와 상관없이 읽음 — npm run release:mark -- v0.18.22 처럼 플래그가 먼저 와도 됨.
const args = process.argv.slice(2);
const isDate = (a) => /^\d{4}-\d{2}-\d{2}$/.test(a);
const flag = args.find((a) => a.startsWith("--"));
const arg = args.find(isDate);
const tag = args.find((a) => !a.startsWith("--") && !isDate(a));
if (!tag) {
  console.error("사용법: node scripts/release-notes.mjs v<버전> [--preview | --mark [YYYY-MM-DD]]");
  process.exit(2);
}
const version = tag.replace(/^v/, "");
const path = new URL("../CHANGELOG.md", import.meta.url);
const text = readFileSync(path, "utf8");

function today() {
  const d = new Date();
  const pad = (n) => String(n).padStart(2, "0");
  return `${d.getFullYear()}-${pad(d.getMonth() + 1)}-${pad(d.getDate())}`;
}

try {
  if (flag === "--mark") {
    writeFileSync(path, markReleased(text, version, arg ?? today()));
    console.log(`CHANGELOG.md에 v${version} 배포 표시를 했어요. 확인한 뒤 커밋하세요.`);
  } else {
    const body = buildNotes(text, version, { preview: flag === "--preview" });
    process.stdout.write(body + installFooter(readAppInfo().binary, version) + "\n");
  }
} catch (e) {
  console.error(e.message);
  process.exit(1);
}
