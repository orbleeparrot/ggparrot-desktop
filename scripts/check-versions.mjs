// 앱 버전은 src-tauri/Cargo.toml의 [workspace.package] 한 곳에만 적음.
// 다른 곳에 버전이 다시 생기지 않았는지 보고, 태그를 넘기면(v0.18.3) 그것과도 비교함:
//   node scripts/check-versions.mjs v0.18.3
import { readFileSync } from "node:fs";

const read = (p) => readFileSync(new URL(`../${p}`, import.meta.url), "utf8");
const problems = [];

const workspace = read("src-tauri/Cargo.toml").match(/^\[workspace\.package\][^[]*?^version\s*=\s*"([^"]+)"/ms)?.[1];
if (!workspace) problems.push("src-tauri/Cargo.toml 의 [workspace.package] 에 version 이 없어요.");

for (const crate of ["src-tauri/Cargo.toml", "src-tauri/crates/runner-core/Cargo.toml"]) {
  const pkg = read(crate).match(/^\[package\][^[]*/ms)?.[0] ?? "";
  if (!/^version\.workspace\s*=\s*true/m.test(pkg))
    problems.push(`${crate}: [package] 에 version.workspace = true 를 쓰세요.`);
}
// 여기에 version이 있으면 Tauri가 Cargo 대신 그 값을 씀(업데이터 비교·설치 파일 이름).
if ("version" in JSON.parse(read("src-tauri/tauri.conf.json")))
  problems.push("src-tauri/tauri.conf.json: version 을 지우세요. Cargo 버전을 따라가요.");
if ("version" in JSON.parse(read("package.json")))
  problems.push("package.json: version 을 지우세요(배포하지 않는 패키지).");

const tag = process.argv[2];
if (tag && workspace && tag.replace(/^v/, "") !== workspace)
  problems.push(`앱 버전(${workspace})과 태그(${tag})가 서로 달라요.`);

if (problems.length) {
  for (const p of problems) console.error(p);
  process.exit(1);
}
console.log(`앱 버전 ${workspace}${tag ? ` · 태그(${tag})와 같아요` : ""}.`);
