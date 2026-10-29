import { readFileSync } from "node:fs";
import { describe, expect, it } from "vitest";

// Rust가 등록한 명령과 화면의 명령 표(src/ipc.ts CommandMap)가 같은지 확인.
// - 테스트는 jsdom에서 돌아 import.meta.url이 file: 주소가 아님 — 저장소 루트(npm test 위치) 기준으로 읽음.
// - 앱 tsconfig에는 Node 타입이 없어 scripts/에 둠.

/** 이름 순 비교 — sort()의 기본 비교(UTF-16 코드 단위)에 기대지 않게. */
const byName = (a, b) => a.localeCompare(b);

/** lib.rs의 `generate_handler![ commands::x, … ]`에 적힌 명령 이름들. */
function rustCommands(src) {
  const body = src.match(/generate_handler!\s*\[([\s\S]*?)\]/)?.[1];
  if (!body) throw new Error("src-tauri/src/lib.rs에서 generate_handler![…]를 찾지 못함");
  return [...body.replace(/\/\/[^\n]*/g, "").matchAll(/commands::(\w+)/g)].map((m) => m[1]);
}

/** ipc.ts `interface CommandMap { … }`의 키들(주석 제외). */
function tsCommands(src) {
  const body = src.match(/export interface CommandMap\s*\{([\s\S]*?)\n\}/)?.[1];
  if (!body) throw new Error("src/ipc.ts에서 CommandMap을 찾지 못함");
  const code = body.replace(/\/\*[\s\S]*?\*\//g, "").replace(/\/\/[^\n]*/g, "");
  return [...code.matchAll(/^\s*(\w+)\s*:/gm)].map((m) => m[1]);
}

describe("명령 목록", () => {
  const rust = rustCommands(readFileSync("src-tauri/src/lib.rs", "utf8"));
  const ts = tsCommands(readFileSync("src/ipc.ts", "utf8"));

  it("두 목록을 읽어 옴", () => {
    expect(rust.length).toBeGreaterThan(10);
    expect(ts.length).toBeGreaterThan(10);
  });

  it("중복 없음", () => {
    expect(new Set(rust).size).toBe(rust.length);
    expect(new Set(ts).size).toBe(ts.length);
  });

  it("Rust generate_handler!와 ipc.ts CommandMap의 이름이 같음", () => {
    expect([...ts].sort(byName)).toEqual([...rust].sort(byName));
  });
});
