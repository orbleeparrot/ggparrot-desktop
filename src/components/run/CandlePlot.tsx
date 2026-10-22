// 매크로 조건 차트 — 봉 · 지표선 · 가격선을 한 판에.
//
// 봉도 지표선도 가격선도 **다시 계산하지 않는다.** 봉은 거래소에서 받은 그대로, 지표선과
// 가격선은 엔진이 낸 그대로다. 화면이 다시 계산하면 규칙 산수가 두 벌이 되고, 실제로 그
// 일이 있었다(E 눌림목 선이 현재가를 따라 영원히 내려왔다).
//
// 축은 봉에 맞춘다. 가격선을 전부 담으려고 축을 늘리면 봉이 납작해져 아무것도 안 보인다 —
// BTC 에서 익절 +5% · 손절 -3% 가 5시간치 봉 범위보다 훨씬 넓어 봉 300개가 한 줄이 됐다.
// 그래서 축은 candleRange 가 정한다 — 먼 선은 봉 범위의 일정 배까지만 축을 늘린다.
import { useEffect, useId, useRef } from "react";
import {
  CandlestickSeries,
  CrosshairMode,
  LineSeries,
  LineStyle,
  createChart,
  type IChartApi,
  type ISeriesApi,
  type Time,
} from "lightweight-charts";
import { candleRange, priceLines, type ChartData, type PriceLine } from "../../state/candles";
import { formatPrice } from "../../state/format";
import type { MacroInfo } from "../../types";

/** 지표선 색 — 이름으로 고른다. 모르는 이름은 회색 참고선. */
const SERIES_TONE: Record<string, string> = {
  ma_fast: "#ea580c",
  ma_slow: "#6366f1",
  ma_filter: "#6366f1",
  band_upper: "#94a3b8",
  band_mid: "#6366f1",
  band_lower: "#94a3b8",
};

