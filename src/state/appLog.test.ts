import { describe, expect, it } from "vitest";
import type { AppLogEntry } from "../types";
import { countAppLog, MAX_APP_LOG, mergeAppLog, visibleAppLog } from "./appLog";
import { settingsElementId } from "./runs";

const e = (id: number, level: AppLogEntry["level"] = "info"): AppLogEntry => ({
  id,
  at: 1_760_000_000_000,
  level,
  source: "app",
  title: `줄 ${id}`,
  detail: "",
});

describe("앱 기록", () => {
  it("get_app_log와 이벤트가 같은 줄을 주면 id로 한 번만 — 바뀐 게 없으면 같은 배열", () => {
    const first = mergeAppLog([], [e(1), e(2)]);
    const next = mergeAppLog(first, [e(2), e(3)]);
    expect(next.map((x) => x.id)).toEqual([1, 2, 3]);
    expect(mergeAppLog(next, [e(3)])).toBe(next);
  });

  it("최근 500줄만 남긴다", () => {
    const many = Array.from({ length: MAX_APP_LOG + 3 }, (_, i) => e(i + 1));
    const out = mergeAppLog([], many);
    expect(out).toHaveLength(MAX_APP_LOG);
    expect(out[0].id).toBe(4);
  });

  it("최근 것이 위 · 필터 · 개수", () => {
    const list = [e(1), e(2, "warn"), e(3, "error"), e(4, "warn")];
    expect(visibleAppLog(list, "all").map((x) => x.id)).toEqual([4, 3, 2, 1]);
    expect(visibleAppLog(list, "warn").map((x) => x.id)).toEqual([4, 2]);
    expect(visibleAppLog(list, "error").map((x) => x.id)).toEqual([3]);
    expect(countAppLog(list)).toEqual({ all: 4, warn: 2, error: 1 });
  });

  it("[자세히]로 데려갈 줄의 DOM id", () => {
    expect(settingsElementId({ kind: "log", id: 7 })).toBe("app-log-7");
    expect(settingsElementId({ kind: "section", section: "log" })).toBe("settings-log");
  });
});
