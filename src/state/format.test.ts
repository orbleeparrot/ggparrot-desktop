import { describe, expect, it } from "vitest";
import {
  formatClock,
  formatAmount,
  formatElapsed,
  formatWhen,
  formatMoney,
  formatPercent,
  formatPrice,
  formatQty,
  formatSigned,
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
  it("반올림해 0 이면 -0 이 아니라 +0", () => {
    expect(formatMoney(-0.001, "USDT")).toBe("+0.00 USDT");
    expect(formatMoney(-0.4, "KRW")).toBe("+0 KRW");
    expect(formatMoney(Number.NaN, "USDT")).toBe("+0.00 USDT");
  });
});

describe("formatPercent · formatSigned", () => {
  it("소수 둘째 자리와 부호", () => {
    expect(formatPercent(0.4249)).toBe("+0.42%");
    expect(formatPercent(-3.1)).toBe("-3.10%");
    expect(formatPercent(0)).toBe("+0.00%");
    expect(formatSigned(-0.004, 2)).toBe("+0.00");
  });
});

describe("formatPrice · formatQty", () => {
  it("KRW 시세는 정수", () => {
    expect(formatPrice(94350123.6, "KRW")).toBe("94,350,124");
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
