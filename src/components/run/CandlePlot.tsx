// 매크로 조건 차트 — 봉·지표선·가격선을 한 판에.
//
// 봉도 지표선도 가격선도 **다시 계산하지 않음.** 봉은 거래소에서 받은 그대로, 지표선과
// 가격선은 엔진이 낸 그대로임. 화면이 다시 계산하면 규칙 산수가 두 벌이 되고 엔진과
// 어긋남(예: E 눌림목 선이 현재가를 따라 영원히 내려옴).
//
// 차트는 **한 번만 만들고** 데이터만 갈아끼움. 봉이 마감될 때마다 다시 만들면 보고 있던
// 확대·스크롤이 그때마다 처음으로 돌아감.
//
// 축은 봉에 맞춤. 가격선을 전부 담으려고 축을 늘리면 봉이 납작해져 아무것도 안 보임 —
// BTC에서 익절 +5%·손절 -3%가 5시간치 봉 범위보다 훨씬 넓어 봉 300개가 한 줄이 됨.
// 그래서 축은 candleRange가 정함.
import { useEffect, useId, useRef } from "react";
import {
  CandlestickSeries,
  CrosshairMode,
  LineSeries,
  LineStyle,
  createChart,
  type IChartApi,
  type IPriceLine,
  type ISeriesApi,
  type Time,
} from "lightweight-charts";
import {
  candleRange,
  priceLines,
  type Candle,
  type ChartData,
  type ChartLive,
  type PriceLine,
} from "../../state/candles";
import { formatPrice } from "../../state/format";
import type { MacroInfo } from "../../types";

/** 지표선 색 — 이름으로 고름. 모르는 이름은 회색 참고선. */
const SERIES_TONE: Record<string, string> = {
  breakout: "#16a34a",
  ma_fast: "#ea580c",
  ma_slow: "#6366f1",
  ma_filter: "#6366f1",
  band_upper: "#94a3b8",
  band_mid: "#6366f1",
  band_lower: "#94a3b8",
};

interface Props {
  chart: ChartData | null;
  /** 봉 마감 사이에 움직이는 부분. 없으면 마감봉만 그림. */
  live?: ChartLive | null;
  price: number;
  /** 보유 중이면 평단 — 가로 점선으로 겹침. */
  entry?: number;
  quote: MacroInfo["quote"];
  intervalLabel: string;
}

function cssColor(name: string, fallback: string): string {
  if (typeof window === "undefined") return fallback;
  const value = getComputedStyle(document.documentElement).getPropertyValue(name).trim();
  return value || fallback;
}

function toneOf(line: PriceLine, up: string, down: string, muted: string): string {
  if (line.tone === "buy") return up;
  if (line.tone === "sell") return down;
  return muted;
}

const bar = (c: Candle) => ({ time: (c[0] / 1000) as Time, open: c[1], high: c[2], low: c[3], close: c[4] });

