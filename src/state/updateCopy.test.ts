import { describe, expect, it } from "vitest";
import { makeState } from "../test/fixtures";
import type { UpdateState } from "../types";
import { updateCopy } from "./updateCopy";

const u = (over: Partial<UpdateState>): UpdateState => ({ ...makeState().update, ...over });

describe("업데이트 문구", () => {
  it("확인 전·확인 중은 덧붙임이 없음", () => {
    expect(updateCopy(u({ status: "idle" }), 0, false)).toEqual({
      headline: { icon: null, text: "아직 업데이트를 확인하지 않았어요" },
      detail: "",
    });
    expect(updateCopy(u({ status: "checking" }), 0, false)).toEqual({
      headline: { icon: null, text: "새 버전을 확인하는 중…" },
      detail: "",
    });
  });

  it("최신이면 마지막 확인 시각을 덧붙이고, 막 업데이트했으면 그 전 버전을 알림", () => {
    const latest = updateCopy(u({ status: "none", updatedFrom: "", checkedAt: 0 }), 0, false);
    expect(latest.headline).toEqual({ icon: "latest", text: "최신 버전이에요" });
    expect(latest.detail).toBe("아직 확인하지 않았어요");
    const updated = updateCopy(u({ status: "none", updatedFrom: "0.19.0", checkedAt: Date.now() }), 0, false);
    expect(updated.headline).toEqual({ icon: "updated", text: "v0.19.0에서 업데이트했어요" });
    expect(updated.detail).toMatch(/^마지막 확인 /);
  });

  it("새 버전·내려받는 중은 매크로가 계속 돈다고 알림", () => {
    const available = updateCopy(u({ status: "available", version: "0.19.3" }), 2, false);
    expect(available.headline).toEqual({ icon: null, text: "새 버전 v0.19.3이 나왔어요" });
    expect(available.detail).toBe("내려받는 동안에도 매크로는 계속 돌아요.");
    const downloading = updateCopy(u({ status: "downloading", version: "0.19.2" }), 2, false);
    expect(downloading.headline.text).toBe("v0.19.2 내려받는 중");
    expect(downloading.detail).toBe("다 받으면 서명을 확인해요. 매크로는 계속 돌아요.");
  });

  it("설치 준비 — 실행 중인 매크로가 있으면 막힌 이유, 없으면 OS별 안내", () => {
    const ready = u({ status: "ready", version: "0.19.2" });
    expect(updateCopy(ready, 0, false).headline.text).toBe("v0.19.2 설치 준비가 끝났어요");
    expect(updateCopy(ready, 2, false).detail).toBe(
      "실행 중인 매크로 2개를 모두 종료하면 설치할 수 있어요. 받은 파일은 그대로 있어요.",
    );
    expect(updateCopy(ready, 0, true).detail).toBe("다시 시작하면 설치 창이 잠깐 뜬 뒤 새 버전으로 켜져요.");
    expect(updateCopy(ready, 0, false).detail).toBe("다시 시작하면 새 버전으로 바뀌어요.");
  });

  it("설치 중은 도는 아이콘과 OS별 안내", () => {
    const installing = u({ status: "installing", version: "0.19.2" });
    expect(updateCopy(installing, 0, false).headline).toEqual({ icon: "installing", text: "v0.19.2 설치하는 중" });
    expect(updateCopy(installing, 0, true).detail).toBe("설치 창이 뜨고 끝나면 앱이 다시 켜져요.");
    expect(updateCopy(installing, 0, false).detail).toBe("곧 앱이 다시 켜져요.");
  });

  it("오류·지원 안 함", () => {
    const error = updateCopy(u({ status: "error", checkedAt: 0 }), 0, false);
    expect(error.headline).toEqual({ icon: "error", text: "업데이트 중 문제가 생겼어요" });
    expect(error.detail).toBe("아직 확인하지 않았어요");
    expect(updateCopy(u({ status: "unsupported" }), 0, false)).toEqual({
      headline: { icon: null, text: "이 빌드는 자동 업데이트를 쓰지 않아요" },
      detail: "개발용 빌드예요.",
    });
  });
});