interface Props {
  chart: ChartData | null;
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

export function CandlePlot({ chart, price, entry = 0, quote, intervalLabel }: Props) {
  const host = useRef<HTMLDivElement>(null);
  const oscHost = useRef<HTMLDivElement>(null);
  const labelId = useId();
  const bars = chart?.bars ?? [];
  const oscillator = chart?.oscillator ?? null;

  // 차트 payload 는 **봉이 마감될 때만** 바뀌는데 그것을 담은 객체는 상태가 갱신될
  // 때마다 새로 만들어진다. 객체를 그대로 의존성에 두면 5초마다 차트를 통째로 다시
  // 그려 깜빡인다 — 내용이 달라졌을 때만 다시 그린다.
  const signature = chart
    ? [chart.interval, bars.length, bars.at(-1)?.[0] ?? 0, JSON.stringify(chart.levels)].join("|")
    : "";

  useEffect(() => {
    const node = host.current;
    if (!node || bars.length < 2) return;

    const up = cssColor("--ok", "#16a34a");
    const down = cssColor("--danger", "#dc2626");
    const muted = cssColor("--muted", "#9ca3af");
    const line = cssColor("--line", "#1a2332");

    const common = {
      layout: { background: { color: "transparent" }, textColor: muted, fontSize: 11, attributionLogo: false },
      grid: { vertLines: { visible: false }, horzLines: { color: line } },
      rightPriceScale: { borderColor: line },
      timeScale: { borderColor: line, timeVisible: true, secondsVisible: false },
      crosshair: { mode: CrosshairMode.Normal },
      autoSize: true,
    } as const;

    // 아래 칸(RSI)이 있으면 시간축은 거기 하나만 둔다 — 두 번 나오면 읽을 것만 늘어난다.
    const api: IChartApi = createChart(node, {
      ...common,
      timeScale: { ...common.timeScale, visible: !oscillator },
    });
    const candles = api.addSeries(CandlestickSeries, {
      upColor: up,
      downColor: down,
      borderUpColor: up,
      borderDownColor: down,
      wickUpColor: up,
      wickDownColor: down,
      priceLineVisible: false,
      lastValueVisible: true,
    });
    const rows = bars.map(([t, o, h, l, c]) => ({ time: (t / 1000) as Time, open: o, high: h, low: l, close: c }));
    candles.setData(rows);

    // 봉만으로 축을 잡고, 먼 가격선은 봉 범위의 LINE_ROOM 배까지만 축을 늘린다.
    // 격자 칸을 고르는 기준은 **마지막 종가** — 틱마다 바뀌는 현재가를 쓰면 차트를
    // 계속 다시 그리게 된다. 봉이 닫힐 때 같이 바뀌면 충분하다.
    const drawn = priceLines(chart?.levels, bars.at(-1)?.[4] ?? 0);
    const range = candleRange(bars, drawn, entry);
    candles.applyOptions({
      autoscaleInfoProvider: () => ({ priceRange: { minValue: range.min, maxValue: range.max } }),
    });

    for (const item of chart?.series ?? []) {
      const series = api.addSeries(LineSeries, {
        color: SERIES_TONE[item.id] ?? muted,
        lineWidth: 1,
        priceLineVisible: false,
        lastValueVisible: false,
        crosshairMarkerVisible: false,
      });
      series.setData(
        bars.map(([t], i) => {
          const value = item.values[i];
          return value == null ? { time: (t / 1000) as Time } : { time: (t / 1000) as Time, value };
        }),
      );
    }

    for (const item of drawn) {
      candles.createPriceLine({
        price: item.value,
        color: toneOf(item, up, down, muted),
        lineWidth: 1,
        lineStyle: LineStyle.Dashed,
        axisLabelVisible: true,
        title: item.label,
      });
    }
    if (entry > 0) {
      candles.createPriceLine({
        price: entry,
        color: muted,
        lineWidth: 1,
        lineStyle: LineStyle.Dotted,
        axisLabelVisible: true,
        title: "평단",
      });
    }
    api.timeScale().fitContent();

    // RSI 는 0~100 이라 가격축에 섞을 수 없다 — 아래 칸에 따로.
    let osc: IChartApi | null = null;
    if (oscillator && oscHost.current) {
      osc = createChart(oscHost.current, { ...common, rightPriceScale: { borderColor: line } });
      const series: ISeriesApi<"Line"> = osc.addSeries(LineSeries, {
        color: "#6366f1",
        lineWidth: 1,
        priceLineVisible: false,
        lastValueVisible: true,
      });
      series.setData(
        bars.map(([t], i) => {
          const value = oscillator.values[i];
          return value == null ? { time: (t / 1000) as Time } : { time: (t / 1000) as Time, value };
        }),
      );
      for (const [value, title, color] of [
        [oscillator.entry, "사는 문턱", up],
        [oscillator.exit, "파는 문턱", down],
      ] as const) {
        if (value == null || !Number.isFinite(value)) continue;
        series.createPriceLine({
          price: value,
          color,
          lineWidth: 1,
          lineStyle: LineStyle.Dashed,
          axisLabelVisible: true,
          title,
        });
      }
      osc.timeScale().fitContent();
      // 두 차트의 가로축을 묶는다 — 따로 놀면 같은 시각을 다른 자리에서 본다.
      const sync = (source: IChartApi, target: IChartApi) =>
        source.timeScale().subscribeVisibleLogicalRangeChange((range) => {
          if (range) target.timeScale().setVisibleLogicalRange(range);
        });
      sync(api, osc);
      sync(osc, api);
    }

    return () => {
      osc?.remove();
      api.remove();
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps -- signature 가 내용 변화를 대신한다
  }, [signature, entry]);

  if (bars.length < 2) {
    return (
      <div className="candle-plot is-empty" role="img" aria-label="봉 차트 준비 중">
        <p className="candle-empty-note">
          {chart?.levels?.pending ? "첫 봉이 마감되면 조건선이 나와요." : "봉을 받는 중이에요."}
        </p>
      </div>
    );
  }

  const lines = priceLines(chart?.levels, bars.at(-1)?.[4] ?? 0);
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
        {lines.map((item) => (
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
