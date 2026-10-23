// mock 실행 루프 — 현재가를 흔들고, 신호·체결·보류·연결 문제를 사람이 읽는 활동과 원문 줄(dev)로 냄.
import { EXCHANGE_LABELS, type MockRun } from "./mockData";
import { formatMoney, formatPrice, formatQty, moneyDigits, toneOf } from "./state/format";

const PLAIN = new Intl.NumberFormat("en-US", { maximumFractionDigits: 2 });
/** 문장 안의 가격 — 군더더기 0 없이("81,806"·"81,831.58"). */
const plain = (price: number, quote: "USDT" | "KRW") =>
  quote === "KRW" ? formatPrice(price, quote) : PLAIN.format(price);
import { MARKET_LABELS, MODE_LABELS, SIDE_LABELS } from "./state/runs";
import type { Activity, ActivityKind, EntryWait, Tone } from "./types";

export type Emit = (
  r: MockRun,
  a: { kind: ActivityKind; title: string; detail?: string; raw?: string; dev?: boolean; tone?: Tone },
  at: number,
) => void;

/** 모의 수수료율 — Rust 기본값(runner-core pnl.rs)과 같음: 바이낸스 현물 0.1%·선물 0.05%·업비트 0.05%·빗썸 0.25%. */
function feeRate(m: MockRun["macro"]): number {
  if (m.exchange === "upbit") return 0.0005;
  if (m.exchange === "bithumb") return 0.0025;
  return m.market === "futures" ? 0.0005 : 0.001;
}

/** 지금 보유분의 남은 진입 수수료(견적 통화) — 화면 계약에는 없는 봇 내부 값이라 따로 둠. */
const openFees = new WeakMap<object, number>();

type Run = NonNullable<MockRun["run"]>;

const START_PRICE: Record<string, number> = {
  BTCUSDT: 81806,
  ETHUSDT: 3120.4,
  SOLUSDT: 182.35,
  "KRW-BTC": 118_350_000,
  ETH_KRW: 4_812_000,
  XRP_KRW: 3120,
};

function round(price: number, quote: "USDT" | "KRW"): number {
  if (quote === "KRW") return price >= 100_000 ? Math.round(price / 1000) * 1000 : Math.round(price);
  return price >= 1000 ? Math.round(price * 100) / 100 : Math.round(price * 10000) / 10000;
}

function orderCap(r: MockRun): number {
  return r.macro.quote === "USDT" ? 100 : 150_000;
}

/** 구조화 활동 하나 — 바로 앞의 원문 줄을 dev로 먼저 내고, 같은 줄을 raw로 들고 있음. */
function say(
  emit: Emit,
  r: MockRun,
  at: number,
  kind: ActivityKind,
  title: string,
  detail: string,
  raw: string,
  tone: Tone = "muted",
) {
  emit(r, { kind, title: raw, dev: true, tone: "muted" }, at);
  emit(r, { kind, title, detail, raw, tone }, at);
}

/** 진입 체결 — 수수료를 더하고(아는 경우) 남은 진입 수수료로 쌓음. 활동 문구("수수료 0.04 USDT")를 돌려줌. */
function payEntryFee(r: MockRun, run: Run, notional: number): string {
  const fee = notional * feeRate(r.macro);
  openFees.set(run, (openFees.get(run) ?? 0) + fee);
  if (run.fees === null) return "";
  run.fees += fee;
  return `수수료 ${formatPrice(fee, r.macro.quote)} ${r.macro.quote}`;
}

/** 정리 체결 — Rust 봇(orders.rs apply_fill·fill_activity)과 같은 계산·문구. 활동 detail·tone을 돌려줌.
 *  실현손익 = 가격 손익 − 정리 수수료 − 남은 진입 수수료(전량 정리). 수수료를 모르면 수수료 전 값에 표시. */
