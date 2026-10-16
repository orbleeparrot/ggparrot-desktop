// 상태 카드 현재가 그래프의 눈금 계산 — 선(SVG)과 그 위 글자(HTML)가 같은 축을 쓰게 한곳에 둠.

export interface ChartRange {
  min: number;
  max: number;
}

/** 값 범위. 평단이 있으면 함께 넣어 점선이 그래프 밖으로 나가지 않게 함. 값이 모두 같으면 위아래로 1씩 벌림. */
export function chartRange(points: number[], entry?: number): ChartRange {
  const values = entry && entry > 0 ? [...points, entry] : points;
  let min = Math.min(...values);
  let max = Math.max(...values);
  if (max - min < 1e-9) {
    min -= 1;
    max += 1;
  }
  return { min, max };
}

/** 값 → 세로 위치(위에서부터, 0~height). 위아래 pad만큼은 비워 선이 가장자리에 붙지 않게 함. */
export function chartY(value: number, range: ChartRange, height: number, pad = 4): number {
  return pad + (1 - (value - range.min) / (range.max - range.min)) * (height - pad * 2);
}

/** 처음보다 지금이 같거나 높으면 "up". */
export function chartDirection(points: number[]): "up" | "down" {
  return points.length > 1 && points[points.length - 1] < points[0] ? "down" : "up";
}

/**
 * 세로로 놓이는 글자 꼬리표들의 가운데 위치를 겹치지 않게 고름.
 * - 각 꼬리표는 [half, height - half] 안에 둠(위아래로 잘리지 않게).
 * - 두 꼬리표 사이가 gap보다 가까우면 아래 것을 밀어 내림. 바닥에 닿으면 위로 되밀어 올림.
 * - 돌려주는 순서는 받은 순서와 같음.
 */
export function spreadLabels(ys: number[], height: number, gap: number, half: number): number[] {
  const lo = half;
  const hi = Math.max(lo, height - half);
  const order = ys.map((y, i) => ({ y: Math.min(hi, Math.max(lo, y)), i })).sort((a, b) => a.y - b.y || a.i - b.i);
  for (let k = 1; k < order.length; k++) order[k].y = Math.max(order[k].y, order[k - 1].y + gap);
  if (order.length) order[order.length - 1].y = Math.min(order[order.length - 1].y, hi);
  for (let k = order.length - 2; k >= 0; k--) order[k].y = Math.min(order[k].y, order[k + 1].y - gap);
  const out = new Array<number>(ys.length);
  for (const o of order) out[o.i] = o.y;
  return out;
}
