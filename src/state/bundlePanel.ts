// 묶음 판이 쓰는 셈 — 레그 모으기·전체 손익·등락률 흐름 (2026-11-02).
//
// 화면에서 떼어 둠: 비중이 다른 묶음의 합산은 눈으로 못 세고, 한 레그라도 수수료를
// 모르면 전체 값의 뜻이 달라짐. 그 판단을 시험으로 못 박아 둘 자리가 필요함.
import type { RunSlot } from "../types";

/** 같은 묶음의 레그들 — 묶음 안 번호 순. */
export function bundleLegs(runs: RunSlot[], key: string): RunSlot[] {
  return runs.filter((r) => r.bundle?.key === key).sort((a, b) => (a.bundle?.index ?? 0) - (b.bundle?.index ?? 0));
}

export interface BundleTotals {
  /** 지금 포지션에 들어간 돈의 합. 아무도 안 들고 있으면 0. */
  invested: number;
  /** 평가손익의 합. */
  unrealized: number;
  /** 실현손익의 합(수수료 반영분을 알면 그것, 모르면 그대로). */
  realized: number;
  /** 지금 들고 있는 것의 **평단 대비**(%). 아무도 안 들고 있으면 null. */
  pct: number | null;
  /** 한 레그라도 수수료를 모르면 거짓 — 화면이 '수수료 전'을 붙임. */
  feeAware: boolean;
  /** 지금 무언가 들고 있는 레그 수. */
  holding: number;
}

/** 묶음 전체 손익.
 *
 * `pct`는 **평가손익을 투입금으로 나눈 것** — 레그마다 적히는 '평단 대비'를 투입금으로
 * 가중한 값이라 표와 머리 숫자가 같은 뜻임.
 *
 * 실현손익은 여기 안 섞음. 섞으면 분모가 어긋남 — 투입금은 '지금 들고 있는 돈'이고
 * 실현손익은 '오늘 하루 치'라, 많이 사고팔고 지금 조금만 들고 있으면 몇백 %가 나옴
 * (실측: 투입금 100에 실현 -408 → -407.83%). 실현손익은 따로 적음. */
export function bundleTotals(legs: RunSlot[]): BundleTotals {
  let invested = 0;
  let unrealized = 0;
  let realized = 0;
  let feeAware = true;
  let holding = 0;
  for (const leg of legs) {
    const run = leg.run;
    if (!run) continue;
    invested += run.invested || 0;
    unrealized += run.unrealized || 0;
    realized += run.realizedNet ?? run.realized ?? 0;
    if (run.inPosition) {
      holding += 1;
      // 보유 중인 레그만 따짐 — 안 들고 있으면 평가손익이 0이라 수수료 기준이 뜻이 없음.
      if (!run.unrealizedFeeAware) feeAware = false;
    }
  }
  const pct = invested > 0 ? (unrealized / invested) * 100 : null;
  return { invested, unrealized, realized, pct, feeAware, holding };
}

/** 첫 점 대비 등락률(%) 흐름. 점이 둘 미만이거나 첫 점이 0 이하면 빈 배열. */
export function changeSeries(points: number[]): number[] {
  if (points.length < 2) return [];
  const base = points[0];
  if (!(base > 0)) return [];
  return points.map((p) => ((p - base) / base) * 100);
}