function settle(r: MockRun, run: Run): { detail: string; tone: Tone; amount: number } {
  const q = r.macro.quote;
  const dir = r.macro.side === "long" ? 1 : -1;
  const gross = (run.price - run.entryPrice) * run.qty * dir;
  const exitFee = run.qty * run.price * feeRate(r.macro);
  const fees = (openFees.get(run) ?? 0) + exitFee;
  openFees.set(run, 0);
  run.realized += gross;
  let parts: string[];
  let amount: number;
  if (run.fees !== null && run.realizedNet !== null) {
    run.fees += exitFee;
    run.realizedNet += gross - fees;
    amount = gross - fees;
    parts = [
      `실현손익 ${formatMoney(amount, q)}`,
      `이번 실행 누적 ${formatMoney(run.realizedNet, q)}`,
      `수수료 ${formatPrice(fees, q)} ${q}`,
    ];
  } else {
    amount = gross;
    parts = [
      `실현손익 ${formatMoney(gross, q)}(수수료 제외 전)`,
      `이번 실행 누적 ${formatMoney(run.realized, q)}(수수료 제외 전)`,
    ];
  }
  if (r.mode === "mock") parts.push("모의 체결");
  const t = toneOf(amount, moneyDigits(q));
  return { detail: parts.join(" · "), tone: t === "up" ? "ok" : t === "down" ? "danger" : "muted", amount };
}

/** 평가손익(지금 정리하면)·투입금을 Rust(bot.rs ui_snapshot·render.rs)와 같은 식으로 다시 셈. */
export function refreshPnl(r: MockRun) {
  const run = r.run;
  if (!run) return;
  const dir = r.macro.side === "long" ? 1 : -1;
  if (!run.inPosition || run.entryPrice <= 0) {
    Object.assign(run, { unrealized: 0, unrealizedPct: 0, invested: 0, unrealizedFeeAware: false });
    return;
  }
  const gross = (run.price - run.entryPrice) * run.qty * dir;
  const openFee = openFees.get(run) ?? 0;
  if (run.fees !== null) {
    const invested = run.qty * run.entryPrice + openFee;
    const net = gross - openFee - run.qty * run.price * feeRate(r.macro);
    Object.assign(run, { unrealized: net, unrealizedPct: (net / invested) * 100, invested, unrealizedFeeAware: true });
  } else {
    Object.assign(run, {
      unrealized: gross,
      unrealizedPct: ((run.price - run.entryPrice) / run.entryPrice) * 100 * dir,
      invested: run.qty * run.entryPrice,
      unrealizedFeeAware: false,
    });
  }
}

/** 미리보기용 — 정해진 평단·현재가로 보유 중인 상태를 만듦(?demo=pnl 스크린샷). */
export function holdAt(r: MockRun, entry: number, price: number, fixedQty?: number) {
  const run = r.run;
  if (!run) return;
  const qty = fixedQty ?? Math.floor((r.macro.orderCap / entry) * 100000) / 100000;
  Object.assign(run, { inPosition: true, entryPrice: entry, qty, price });
  r.holdPrice = price;
  openFees.set(run, 0);
  payEntryFee(r, run, qty * entry);
  refreshPnl(r);
}

/** 조건이 와야 사는 전략인지 — 실제 판단은 서버(entry_wait.is_waiting)가 규칙 종류로 함. */
export function waitsForEntry(strategy: string): boolean {
  return /RSI|볼린저|트레일링|돌파|이동평균/.test(strategy);
}

