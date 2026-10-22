// 봉 차트의 눈금 계산 — 캔들(SVG)과 그 위 글자(HTML)가 같은 축을 쓰게 한곳에 둠.
//
// 값은 **엔진이 낸 그대로** 받는다. 화면이 가격선을 다시 계산하면 규칙 산수가 두 벌이 되고,
// 실제로 그 일이 있었다 — E 눌림목의 '진입 목표 −3%' 를 웹이 현재가 기준으로 그려 가격을
// 따라 영원히 내려왔고, 사용자는 "안 사진다" 로 읽었다.

/** 실행기가 보내는 봉 하나 — `[열린 시각(ms), 시가, 고가, 저가, 종가]`. */
export type Candle = [number, number, number, number, number];

/** 가격이 아닌 지표(RSI 등) — 가격축에 섞으면 축이 망가져 아래 칸에 따로 그림. */
export interface ChartIndicator {
  name: string;
  value: number;
  entry_threshold: number | null;
  exit_threshold: number | null;
}

/** 엔진이 낸 가격선. 모르는 선은 null 이고 그리지 않음. */
export interface ChartLevels {
  entry_target: number | null;
  exit_target: number | null;
  stop_loss: number | null;
  trail_stop: number | null;
  trail_activation: number | null;
  armed: boolean;
  reference: number | null;
  band_upper: number | null;
  band_lower: number | null;
  buy_levels?: number[];
  sell_levels?: number[];
  next_buy_ms: number | null;
  indicator: ChartIndicator | null;
  /** 아직 첫 봉이 안 닫혀 값이 없음 — "이 규칙엔 그 선이 없음"과 다름. */
  pending: boolean;
}

export interface ChartData {
  interval: string;
  bars: Candle[];
  levels: ChartLevels;
}

/** 선 하나 — 화면에 그릴 가격과 이름. */
export interface PriceLine {
  key: string;
  label: string;
  value: number;
  /** 사는 쪽(초록) · 파는 쪽(빨강) · 참고(회색). */
  tone: "buy" | "sell" | "info";
}

function usable(value: number | null | undefined): value is number {
  return typeof value === "number" && Number.isFinite(value) && value > 0;
}

/**
 * 그릴 가격선 목록. 엔진이 낸 값만 들어가고, 없는 선은 아예 빠짐.
 *
 * 그리드처럼 선이 여럿인 규칙은 가까운 몇 개만 — 전부 그리면 차트가 줄무늬가 됨.
 */
export function priceLines(levels: ChartLevels | null | undefined, price: number): PriceLine[] {
  if (!levels) return [];
  const out: PriceLine[] = [];
  const add = (key: string, label: string, value: number | null, tone: PriceLine["tone"]) => {
    if (usable(value)) out.push({ key, label, value, tone });
  };
  add("entry", "사는 선", levels.entry_target, "buy");
  add("exit", "파는 선", levels.exit_target, "sell");
  add("stop", "손절선", levels.stop_loss, "sell");
  add("trail", "트레일 손절", levels.trail_stop, "sell");
  // 발동 전에만 뜻이 있음 — 켜진 뒤에는 엔진이 null 을 보냄.
  add("arm", "트레일 발동", levels.trail_activation, "info");
  add("ref", "기준선", levels.reference, "info");
  add("band-up", "위 밴드", levels.band_upper, "info");
  add("band-low", "아래 밴드", levels.band_lower, "info");

  // 그리드 칸 — 지금 가격에서 가까운 것부터 둘씩만.
  const near = (values: number[] | undefined, count: number) =>
    [...(values ?? [])]
      .filter(usable)
      .sort((a, b) => Math.abs(a - price) - Math.abs(b - price))
      .slice(0, count);
  near(levels.buy_levels, 2).forEach((value, i) =>
    out.push({ key: `grid-buy-${i}`, label: "격자 매수", value, tone: "buy" }),
  );
  near(levels.sell_levels, 2).forEach((value, i) =>
    out.push({ key: `grid-sell-${i}`, label: "격자 매도", value, tone: "sell" }),
  );
  return out;
}

