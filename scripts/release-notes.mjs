// CHANGELOG.md에서 릴리스 본문을 만듦 — 배포 워크플로가 GitHub 릴리스와 앱 업데이트의 '바뀐 점'에 씀.
//   node scripts/release-notes.mjs v0.20.1   (npm run release:notes -- v0.20.1)
// - 지난 배포는 git 태그(v<버전>)로 앎 — 로컬에서 미리 볼 때는 먼저 `git fetch --tags`.
// 규칙은 scripts/release-notes-lib.mjs.
import { execFileSync } from "node:child_process";
import { readFileSync } from "node:fs";
import { readAppInfo } from "./bundle-names.mjs";
import { buildNotes, installFooter, releasedVersions } from "./release-notes-lib.mjs";

const tag = process.argv.slice(2).find((a) => !a.startsWith("--"));
if (!tag) {
  console.error("사용법: node scripts/release-notes.mjs v<버전>");
  process.exit(2);
}
const version = tag.replace(/^v/, "");
const text = readFileSync(new URL("../CHANGELOG.md", import.meta.url), "utf8");

try {
  const tags = execFileSync("git", ["tag", "--list", "v*"], { encoding: "utf8" }).split("\n");
  const body = buildNotes(text, version, releasedVersions(tags));
  process.stdout.write(body + installFooter(readAppInfo().binary, version) + "\n");
} catch (e) {
  console.error(e.message);
  process.exit(1);
}