/** 진입 대기 흉내 — 실제 값은 서버(entry_wait.py)가 엔진과 같은 규칙으로 계산해 보냄. */
export function mockEntryWait(at: number, tick: number): EntryWait {
  const rsi = 38 + ((tick * 7) % 9);
  const minute = 60_000;
  const next = Math.ceil((at + 1) / minute) * minute;
  return {
    rule: "F",
    interval: "1m",
    interval_label: "1분",
    realtime: false,
    sentence: "RSI(14)가 30 아래로 내려가면 사요.",
    distance: `지금 RSI ${rsi.toFixed(1)} · ${(rsi - 30).toFixed(1)} 더 내려가면`,
    gap: `RSI ${rsi.toFixed(1)}`,
    reached: false,
    meter: {
      from: { label: "청산 기준", value: "70" },
      to: { label: "사는 선", value: "30" },
      now: rsi.toFixed(1),
      progress: (70 - rsi) / 40,
    },
    // 서버 _ladder 와 같은 눈금 — 30 · 70 · 지금 값에 15% 여유(24 ~ 76), 사는 구역은 30 아래.
    scale: {
      marks: [
        { key: "line", label: "사는 선", value: "30", pos: (30 - 24) / 52, main: true },
        { key: "exit", label: "청산 기준", value: "70", pos: (70 - 24) / 52, main: false },
      ],
      now: { label: "지금", value: rsi.toFixed(1), pos: (rsi - 24) / 52 },
      zone: (30 - 24) / 52,
      zone_side: "below",
    },
    buy_line: null,
    next_check_ms: next,
    checks: Math.floor(tick / 12),
    last_check_ms: tick >= 12 ? next - minute : null,
    typical: { episodes: 21, span_ms: 300 * minute, avg_ms: 14 * minute, longest_ms: 52 * minute },
    log: tick >= 12 ? [{ at_ms: next - minute, text: `RSI ${rsi.toFixed(1)} · 사는 선 30`, hit: false }] : [],
  };
}

/**
 * 디자인 검토용 봉 차트 한 벌. 실제로는 실행기가 거래소에서 받은 봉과 엔진이 낸 선이지만,
 * 브라우저 미리보기에는 둘 다 없으므로 모양만 같은 값을 만든다 — 화면이 값을 다시 계산하지
 * 않는지 보려면 `src/components/run/CandlePlot.test.tsx` 쪽을 본다.
 */
function mockChart(price: number, interval: string, withRsi: boolean): Run["chart"] {
  const step = 15 * 60_000;
  const start = Date.now() - 120 * step;
  const bars: [number, number, number, number, number][] = [];
  let close = price * 0.97;
  for (let i = 0; i < 120; i += 1) {
    const drift = Math.sin(i / 9) * price * 0.004 + (price * 0.03 * i) / 120;
    const open = close;
    close = price * 0.97 + drift;
    bars.push([start + i * step, open, Math.max(open, close) * 1.002, Math.min(open, close) * 0.998, close]);
  }
  // 이동평균선 — 앞쪽은 값이 없어 비운다(0 으로 채우면 그 0 이 지표값처럼 그려진다).
  const period = 20;
  const ma = bars.map((_, i) =>
    i + 1 < period ? null : bars.slice(i + 1 - period, i + 1).reduce((sum, b) => sum + b[4], 0) / period,
  );
  return {
    interval,
    bars,
    levels: {
      entry_target: price * 1.006,
      exit_target: price * 1.03,
      stop_loss: price * 0.98,
      trail_stop: null,
      trail_activation: null,
      armed: false,
      reference: null,
      band_upper: null,
      band_lower: null,
      next_buy_ms: null,
      indicator: null,
      pending: false,
    },
    series: [{ id: "ma_filter", label: `이동평균 ${period}`, values: ma }],
    oscillator: withRsi
      ? {
          label: "RSI 14",
          values: bars.map((_, i) => (i < 14 ? null : 42 + Math.sin(i / 7) * 22)),
          entry: 30,
          exit: 70,
        }
      : null,
  };
}