export interface CandleRange {
  min: number;
  max: number;
}

/**
 * 봉과 가격선을 담는 값 범위.
 *
 * 선이 차트 밖으로 나가면 "어디서 사는지" 를 볼 수 없지만, 선을 담으려고 축을 무한정
 * 늘리면 이번엔 **봉이 납작해져** 아무것도 안 보인다 — BTC 에서 익절 +5% · 손절 -3% 가
 * 5시간치 봉 범위(약 0.7%)보다 훨씬 넓어 봉 300개가 한 줄이 됐다.
 *
 * 그래서 선은 봉 범위의 {@link LINE_ROOM} 배까지만 축을 늘린다. 더 먼 선은 가장자리에
 * 붙어 "그쪽에 있다" 만 말한다(값은 꼬리표에 그대로 적힌다).
 */
export const LINE_ROOM = 1.5;

export function candleRange(bars: Candle[], lines: PriceLine[] = [], entry = 0): CandleRange {
  const values: number[] = [];
  for (const [, , high, low] of bars) {
    if (Number.isFinite(high)) values.push(high);
    if (Number.isFinite(low)) values.push(low);
  }
  if (values.length === 0) {
    // 봉이 없으면 늘릴 기준도 없다 — 선만이라도 담는다.
    for (const line of lines) values.push(line.value);
    if (usable(entry)) values.push(entry);
    if (values.length === 0) return { min: 0, max: 1 };
  }
  let min = Math.min(...values);
  let max = Math.max(...values);
  if (bars.length > 0) {
    const span = max - min || Math.abs(max) * 0.01 || 1;
    const floor = min - span * LINE_ROOM;
    const ceiling = max + span * LINE_ROOM;
    const include = (value: number) => {
      if (!Number.isFinite(value)) return;
      min = Math.min(min, Math.max(value, floor));
      max = Math.max(max, Math.min(value, ceiling));
    };
    for (const line of lines) include(line.value);
    if (usable(entry)) include(entry);
  }
  if (max - min < 1e-9) {
    // 값이 모두 같음 — 위아래로 조금 벌려 평평한 선이 가장자리에 붙지 않게.
    const pad = Math.abs(max) * 0.01 || 1;
    min -= pad;
    max += pad;
  }
  return { min, max };
}

/**
 * 값 → 세로 위치(위에서부터 0~height). 위아래 pad 만큼은 비움.
 *
 * 범위 밖의 값은 가장자리에 붙인다 — {@link candleRange} 가 먼 가격선을 담지 않으므로
 * 묶지 않으면 그 선이 차트 밖에 그려진다.
 */
export function candleY(value: number, range: CandleRange, height: number, pad = 6): number {
  const span = range.max - range.min || 1;
  const y = pad + (1 - (value - range.min) / span) * (height - pad * 2);
  return Math.min(Math.max(y, pad), height - pad);
}

/** 봉 `i` 의 가로 중심(0~width). 봉 사이 간격은 고르게. */
export function candleX(index: number, count: number, width: number, pad = 2): number {
  if (count <= 1) return width / 2;
  const step = (width - pad * 2) / count;
  return pad + step * (index + 0.5);
}

/** 봉 하나의 너비(px). 너무 좁아지면 선처럼 보이게 1px 로. */
export function candleWidth(count: number, width: number, pad = 2): number {
  if (count <= 0) return 1;
  const step = (width - pad * 2) / count;
  return Math.max(1, step * 0.66);
}

/** 차트가 덮는 시간 — "1시간 봉 300개 · 12일" 처럼 보여 줄 때 쓰는 ms. */
export function chartSpanMs(bars: Candle[]): number {
  if (bars.length < 2) return 0;
  return bars[bars.length - 1][0] - bars[0][0];
}
