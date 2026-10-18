import { describe, expect, it } from "vitest";
import { binanceMacro, upbitMacro } from "../test/fixtures";
import {
  canStop,
  coinName,
  coinTicker,
  futuresTag,
  isIdle,
  primaryAction,
  runningModeText,
  settingsElementId,
  settingsTargetOf,
  statusParts,
} from "./runs";

describe("막힌 이유 → 설정의 칸", () => {
  it("키가 필요하면 그 키 칸으로", () => {
    const t = settingsTargetOf({ kind: "key", slot: "binance_testnet" });
    expect(t).toEqual({ kind: "slot", slot: "binance_testnet" });
    expect(settingsElementId(t!)).toBe("settings-slot-binance_testnet");
  });
  it("회원 키면 껄무새 계정으로", () => {
    const t = settingsTargetOf({ kind: "member" });
    expect(settingsElementId(t!)).toBe("settings-member");
  });
  it("필요한 칸이 없으면(같은 종목 실행 중 등) 버튼이 없다", () => {
    expect(settingsTargetOf(null)).toBeNull();
  });
  it("묶음으로도 갈 수 있다", () => {
    expect(settingsElementId({ kind: "section", section: "about" })).toBe("settings-about");
  });
});

describe("단계별 버튼", () => {
  it("대기 · 끝남은 시작, 시작 중 · 실행 중은 종료, 확인 · 종료 중은 꺼진 버튼", () => {
    expect(primaryAction("idle")).toEqual({ kind: "start" });
    expect(primaryAction("finished")).toEqual({ kind: "start" });
    expect(primaryAction("running")).toEqual({ kind: "stop" });
    expect(primaryAction("starting")).toEqual({ kind: "stop" });
    expect(primaryAction("checking")).toEqual({ kind: "busy", label: "연결 확인 중…" });
    expect(primaryAction("stopping")).toEqual({ kind: "busy", label: "종료 처리 중…" });
  });
  it("모드는 대기 · 끝남에서만 바꾼다", () => {
    expect(isIdle("finished")).toBe(true);
    expect(isIdle("checking")).toBe(false);
    expect(canStop("stopping")).toBe(false);
  });
});

describe("상태 문구 나누기", () => {
  it("첫 ' · ' 앞뒤로 나눔 — 카드가 좁으면 뒷부분을 숨김", () => {
    expect(statusParts("실행 중 · 서버 재시도 중")).toEqual(["실행 중", "서버 재시도 중"]);
    expect(statusParts("종료됨 · 손절 · 수동")).toEqual(["종료됨", "손절 · 수동"]);
    expect(statusParts("시작 전")).toEqual(["시작 전", ""]);
  });
});

describe("실행 중 모드 배지", () => {
  it("받침에 맞춘 조사와 단계별 문구", () => {
    expect(runningModeText("mock", "running")).toBe("모의로 실행 중");
    expect(runningModeText("testnet", "starting")).toBe("테스트넷으로 시작하는 중");
    expect(runningModeText("live", "checking")).toBe("실전으로 시작 전 확인 중");
    expect(runningModeText("live", "stopping")).toBe("실전으로 실행 중");
  });
});

describe("종목 글자 — 기초 자산만, 시장 쌍은 보이지 않음", () => {
  it("KRW-B3 · BTCUSDT · ETH_KRW → B3 · BTC · ETH", () => {
    expect(coinTicker({ baseAsset: "B3", symbol: "KRW-B3" })).toBe("B3");
    expect(coinTicker({ baseAsset: "btc", symbol: "BTCUSDT" })).toBe("BTC");
    expect(coinTicker({ baseAsset: "ETH", symbol: "ETH_KRW" })).toBe("ETH");
  });
  it("기초 자산을 모르면 시장 쌍 그대로(빈 칸으로 두지 않음)", () => {
    expect(coinTicker({ baseAsset: "", symbol: "weird" })).toBe("WEIRD");
  });
  it("바이낸스 선물만 '선물 N배' — 1배 · 모름은 '선물', 현물은 없음", () => {
    expect(futuresTag({ market: "futures", leverage: 3 })).toBe("선물 3배");
    expect(futuresTag({ market: "futures", leverage: 1 })).toBe("선물");
    expect(futuresTag({ market: "futures", leverage: 0 })).toBe("선물");
    expect(futuresTag({ market: "spot", leverage: 1 })).toBe("");
  });
  it("카드 풍선 도움말은 거래소와 종목 — 시장 쌍 없이", () => {
    expect(coinName(binanceMacro)).toBe("바이낸스 BTC 선물 3배");
    expect(coinName(upbitMacro)).toBe("업비트 BTC");
    expect(coinName({ ...upbitMacro, exchangeLabel: "", exchangeName: "kucoin" })).toBe("kucoin BTC");
  });
});