export function beginRun(emit: Emit, r: MockRun, at: number) {
  const m = r.macro;
  const price = START_PRICE[m.symbol] ?? (m.quote === "USDT" ? 100 : 10_000);
  r.tick = 0;
  r.run = {
    startedAt: at,
    sessionId: 4800 + Number(r.id.replace(/\D/g, "")),
    online: true,
    authFailed: false,
    rejected: 0,
    // 0.21.0 부터 판단이 이 PC 에서 돌아 앱이 봉을 들고 있다 — 조건 차트가 기본이다.
    // 레버리지 견본은 **서버가 판단하는 세션**으로 남긴다(옛 실행기 · 스위치를 켜기 전에
    // 시작한 세션이 그렇다). 두 화면이 다 살아 있어야 e2e 가 현재가 그래프와 봉 차트를
    // 둘 다 잰다 — 활동이 가장 많은 견본이라 '활동 기록 세 줄' 예산도 거기서 잰다.
    chart: m.leverage > 1 ? null : mockChart(price, m.interval || "15m", /RSI/i.test(m.strategy)),
    offlineSince: 0,
    priceStaleSince: 0,
    closeFailures: 0,
    fundsShort: false,
    updatedAt: at,
    price,
    inPosition: false,
    uncertain: false,
    entryPrice: 0,
    qty: 0,
    unrealizedPct: 0,
    unrealized: 0,
    unrealizedFeeAware: false,
    invested: 0,
    realized: 0,
    // 모의는 선물도 수수료를 알려 줌(Rust MockBroker). 실전·테스트넷 선물은 체결 수수료를 읽지 않아 모름.
    realizedNet: m.market === "spot" || r.mode === "mock" ? 0 : null,
    fees: m.market === "spot" || r.mode === "mock" ? 0 : null,
    dayChangePct: m.exchange === "upbit" || m.exchange === "bithumb" ? 1.24 : null,
    macroOrigin: m.source === "web" ? "웹에서 바로 실행" : m.signature === "signed" ? "원본 파일(서명 확인)" : "",
    entryWait: waitsForEntry(m.strategy) ? mockEntryWait(at, 0) : null,
    note: "",
    finished: null,
  };
  const ex = m.exchange ? EXCHANGE_LABELS[m.exchange] : m.exchangeName;
  const lev = m.leverage > 1 ? ` ${m.leverage}배` : "";
  say(
    emit,
    r,
    at,
    "start",
    `실행을 시작했어요 · ${MODE_LABELS[r.mode]}`,
    `${m.symbol} · ${MARKET_LABELS[m.market]} · ${SIDE_LABELS[m.side]}${lev}`,
    `세션 시작 (id=${r.run.sessionId}) · ${ex} · ${m.symbol} · ${m.market} · ${MODE_LABELS[r.mode]}`,
    "ok",
  );
  if (r.mode !== "mock") {
    const balance = m.quote === "KRW" ? "사용 가능 잔고 1,234,567 KRW" : "사용 가능 잔고 1,234.5 USDT";
    say(emit, r, at, "info", "잔고를 확인했어요", balance, `잔고: ${balance}`);
  }
  const cap = formatMoney(orderCap(r), m.quote).slice(1);
  say(
    emit,
    r,
    at,
    "info",
    "리스크 안전망을 켰어요",
    `주문 상한 ${cap} · 하루 최대 손실 5%(수수료 포함) · 최대 보유 24시간`,
    `공통 리스크: 하루 최대 손실 5% · 최대 보유 24시간 · 손절 뒤 재진입 대기 30.0분 · 주문 상한 ${cap} · 5초마다 평가`,
  );
}

