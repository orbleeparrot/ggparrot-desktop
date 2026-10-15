// mock 실행 루프 — 현재가를 흔들고, 신호·체결·보류·연결 문제를 사람이 읽는 활동과 원문 줄(dev)로 냄.
import { EXCHANGE_LABELS, type MockRun } from "./mockData";
import { formatMoney, formatPrice, formatQty } from "./state/format";

const PLAIN = new Intl.NumberFormat("en-US", { maximumFractionDigits: 2 });
/** 문장 안의 가격 — 군더더기 0 없이("81,806"·"81,831.58"). */
const plain = (price: number, quote: "USDT" | "KRW") =>
  quote === "KRW" ? formatPrice(price, quote) : PLAIN.format(price);
import { MARKET_LABELS, MODE_LABELS, SIDE_LABELS } from "./state/runs";
import type { Activity, ActivityKind, Tone } from "./types";

export type Emit = (
  r: MockRun,
  a: { kind: ActivityKind; title: string; detail?: string; raw?: string; dev?: boolean; tone?: Tone },
  at: number,
) => void;

/** 모의 수수료율 — 현물만 셈(선물은 실전에서도 수수료를 읽지 않음). */
const MOCK_FEE = 0.0005;

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

/** 모의 체결 수수료를 더하고 활동 문구("수수료 0.04 USDT")를 돌려줌. 모르는 시장(선물)이면 "". */
function payFee(run: NonNullable<MockRun["run"]>, notional: number, q: "USDT" | "KRW"): string {
  if (run.fees === null) return "";
  const fee = notional * MOCK_FEE;
  run.fees += fee;
  return `수수료 ${formatPrice(fee, q)} ${q}`;
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
    offlineSince: 0,
    priceStaleSince: 0,
    closeFailures: 0,
    updatedAt: at,
    price,
    inPosition: false,
    uncertain: false,
    entryPrice: 0,
    qty: 0,
    unrealizedPct: 0,
    unrealized: 0,
    realized: 0,
    fees: m.market === "spot" ? 0 : null,
    dayChangePct: m.exchange === "upbit" || m.exchange === "bithumb" ? 1.24 : null,
    macroOrigin: m.source === "web" ? "웹에서 바로 실행" : m.signature === "signed" ? "원본 파일(서명 확인)" : "",
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
    `주문 상한 ${cap} · 일일 최대 손실 5% · 최대 보유 24시간`,
    `공통 리스크: 일일최대손실 5% · 최대보유 24h · 재진입금지 30.0분 · 주문 상한 ${cap} · 5초마다 평가`,
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
  if (run.dayChangePct !== null) run.dayChangePct += ((run.price - before) / before) * 100;
  run.online = t % 13 !== 9;
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
      [`신호가 대비 +0.00%`, payFee(run, qty * run.price, q), "모의 체결 · 실제 주문은 보내지 않았어요"]
        .filter(Boolean)
        .join(" · "),
      `${m.side === "long" ? "BUY" : "SELL"} 체결: id=mock-ggp-${(at % 1e8).toString(16)} 수량=${qty}`,
      "ok",
    );
  } else if (run.inPosition) {
    const dir = m.side === "long" ? 1 : -1;
    const pct = ((run.price - run.entryPrice) / run.entryPrice) * 100 * dir;
    const pnl = (run.price - run.entryPrice) * run.qty * dir;
    const close = (kind: "sell" | "stop_loss", title: string, rawHead: string) => {
      const fee = payFee(run, run.qty * run.price, q);
      const detail = [`손익 ${formatMoney(pnl, q)} (${pct >= 0 ? "+" : ""}${pct.toFixed(2)}%)`, fee]
        .filter(Boolean)
        .join(" · ");
      say(
        emit,
        r,
        at,
        kind,
        title,
        detail,
        `${rawHead} ${run.qty} ${m.symbol} @ ${run.price} · 손익 ${pnl.toFixed(q === "KRW" ? 0 : 4)}`,
        pnl >= 0 ? "ok" : "danger",
      );
      run.realized += pnl;
      run.inPosition = false;
      run.entryPrice = 0;
      run.qty = 0;
    };
    if (pct <= -0.6) {
      close("stop_loss", "손절 기준에 닿아 정리했어요", "[손절] 시장가 청산");
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
      close(
        "sell",
        `${m.baseAsset} ${formatQty(run.qty)}개를 ${p} ${q}에 ${m.side === "long" ? "팔았어요" : "되사서 정리했어요"}`,
        "[청산] 시장가",
      );
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
  const dir = m.side === "long" ? 1 : -1;
  run.unrealizedPct = run.inPosition ? ((run.price - run.entryPrice) / run.entryPrice) * 100 * dir : 0;
  run.unrealized = run.inPosition ? (run.price - run.entryPrice) * run.qty * dir : 0;
}

export function stopActivities(emit: Emit, r: MockRun, at: number, closeFirst: boolean) {
  const run = r.run;
  if (!run) return;
  if (closeFirst && run.inPosition) {
    const m = r.macro;
    const pnl = run.unrealized;
    say(
      emit,
      r,
      at,
      "sell",
      `${plain(run.price, m.quote)} ${m.quote}에 ${formatQty(run.qty)} ${m.baseAsset} 정리했어요`,
      [`손익 ${formatMoney(pnl, m.quote)}`, payFee(run, run.qty * run.price, m.quote)].filter(Boolean).join(" · "),
      `[청산] 시장가 ${run.qty} ${m.symbol} @ ${run.price}`,
      pnl >= 0 ? "ok" : "danger",
    );
    run.realized += pnl;
    run.inPosition = false;
    run.unrealized = 0;
    run.unrealizedPct = 0;
    run.qty = 0;
    run.entryPrice = 0;
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
