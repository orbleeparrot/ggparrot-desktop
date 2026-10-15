import { describe, expect, it } from "vitest";
import { makeRun } from "../test/fixtures";
import { alertOf, labelOf, serverCell } from "./runStatus";

const ok = { text: "실행 중", tone: "ok" as const };

describe("실행 상태 문구", () => {
  it("라벨은 끝남 · 종료 중 · 시작 중을 앞세우고, 실행 중이면 목록 카드와 같은 상태 문구", () => {
    expect(labelOf(makeRun({ finished: { status: "error", note: "x", at: 1 } }), "finished", ok)).toBe(
      "오류로 멈춤 · x",
    );
    expect(labelOf(makeRun(), "stopping", ok)).toBe("종료 처리 중");
    expect(labelOf(makeRun(), "running", { text: "실행 중 · 서버 재시도 중", tone: "warn" })).toBe(
      "실행 중 · 서버 재시도 중",
    );
  });

  it("서버 칸은 거절과 끊김을 나누고, 1분이 넘은 끊김만 시간을 붙인다", () => {
    const now = 1_000_000_000;
    expect(serverCell(makeRun({ updatedAt: 0 }), now).text).toBe("확인 중");
    expect(serverCell(makeRun({ authFailed: true, online: false }), now).text).toBe("회원 키 거절됨");
    expect(serverCell(makeRun({ rejected: 422, online: false }), now).text).toBe("서버가 거절함(HTTP 422)");
    expect(serverCell(makeRun({ online: false, offlineSince: now - 30_000 }), now).text).toBe("다시 연결하는 중");
    expect(serverCell(makeRun({ online: false, offlineSince: now - 180_000 }), now).text).toBe(
      "다시 연결하는 중 · 3분째",
    );
  });

  it("경고 띠는 모드에 맞는 확인 장소를 말한다", () => {
    const stale = makeRun({ inPosition: true, priceStaleSince: 1 });
    expect(alertOf(stale, "live")).toContain("거래소에서 포지션을 확인하세요");
    expect(alertOf(stale, "testnet")).toContain("바이낸스 테스트넷");
    expect(alertOf(stale, "mock")).toContain("실제 계정에는 영향이 없어요");
    expect(alertOf(makeRun({ priceStaleSince: 1 }), "live")).toBeNull();
  });
});