export function tickRun(emit: Emit, r: MockRun, at: number) {
  const run = r.run;
  if (!run || r.phase !== "running") return;
  const m = r.macro;
  const q = m.quote;
  r.tick += 1;
  // 슬롯마다 박자를 어긋나게 해서 두 매크로가 같은 순간에 같은 일을 겪지 않게 함.
  const t = r.tick + Number(r.id.replace(/\D/g, "")) * 4;
  const wasOnline = run.online;
  const before = run.price;
  run.price = round(run.price * (1 + (Math.random() - 0.48) * 0.0024), q);
  if (r.holdPrice) run.price = round(r.holdPrice * (1 + (Math.random() - 0.5) * 0.0006), q);
  if (run.dayChangePct !== null) run.dayChangePct += ((run.price - before) / before) * 100;
  if (r.holdPrice) {
    run.updatedAt = at;
    refreshPnl(r);
    return;
  }
  if (r.fixedWait) {
    // ?demo=wait — 신호 · 매수 없이 고정한 진입 대기 값만 보임.
    // 원화 반올림(round)을 쓰면 26.2 가 26 으로 굳어 그래프가 안 그려짐 — 소수 셋째 자리까지만.
    const base = Number(r.fixedWait.meter?.now ?? run.price);
    run.price = Math.round(base * (1 + (Math.random() - 0.5) * 0.004) * 1000) / 1000;
    Object.assign(run, { online: true, updatedAt: at, offlineSince: 0, entryWait: r.fixedWait });
    return;
  }
  run.online = !r.offline && t % 13 !== 9;
  run.updatedAt = at;
  run.offlineSince = run.online ? 0 : run.offlineSince || at;
  if (!run.online && wasOnline) {
    say(
      emit,
      r,
      at,
      "connection",
      "서버 연결을 다시 시도하고 있어요",
      "그동안 새 진입은 멈추고 손절만 지켜요",
      "서버 연결 재시도 중 — 신호 대기(진입 없음, 손절만 로컬에서 봅니다)",
      "warn",
    );
  } else if (run.online && !wasOnline) {
    say(emit, r, at, "connection", "서버 연결이 돌아왔어요", "", "서버 연결 복구 · heartbeat ok", "ok");
  }

  const p = plain(run.price, q);
  // 진입 대기 — 서버가 하트비트로 보내는 entry_wait 흉내(RSI 매크로 · 1분 봉). 산 뒤에는 서버가 보내지 않음.
  run.entryWait = run.inPosition || !waitsForEntry(m.strategy) ? null : mockEntryWait(at, r.tick);
  if (run.online && !run.inPosition && t % 7 === 2) {
    const qty = Math.floor((orderCap(r) / run.price) * 100000) / 100000;
    const sideWord = m.side === "long" ? "매수" : "매도";
    say(
      emit,
      r,
      at,
      "signal",
      `${sideWord} 신호 · RSI 30 이탈 반등`,
      `현재가 ${p} ${q}에서 신호를 받았어요`,
      `[신호] RSI 30 이탈 반등 → ${m.side === "long" ? "BUY" : "SELL"} ${qty} ${m.symbol} @ ${run.price}`,
      "muted",
    );
    run.inPosition = true;
    run.entryPrice = run.price;
    run.qty = qty;
    const verb = m.side === "long" ? "샀어요" : "공매도했어요";
    say(
      emit,
      r,
      at,
      "buy",
      `${m.baseAsset} ${formatQty(qty)}개를 ${p} ${q}에 ${verb}`,
      [
        `신호가 대비 0.00%`,
        payEntryFee(r, run, qty * run.price),
        r.mode === "mock" ? "모의 체결 · 실제 주문은 보내지 않았어요" : "",
      ]
        .filter(Boolean)
        .join(" · "),
      `${m.side === "long" ? "BUY" : "SELL"} 체결: id=mock-ggp-${(at % 1e8).toString(16)} 수량=${qty}`,
      "ok",
    );
  } else if (run.inPosition) {
    const dir = m.side === "long" ? 1 : -1;
    const move = ((run.price - run.entryPrice) / run.entryPrice) * 100 * dir;
    const sl = m.exits.sl?.pct ?? 0;
    const sellTitle = `${m.baseAsset} ${formatQty(run.qty)}개를 ${p} ${q}에 ${m.side === "long" ? "팔았어요" : "되사서 정리했어요"}`;
    const close = (rawHead: string) => {
      const { detail, tone } = settle(r, run);
      say(
        emit,
        r,
        at,
        "sell",
        sellTitle,
        detail,
        `  ✓ ${m.side === "long" ? "SELL" : "BUY"} 체결: id=mock-ggp-${(at % 1e8).toString(16)} 수량=${run.qty} · ${rawHead}`,
        tone,
      );
      run.inPosition = false;
      run.entryPrice = 0;
      run.qty = 0;
    };
    if (sl > 0 && move <= -sl) {
      // Rust run_loop.rs와 같은 문구 — 손절은 평단 대비 가격 기준.
      const moved = `${move < 0 ? "-" : "+"}${Math.abs(move).toFixed(2)}%`;
      say(
        emit,
        r,
        at,
        "stop_loss",
        "손절 기준에 닿아 정리해요",
        `평단 대비 가격 ${moved}(손절 기준 -${sl}%) · 현재가 ${p} ${q} · 평단 ${plain(run.entryPrice, q)} ${q}`,
        `[손절] ${run.price} (평단 ${run.entryPrice} · 평단 대비 가격 ${moved})`,
        "warn",
      );
      close("손절");
    } else if (run.online && t % 7 === 0) {
      say(
        emit,
        r,
        at,
        "signal",
        "청산 신호 · RSI 70 위에서 꺾임",
        `현재가 ${p} ${q}`,
        `[신호] RSI 70 하향 이탈 → 전량 청산 @ ${run.price}`,
      );
      close("신호 청산");
    }
  } else if (run.online && t % 9 === 5) {
    say(
      emit,
      r,
      at,
      "blocked",
      "진입을 미뤘어요",
      `직전 청산 뒤 재진입 대기 · ${(30 - (t % 30)).toFixed(1)}분 남음`,
      "  ⏸ 진입 보류: 재진입금지 남은 시간",
      "warn",
    );
  } else if (run.online && t % 23 === 19) {
    say(
      emit,
      r,
      at,
      "error",
      "주문이 거절됐어요",
      `최소 주문 금액보다 작아요 · 다음 신호를 기다려요`,
      "주문 실패: -4164 Order's notional must be no smaller than 5",
      "danger",
    );
  }

  if (t % 3 === 0)
    emit(
      r,
      { kind: "info", title: `heartbeat ok · price=${run.price} · ${run.inPosition ? "보유" : "무포지션"}`, dev: true },
      at,
    );
  refreshPnl(r);
}

