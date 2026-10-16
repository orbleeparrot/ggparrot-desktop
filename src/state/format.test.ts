import { describe, expect, it } from "vitest";
import {
  formatClock,
  formatClockMillis,
  formatClockSeconds,
  formatAmount,
  formatElapsed,
  formatFee,
  formatWhen,
  formatMoney,
  formatPercent,
  formatPrice,
  formatQty,
  formatSigned,
  formatDuration,
  roundAway,
  toneOf,
} from "./format";

describe("formatMoney", () => {
  it("KRW 는 정수 · 천 단위 쉼표 · 부호", () => {
    expect(formatMoney(1234567.8, "KRW")).toBe("+1,234,568 KRW");
    expect(formatMoney(-15000, "KRW")).toBe("-15,000 KRW");
  });
  it("USDT 는 소수 둘째 자리 · 부호", () => {
    expect(formatMoney(12.345, "USDT")).toBe("+12.35 USDT");
    expect(formatMoney(-0.5, "USDT")).toBe("-0.50 USDT");
    expect(formatMoney(1234.5, "USDT")).toBe("+1,234.50 USDT");
  });
  it("반올림해 0 이면 부호 없이 0 — -0.004 USDT 는 0.00 USDT(Rust money 와 같음)", () => {
    expect(formatMoney(-0.004, "USDT")).toBe("0.00 USDT");
    expect(formatMoney(0.004, "USDT")).toBe("0.00 USDT");
    expect(formatMoney(-0.4, "KRW")).toBe("0 KRW");
    expect(formatMoney(Number.NaN, "USDT")).toBe("0.00 USDT");
    expect(formatMoney(-0.005, "USDT")).toBe("-0.01 USDT");
    expect(toneOf(-0.004)).toBe("flat");
    expect(toneOf(-0.005)).toBe("down");
  });
  it("0 에서 먼 쪽으로 반올림 — Rust money.rs 와 같은 숫자 · 같은 결과", () => {
    expect(formatMoney(1002.5, "KRW")).toBe("+1,003 KRW");
    expect(formatMoney(500 + 1_005_000 * 0.0005, "KRW")).toBe("+1,003 KRW");
    expect(formatMoney(3997.5, "KRW")).toBe("+3,998 KRW");
    expect(formatMoney(-3997.5, "KRW")).toBe("-3,998 KRW");
    expect(formatMoney(1.005, "USDT")).toBe("+1.01 USDT");
    expect(formatMoney(-0.125, "USDT")).toBe("-0.13 USDT");
    expect(roundAway(-0.125, 2)).toBe(-0.13);
    expect(roundAway(-0.004, 2)).toBe(0);
  });
});

describe("formatFee", () => {
  it("부호 없이 돈의 자릿수 — 1,002.5원은 1,003 KRW, 0.1 USDT는 0.10 USDT", () => {
    expect(formatFee(1002.5, "KRW")).toBe("1,003 KRW");
    expect(formatFee(0.1, "USDT")).toBe("0.10 USDT");
    expect(formatFee(0.105, "USDT")).toBe("0.11 USDT");
  });
});

describe("formatPercent · formatSigned", () => {
  it("소수 둘째 자리와 부호", () => {
    expect(formatPercent(0.4249)).toBe("+0.42%");
    expect(formatPercent(-3.1)).toBe("-3.10%");
    expect(formatPercent(0)).toBe("0.00%");
    expect(formatSigned(-0.004, 2)).toBe("0.00");
  });
});

