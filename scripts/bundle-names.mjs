// 설치 파일 이름 규칙 한 곳 — 배포(release.yml의 releaseAssetNamePattern)·로컬 빌드(name-bundles.mjs)·
// 릴리스 본문의 설치 안내(release-notes-lib.mjs)가 함께 씀. release.yml의 패턴과 같은지는 테스트가 확인함.
import { readFileSync } from "node:fs";

/** tauri-action의 releaseAssetNamePattern과 같은 글자. */
export const ASSET_PATTERN = "[mainBinaryName]_[version]_[platform]_[arch][setup][ext]";

/** 배포하는 설치 파일 — macOS는 Apple Silicon·Intel 공용 디스크 이미지, Windows는 NSIS 설치 프로그램. */
export const INSTALLERS = {
  mac: { platform: "darwin", arch: "universal", setup: "", ext: ".dmg" },
  windows: { platform: "windows", arch: "x64", setup: "-setup", ext: ".exe" },
};

/** 패턴대로 만든 설치 파일 이름 — `ggparrot-desktop_0.18.21_darwin_universal.dmg`. */
export function assetName(binary, version, { platform, arch, setup, ext }) {
  return ASSET_PATTERN.replace("[mainBinaryName]", binary)
    .replace("[version]", version)
    .replace("[platform]", platform)
    .replace("[arch]", arch)
    .replace("[setup]", setup)
    .replace("[ext]", ext);
}

/** 저장소의 앱 정보 — 화면 이름(productName, Windows는 tauri.windows.conf.json 값)·기술 이름(mainBinaryName)·버전. */
export function readAppInfo(root = new URL("..", import.meta.url)) {
  const conf = JSON.parse(readFileSync(new URL("src-tauri/tauri.conf.json", root), "utf8"));
  const win = JSON.parse(readFileSync(new URL("src-tauri/tauri.windows.conf.json", root), "utf8"));
  const cargo = readFileSync(new URL("src-tauri/Cargo.toml", root), "utf8");
  const version = cargo.match(/^\[workspace\.package\][^[]*?^version\s*=\s*"([^"]+)"/ms)?.[1];
  if (!version) throw new Error("src-tauri/Cargo.toml에서 [workspace.package] version을 찾지 못했어요.");
  return {
    product: conf.productName,
    productWindows: win.productName ?? conf.productName,
    binary: conf.mainBinaryName,
    version,
  };
}