export function stopActivities(emit: Emit, r: MockRun, at: number, closeFirst: boolean) {
  const run = r.run;
  if (!run) return;
  if (closeFirst && run.inPosition) {
    const m = r.macro;
    const { detail, tone } = settle(r, run);
    say(
      emit,
      r,
      at,
      "sell",
      `${m.baseAsset} ${formatQty(run.qty)}개를 ${plain(run.price, m.quote)} ${m.quote}에 ${m.side === "long" ? "팔았어요" : "되사서 정리했어요"}`,
      detail,
      `[청산] 시장가 ${run.qty} ${m.symbol} @ ${run.price}`,
      tone,
    );
    run.inPosition = false;
    run.qty = 0;
    run.entryPrice = 0;
    refreshPnl(r);
  }
  const note = closeFirst ? "청산 완료 후 종료" : "매크로만 종료";
  say(
    emit,
    r,
    at,
    "stop",
    `실행을 끝냈어요 · ${note}`,
    closeFirst || !run.inPosition ? "" : "열린 포지션은 그대로예요. 거래소에서 직접 관리하세요.",
    `종료됨 (stopped) · ${note}`,
  );
  run.finished = { status: "stopped", note, at };
}

export function activityText(items: Activity[], clock: (at: number) => string): string {
  return items
    .map((a) =>
      a.dev
        ? `${clock(a.at)}  · ${a.title}`
        : `${clock(a.at)}  ${a.title}${a.detail ? ` — ${a.detail}` : ""}${a.raw ? `\n          ${a.raw}` : ""}`,
    )
    .join("\n");
}