export function CandlePlot({ chart, live = null, price, entry = 0, quote, intervalLabel }: Props) {
  const host = useRef<HTMLDivElement>(null);
  const oscHost = useRef<HTMLDivElement>(null);
  const labelId = useId();
  const bars = chart?.bars ?? [];
  const oscillator = chart?.oscillator ?? null;
  // 엔진이 낸 가격선이 둘 있으면 **새것**(live)을 씀 — 마감봉 payload의 선은 그 봉까지의 값임.
  const levels = live?.levels ?? chart?.levels ?? null;

  const api = useRef<IChartApi | null>(null);
  const osc = useRef<IChartApi | null>(null);
  const candles = useRef<ISeriesApi<"Candlestick"> | null>(null);
  const oscLine = useRef<ISeriesApi<"Line"> | null>(null);
  const lines = useRef(new Map<string, IPriceLine>());
  const oscLines = useRef(new Map<string, IPriceLine>());
  const overlays = useRef(new Map<string, ISeriesApi<"Line">>());
  /** 축 범위 — autoscaleInfoProvider가 읽음. 데이터가 바뀔 때마다 여기에 새로 담음. */
  const span = useRef({ min: 0, max: 1 });
  /** 처음 데이터를 받았을 때 한 번만 전체 보기로 맞춤. 그 뒤 확대·스크롤은 사용자 것임. */
  const fitted = useRef(false);
  /** 직전에 그린 봉들의 시각 — 링버퍼가 앞을 버리면 보고 있던 구간이 밀림. */
  const times = useRef<number[]>([]);

  const hasOscillator = !!oscillator;
  const blank = bars.length < 2;

  // --- 한 번만: 차트를 세움 --------------------------------------------------
  useEffect(() => {
    const node = host.current;
    if (!node || blank) return;

    // 정리 함수가 읽을 것은 여기서 붙잡아 둠 — 그때는 ref가 이미 바뀌어 있을 수 있음.
    const held = lines.current;
    const heldOsc = oscLines.current;
    const heldOverlays = overlays.current;

    const line = cssColor("--line", "#1a2332");
    const muted = cssColor("--muted", "#9ca3af");
    const up = cssColor("--ok", "#16a34a");
    const down = cssColor("--danger", "#dc2626");
    const common = {
      layout: { background: { color: "transparent" }, textColor: muted, fontSize: 11, attributionLogo: false },
      grid: { vertLines: { visible: false }, horzLines: { color: line } },
      rightPriceScale: { borderColor: line },
      timeScale: { borderColor: line, timeVisible: true, secondsVisible: false },
      crosshair: { mode: CrosshairMode.Normal },
      autoSize: true,
    } as const;

    // 아래 칸(RSI)이 있으면 시간축은 거기 하나만 둠 — 두 번 나오면 읽을 것만 늘어남.
    const main = createChart(node, { ...common, timeScale: { ...common.timeScale, visible: !hasOscillator } });
    api.current = main;
    candles.current = main.addSeries(CandlestickSeries, {
      upColor: up,
      downColor: down,
      borderUpColor: up,
      borderDownColor: down,
      wickUpColor: up,
      wickDownColor: down,
      priceLineVisible: false,
      lastValueVisible: true,
      autoscaleInfoProvider: () => ({ priceRange: { minValue: span.current.min, maxValue: span.current.max } }),
    });

    if (hasOscillator && oscHost.current) {
      const below = createChart(oscHost.current, common);
      osc.current = below;
      oscLine.current = below.addSeries(LineSeries, {
        color: "#6366f1",
        lineWidth: 1,
        priceLineVisible: false,
        lastValueVisible: true,
      });
      // 두 차트의 가로축을 묶음. 빗장이 없으면 A가 B를, B가 다시 A를 설정하며 영원히 주고받음.
      let syncing = false;
      const sync = (source: IChartApi, target: IChartApi) =>
        source.timeScale().subscribeVisibleLogicalRangeChange((range) => {
          if (!range || syncing) return;
          syncing = true;
          try {
            target.timeScale().setVisibleLogicalRange(range);
          } finally {
            syncing = false;
          }
        });
      sync(main, below);
      sync(below, main);
    }

    return () => {
      osc.current?.remove();
      main.remove();
      api.current = null;
      osc.current = null;
      candles.current = null;
      oscLine.current = null;
      held.clear();
      heldOsc.clear();
      heldOverlays.clear();
      fitted.current = false;
      times.current = [];
    };
  }, [blank, hasOscillator]);

  // --- 봉이 마감될 때: 봉 배열과 지표선 ---------------------------------------
  const shape = chart ? `${chart.interval}|${bars.length}|${bars.at(-1)?.[0] ?? 0}` : "";
  useEffect(() => {
    const series = candles.current;
    const main = api.current;
    if (!series || !main || blank) return;

    // 링버퍼가 앞 봉을 버리면 같은 자리를 보려면 그만큼 당겨야 함 — 안 하면 보던 구간이 밀림.
    const before = times.current;
    const now = bars.map((b) => b[0]);
    const dropped = before.length ? now.indexOf(before[0]) : -1;
    const keep = fitted.current && dropped > 0 ? main.timeScale().getVisibleLogicalRange() : null;

    series.setData(bars.map(bar));
    times.current = now;

    for (const item of chart?.series ?? []) {
      let overlay = overlays.current.get(item.id);
      if (!overlay) {
        overlay = main.addSeries(LineSeries, {
          color: SERIES_TONE[item.id] ?? cssColor("--muted", "#9ca3af"),
          lineWidth: 1,
          priceLineVisible: false,
          lastValueVisible: false,
          crosshairMarkerVisible: false,
        });
        overlays.current.set(item.id, overlay);
      }
      overlay.setData(
        bars.map((b, i) => {
          const value = item.values[i];
          return value == null ? { time: (b[0] / 1000) as Time } : { time: (b[0] / 1000) as Time, value };
        }),
      );
    }

    if (oscLine.current && oscillator) {
      oscLine.current.setData(
        bars.map((b, i) => {
          const value = oscillator.values[i];
          return value == null ? { time: (b[0] / 1000) as Time } : { time: (b[0] / 1000) as Time, value };
        }),
      );
      syncPriceLines(oscLines.current, oscLine.current, [
        ...(oscillator.entry == null
          ? []
          : [{ key: "osc-in", label: "사는 문턱", value: oscillator.entry, color: cssColor("--ok", "#16a34a") }]),
        ...(oscillator.exit == null
          ? []
          : [{ key: "osc-out", label: "파는 문턱", value: oscillator.exit, color: cssColor("--danger", "#dc2626") }]),
      ]);
    }

    if (!fitted.current) {
      main.timeScale().fitContent();
      osc.current?.timeScale().fitContent();
      fitted.current = true;
    } else if (keep && dropped > 0) {
      main.timeScale().setVisibleLogicalRange({ from: keep.from - dropped, to: keep.to - dropped });
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps -- shape가 내용 변화를 대신함
  }, [shape, blank]);

  // --- 매 바퀴: 만들어지는 중인 봉과 가격선 -----------------------------------
  const drawn = priceLines(levels, live?.live?.[4] ?? bars.at(-1)?.[4] ?? 0);
  const tick = `${live?.live?.join(",") ?? ""}|${drawn.map((l) => `${l.key}:${l.value}`).join(",")}|${entry}`;
  useEffect(() => {
    const series = candles.current;
    if (!series || blank) return;

    if (live?.live) series.update(bar(live.live));

    const up = cssColor("--ok", "#16a34a");
    const down = cssColor("--danger", "#dc2626");
    const muted = cssColor("--muted", "#9ca3af");
    syncPriceLines(lines.current, series, [
      ...drawn.map((item) => ({
        key: item.key,
        label: item.label,
        value: item.value,
        color: toneOf(item, up, down, muted),
      })),
      ...(entry > 0 ? [{ key: "entry", label: "평단", value: entry, color: muted, dotted: true }] : []),
    ]);

    // 축은 봉과 지금 선을 함께 담되, 먼 선 때문에 봉이 납작해지지 않게 candleRange가 정함.
    const rows = live?.live ? [...bars, live.live] : bars;
    span.current = candleRange(rows, drawn, entry);
    series.applyOptions({
      autoscaleInfoProvider: () => ({ priceRange: { minValue: span.current.min, maxValue: span.current.max } }),
    });
    // eslint-disable-next-line react-hooks/exhaustive-deps -- tick이 내용 변화를 대신함
  }, [tick, blank]);

  if (blank) {
    return (
      <div className="candle-plot is-empty" role="img" aria-label="봉 차트 준비 중">
        <p className="candle-empty-note">
          {levels?.pending ? "첫 봉이 마감되면 조건선이 나와요." : "봉을 받는 중이에요."}
        </p>
      </div>
    );
  }

  return (
    <div className="candle-plot">
      <div className="candle-head">
        <span className="candle-interval">
          {intervalLabel} 봉 {bars.length}개
        </span>
        <span className="candle-price">
          {formatPrice(price, quote)} {quote}
        </span>
      </div>
      <div ref={host} className="candle-canvas" role="img" aria-labelledby={labelId} />
      {oscillator && <div ref={oscHost} className="candle-osc" aria-hidden="true" />}
      <ul className="candle-legend" id={labelId}>
        {(chart?.series ?? []).map((item) => (
          <li key={item.id} className="candle-legend-item">
            <i style={{ background: SERIES_TONE[item.id] ?? "currentColor" }} aria-hidden="true" />
            {item.label}
          </li>
        ))}
        {drawn.map((item) => (
          <li key={item.key} className={`candle-legend-item tone-${item.tone}`}>
            <i aria-hidden="true" />
            {item.label} {formatPrice(item.value, quote)}
          </li>
        ))}
        {oscillator && <li className="candle-legend-item">{oscillator.label}</li>}
      </ul>
    </div>
  );
}

interface Drawn {
  key: string;
  label: string;
  value: number;
  color: string;
  dotted?: boolean;
}

/**
 * 가격선을 **지우고 다시 긋지 않고** 값만 고침. 매 바퀴 지웠다 그으면 선이 깜빡이고,
 * 보고 있던 자리도 흔들림.
 */
function syncPriceLines(
  held: Map<string, IPriceLine>,
  series: ISeriesApi<"Candlestick"> | ISeriesApi<"Line">,
  want: Drawn[],
) {
  const keys = new Set(want.map((item) => item.key));
  for (const [key, line] of held) {
    if (keys.has(key)) continue;
    series.removePriceLine(line);
    held.delete(key);
  }
  for (const item of want) {
    const options = {
      price: item.value,
      color: item.color,
      lineWidth: 1 as const,
      lineStyle: item.dotted ? LineStyle.Dotted : LineStyle.Dashed,
      axisLabelVisible: true,
      title: item.label,
    };
    const line = held.get(item.key);
    if (line) line.applyOptions(options);
    else held.set(item.key, series.createPriceLine(options));
  }
}
