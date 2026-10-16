import { describe, expect, it } from "vitest";
import { makeRun, makeSlot, makeState } from "../test/fixtures";
import { appendPrice, downsampleTrail, MAX_TRAIL_POINTS, trackPrices, trailSpanLabel } from "./priceTrail";

/** 1초 간격 점 n개 — 값은 톱니(0~9) 위에 i번째에 큰 봉우리 하나. */
function series(n: number, spike = -1) {
  const points = Array.from({ length: n }, (_, i) => (i === spike ? 500 : 100 + (i % 10)));
  const times = Array.from({ length: n }, (_, i) => 1_000_000 + i * 1000);
  return { points, times };
}

describe("현재가 점 줄이기", () => {
  it("상한 안이면 그대로", () => {
    const s = series(50);
    const out = downsampleTrail(s.points, s.times, 240);
    expect(out.points).toBe(s.points);
    expect(out.times).toBe(s.times);
  });

  it("넘으면 상한 아래로 — 첫 점(시작 시각)·마지막 점·전체 최고·최저는 남고 시각은 오름차순", () => {
    const s = series(400, 37);
    const out = downsampleTrail(s.points, s.times, 240);
    expect(out.points.length).toBeLessThanOrEqual(240);
    expect(out.points.length).toBe(out.times.length);
    expect(out.times[0]).toBe(s.times[0]);
    expect(out.times[out.times.length - 1]).toBe(s.times[399]);
    expect(Math.max(...out.points)).toBe(500);
    expect(Math.min(...out.points)).toBe(100);
    for (let i = 1; i < out.times.length; i++) expect(out.times[i]).toBeGreaterThan(out.times[i - 1]);
    // 최근 1/4은 줄이지 않음.
    expect(out.times.slice(-60)).toEqual(s.times.slice(-60));
  });

  it("계속 더해도 상한을 넘지 않고 시작부터 지금까지를 덮음", () => {
    let trail = { startedAt: 1_000_000, points: [100], times: [1_000_000] };
    for (let i = 1; i < 3000; i++) trail = appendPrice(trail, 100 + (i % 7) + i / 1000, 1_000_000 + i * 5000);
    expect(trail.points.length).toBeLessThanOrEqual(MAX_TRAIL_POINTS);
    expect(trail.times[0]).toBe(1_000_000);
    expect(trail.times[trail.times.length - 1]).toBe(1_000_000 + 2999 * 5000);
  });

  it("값이 같으면 점을 더하지 않음", () => {
    const t = { startedAt: 1, points: [100], times: [1] };
    expect(appendPrice(t, 100, 2)).toBe(t);
    expect(appendPrice(t, 0, 2)).toBe(t);
  });

  it("상태 스냅샷마다 슬롯별로 쌓고, 실행이 바뀌면 새로 시작함", () => {
    const run = makeRun({ startedAt: 1000, updatedAt: 2000, price: 100 });
    const s1 = makeState({ runs: [makeSlot({ id: "a", phase: "running", run })] });
    const p1 = trackPrices({}, s1);
    expect(p1.a.points).toEqual([100]);
    const s2 = makeState({
      runs: [makeSlot({ id: "a", phase: "running", run: { ...run, price: 101, updatedAt: 3000 } })],
    });
    const p2 = trackPrices(p1, s2);
    expect(p2.a).toEqual({ startedAt: 1000, points: [100, 101], times: [2000, 3000] });
    expect(trackPrices(p2, s2)).toBe(p2);
    const s3 = makeState({
      runs: [makeSlot({ id: "a", phase: "running", run: { ...run, startedAt: 9000, price: 90, updatedAt: 9500 } })],
    });
    expect(trackPrices(p2, s3).a.points).toEqual([90]);
  });
});

describe("그래프가 덮는 시간 이름", () => {
  const start = 1_000_000;
  it("첫 점이 시작 직후면 '실행 시작부터' — 시작 시각부터 셈", () => {
    expect(trailSpanLabel(start, start + 5_000, start + 80 * 60_000)).toBe("실행 시작부터 · 1시간 20분");
    expect(trailSpanLabel(start, start + 3_000, start + 45_000)).toBe("실행 시작부터 · 45초");
  });
  it("앱을 실행 뒤에 열어 앞부분이 없으면 '앱을 연 뒤부터' — 첫 점부터 셈", () => {
    expect(trailSpanLabel(start, start + 30 * 60_000, start + 42 * 60_000)).toBe("앱을 연 뒤부터 · 12분");
    expect(trailSpanLabel(start, start + 86_400_000, start + 86_400_000 * 3 + 3_600_000)).toBe(
      "앱을 연 뒤부터 · 2일 1시간",
    );
  });
  it("점이 하나뿐이거나 시각을 모르면 비움", () => {
    expect(trailSpanLabel(start, 0, start + 1000)).toBe("");
    expect(trailSpanLabel(start, start + 5000, start + 5000)).toBe("");
  });
});
