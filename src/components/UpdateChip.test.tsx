import { describe, expect, it } from "vitest";
import { makeState } from "../test/fixtures";
import type { UpdateState } from "../types";
import { formatBytes } from "../state/format";
import { progressPct, progressText } from "../state/update";
import { updateChipLabel } from "./UpdateChip";

const u = (over: Partial<UpdateState>): UpdateState => ({ ...makeState().update, ...over });

describe("업데이트 알림", () => {
  it("버전 옆 칩 글은 상태마다 다르고, 할 일이 없으면 칩이 없다", () => {
    expect(updateChipLabel(u({ status: "none" }))).toBeNull();
    expect(updateChipLabel(u({ status: "available", version: "0.18.3" }))).toBe("v0.18.3 새 버전");
    expect(updateChipLabel(u({ status: "downloading", progress: 0.426 }))).toBe("내려받는 중 43%");
    expect(updateChipLabel(u({ status: "downloading", progress: -1 }))).toBe("내려받는 중");
    expect(updateChipLabel(u({ status: "ready" }))).toBe("설치 준비됨");
  });

  it("진행 글은 % 와 받은 크기 / 전체 크기", () => {
    const mb = 1024 * 1024;
    expect(progressText(u({ progress: 0.5, downloaded: 7 * mb, total: 14 * mb }))).toBe("50% · 7.0 MB / 14.0 MB");
    expect(progressText(u({ progress: -1, downloaded: 3 * mb, total: 0 }))).toBe("3.0 MB 받음");
    expect(progressText(u({ progress: -1, downloaded: 0, total: 0 }))).toBe("연결하는 중…");
  });

  it("진행률은 0~100 정수, 크기를 모르면(-1) 없음", () => {
    expect(progressPct(u({ progress: 0.426 }))).toBe(43);
    expect(progressPct(u({ progress: 0 }))).toBe(0);
    expect(progressPct(u({ progress: -1 }))).toBeNull();
  });

  it("크기는 1 MB부터 MB, 그 아래는 KB", () => {
    expect(formatBytes(14 * 1024 * 1024)).toBe("14.0 MB");
    expect(formatBytes(512 * 1024)).toBe("512 KB");
    expect(formatBytes(0)).toBe("0 KB");
  });
});
