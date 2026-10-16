// 로컬 빌드의 설치 파일 이름을 배포(CI)와 같게 바꿈.
//   Tauri는 화면 이름(productName)으로 파일을 만듦 → macOS `GGParrot_0.19.0_universal.dmg`, Windows `껄무새_0.19.0_x64-setup.exe`
//   배포 이름(bundle-names.mjs의 패턴)은 기술 이름 → `ggparrot-desktop_0.18.22_darwin_universal.dmg`
// 사용: npm run build:mac / npm run build:win 끝에 자동으로 돎. 따로: node scripts/name-bundles.mjs
import { existsSync, readdirSync, renameSync } from "node:fs";
import { join } from "node:path";
import { assetName, readAppInfo } from "./bundle-names.mjs";

const root = new URL("..", import.meta.url).pathname;
let app;
try {
  app = readAppInfo();
} catch (e) {
  console.error(e.message);
  process.exit(1);
}
const { product, productWindows, binary, version } = app;

// 번들 폴더 — 대상(target)별·형식별. 없는 폴더는 건너뜀.
const targets = [
  [
    "src-tauri/target",
    ["release", "universal-apple-darwin/release", "aarch64-apple-darwin/release", "x86_64-apple-darwin/release"],
  ],
  ["src-tauri/target-win", ["x86_64-pc-windows-gnu/release", "x86_64-pc-windows-msvc/release"]],
];
const kinds = { dmg: "darwin", nsis: "windows" };

let renamed = 0;
for (const [base, dirs] of targets) {
  for (const dir of dirs) {
    for (const [kind, platform] of Object.entries(kinds)) {
      const folder = join(root, base, dir, "bundle", kind);
      if (!existsSync(folder)) continue;
      for (const name of readdirSync(folder)) {
        // `GGParrot_<버전>_<아키텍처>.dmg`, `껄무새_<버전>_<아키텍처>-setup.exe`(Windows 화면 이름)
        const prefix = `${kind === "nsis" ? productWindows : product}_${version}_`;
        if (!name.startsWith(prefix)) continue;
        const m = name.slice(prefix.length).match(/^(.+?)(-setup)?(\.[A-Za-z0-9]+)$/);
        if (!m) continue;
        const to = assetName(binary, version, { platform, arch: m[1], setup: m[2] ?? "", ext: m[3] });
        renameSync(join(folder, name), join(folder, to));
        console.log(`${join(base, dir, "bundle", kind)}/${to}`);
        renamed += 1;
      }
    }
  }
}
if (renamed === 0) console.log(`바꿀 파일이 없어요(${product}·${productWindows}_${version}_* 를 찾지 못함).`);
