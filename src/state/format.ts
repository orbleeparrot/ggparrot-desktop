// 숫자·돈·시간 표시. 돈: KRW는 정수, 그 밖(USDT)은 소수 둘째 자리, 늘 부호를 붙임.
import type { MacroInfo } from "../types";

type Quote = MacroInfo["quote"];

const nf = (min: number, max: number) =>
  new Intl.NumberFormat("en-US", { minimumFractionDigits: min, maximumFractionDigits: max });
const INT = nf(0, 0);
const TWO = nf(2, 2);

function clean(n: number): number {
  return Number.isFinite(n) ? n : 0;
}

/** "+1,234 KRW"·"-12.30 USDT". 반올림해서 0이 되면 "+0"으로 씀(“-0.00”을 보이지 않음). */
export function formatMoney(amount: number, quote: Quote): string {
  return `${formatSigned(amount, quote === "KRW" ? 0 : 2)} ${quote}`;
}

export function formatSigned(value: number, digits: number): string {
  const v = clean(value);
  const body = (digits === 0 ? INT : digits === 2 ? TWO : nf(digits, digits)).format(Math.abs(v));
  const zero = Number(body.replace(/,/g, "")) === 0;
  return `${v < 0 && !zero ? "-" : "+"}${body}`;
}

/** 부호 없는 금액 "1,000 USDT"·"500,000 KRW" — 투자 금액처럼 손익이 아닌 금액. */
export function formatAmount(amount: number, quote: Quote): string {
  const v = clean(amount);
  return `${(quote === "KRW" ? INT : nf(0, 2)).format(quote === "KRW" ? Math.round(v) : v)} ${quote}`;
}

/** "+0.42%" */
export function formatPercent(value: number): string {
  return `${formatSigned(value, 2)}%`;
}

/** 시세·평단. KRW는 정수, USDT는 큰 값은 소수 둘, 작은 값은 유효 자릿수를 살림. */
export function formatPrice(price: number, quote: Quote): string {
  const v = clean(price);
  if (quote === "KRW") return INT.format(Math.round(v));
  if (Math.abs(v) >= 1000) return TWO.format(v);
  if (Math.abs(v) >= 1) return nf(2, 4).format(v);
  if (v === 0) return "0";
  return nf(2, 8).format(v);
}

/** 수량: 군더더기 0 없이 최대 8자리. */
export function formatQty(qty: number): string {
  return nf(0, 8).format(clean(qty));
}

/** 경과 시간 "05:12"·"1:05:12"·"2일 03:04:05". */
export function formatElapsed(ms: number): string {
  const total = Math.max(0, Math.floor(clean(ms) / 1000));
  const days = Math.floor(total / 86400);
  const h = Math.floor((total % 86400) / 3600);
  const m = Math.floor((total % 3600) / 60);
  const s = total % 60;
  const pad = (n: number) => String(n).padStart(2, "0");
  if (days > 0) return `${days}일 ${pad(h)}:${pad(m)}:${pad(s)}`;
  if (h > 0) return `${h}:${pad(m)}:${pad(s)}`;
  return `${pad(m)}:${pad(s)}`;
}

/** 지난 시간을 짧게 — "12초 전", "3분 전". */
export function formatAgo(ms: number): string {
  const s = Math.max(0, Math.floor(clean(ms) / 1000));
  if (s < 60) return `${s}초 전`;
  const m = Math.floor(s / 60);
  if (m < 60) return `${m}분 전`;
  return `${Math.floor(m / 60)}시간 전`;
}

/** 값의 부호에 맞는 톤. 보이는 자릿수(digits)로 반올림해 0이면 flat — "+0.00%"가 빨갛게 보이지 않게. */
export function toneOf(value: number, digits = 2): "up" | "down" | "flat" {
  const v = clean(value);
  if (Math.abs(v) < 0.5 * 10 ** -digits) return "flat";
  return v > 0 ? "up" : "down";
}

export function moneyDigits(quote: Quote): number {
  return quote === "KRW" ? 0 : 2;
}

const CLOCK = new Intl.DateTimeFormat("ko-KR", { hour: "numeric", minute: "2-digit" });
const CLOCK_SEC = new Intl.DateTimeFormat("ko-KR", { hour: "numeric", minute: "2-digit", second: "2-digit" });
const DAY = new Intl.DateTimeFormat("ko-KR", { month: "long", day: "numeric" });

/** 활동 기록 시각 "오후 3:12"(현지 시간). */
export function formatClock(at: number): string {
  return CLOCK.format(new Date(at));
}

/** 마우스를 올렸을 때 보이는 초까지 "오후 3:12:05". */
export function formatClockSeconds(at: number): string {
  return CLOCK_SEC.format(new Date(at));
}

/** 파일 수정 시각·업데이트 확인 시각. 오늘이면 "오늘 오후 3:12", 아니면 "10월 14일 오후 3:12". */
export function formatWhen(at: number, now = Date.now()): string {
  const d = new Date(at);
  const today = new Date(now);
  const same =
    d.getFullYear() === today.getFullYear() && d.getMonth() === today.getMonth() && d.getDate() === today.getDate();
  return `${same ? "오늘" : DAY.format(d)} ${CLOCK.format(d)}`;
}

/** 내려받은 크기 — 1 MB 이상은 "6.1 MB", 그 아래는 "512 KB". */
export function formatBytes(n: number): string {
  if (n >= 1024 * 1024) return `${(n / (1024 * 1024)).toFixed(1)} MB`;
  return `${Math.max(0, Math.round(n / 1024))} KB`;
}