describe("formatPrice · formatQty", () => {
  it("KRW 시세는 100원 이상 정수(반올림), 그 아래 유효숫자 4자리", () => {
    expect(formatPrice(94350123.6, "KRW")).toBe("94,350,124");
    expect(formatPrice(3120.6, "KRW")).toBe("3,121");
    expect(formatPrice(3120.4, "KRW")).toBe("3,120");
    expect(formatPrice(3120.5, "KRW")).toBe("3,121");
    expect(formatPrice(0.0234, "KRW")).toBe("0.0234");
    expect(formatPrice(12.345, "KRW")).toBe("12.35");
    expect(formatPrice(99.99, "KRW")).toBe("99.99");
    expect(formatPrice(5, "KRW")).toBe("5");
    expect(formatPrice(0.000123456, "KRW")).toBe("0.0001235");
  });
  it("USDT 시세는 크기에 맞춘 소수", () => {
    expect(formatPrice(64210.5, "USDT")).toBe("64,210.50");
    expect(formatPrice(1.23456, "USDT")).toBe("1.2346");
    expect(formatPrice(0.000123456, "USDT")).toBe("0.00012346");
  });
  it("수량은 군더더기 0 없이", () => {
    expect(formatQty(0.0046)).toBe("0.0046");
    expect(formatQty(2)).toBe("2");
  });
  it("수량은 줄이지 않고 받은 자릿수 그대로 — 끝의 0만 지우고 정수부에 쉼표", () => {
    expect(formatQty(499.75012494)).toBe("499.75012494");
    expect(formatQty(0.00127)).toBe("0.00127");
    expect(formatQty(0.000000123456789)).toBe("0.000000123456789");
    expect(formatQty(1234.5)).toBe("1,234.5");
    expect(formatQty(0.1 + 0.2)).toBe("0.3");
    expect(formatQty(0)).toBe("0");
    expect(formatQty(Number.NaN)).toBe("0");
  });
});

describe("formatElapsed", () => {
  it("분:초 · 시:분:초 · 일", () => {
    expect(formatElapsed(0)).toBe("00:00");
    expect(formatElapsed(65_000)).toBe("01:05");
    expect(formatElapsed(3_725_000)).toBe("1:02:05");
    expect(formatElapsed(2 * 86_400_000 + 3_661_000)).toBe("2일 01:01:01");
    expect(formatElapsed(-5000)).toBe("00:00");
  });
});

describe("toneOf", () => {
  it("보이는 자릿수에서 0 이면 flat", () => {
    expect(toneOf(-0.0016)).toBe("flat");
    expect(toneOf(0.01)).toBe("up");
    expect(toneOf(-0.4, 0)).toBe("flat");
    expect(toneOf(-2, 0)).toBe("down");
  });
});

describe("formatClock · formatWhen (현지 시간)", () => {
  it("오후 3:12 처럼", () => {
    expect(formatClock(new Date(2026, 9, 15, 15, 12, 40).getTime())).toBe("오후 3:12");
    expect(formatClock(new Date(2026, 9, 15, 9, 5).getTime())).toBe("오전 9:05");
  });
  it("활동 기록은 초까지, 꼬리표는 밀리초까지", () => {
    const at = new Date(2026, 9, 15, 22, 11, 5, 123).getTime();
    expect(formatClockSeconds(at)).toBe("오후 10:11:05");
    expect(formatClockMillis(at)).toBe("오후 10:11:05.123");
    expect(formatClockSeconds(new Date(2026, 9, 15, 9, 5, 7).getTime())).toBe("오전 9:05:07");
  });
  it("오늘이면 '오늘', 아니면 날짜", () => {
    const now = new Date(2026, 9, 15, 18, 0).getTime();
    expect(formatWhen(new Date(2026, 9, 15, 15, 12).getTime(), now)).toBe("오늘 오후 3:12");
    expect(formatWhen(new Date(2026, 9, 14, 9, 5).getTime(), now)).toBe("10월 14일 오전 9:05");
  });
});

describe("formatAmount", () => {
  it("부호 없이, 원화는 정수 · USDT 는 군더더기 0 없이", () => {
    expect(formatAmount(1000, "USDT")).toBe("1,000 USDT");
    expect(formatAmount(250.5, "USDT")).toBe("250.5 USDT");
    expect(formatAmount(150000.4, "KRW")).toBe("150,000 KRW");
  });
});

describe("formatDuration", () => {
  it("걸린 시간 — 초·분·시간·일, 아래 단위는 버림", () => {
    expect(formatDuration(0)).toBe("");
    expect(formatDuration(800)).toBe("");
    expect(formatDuration(45_000)).toBe("45초");
    expect(formatDuration(12 * 60_000 + 30_000)).toBe("12분");
    expect(formatDuration(2 * 3_600_000)).toBe("2시간");
    expect(formatDuration(80 * 60_000)).toBe("1시간 20분");
    expect(formatDuration(51 * 3_600_000 + 59 * 60_000)).toBe("2일 3시간");
    expect(formatDuration(3 * 86_400_000)).toBe("3일");
    expect(formatDuration(-5_000)).toBe("");
  });
});
