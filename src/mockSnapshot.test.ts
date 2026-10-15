import { describe, expect, it } from "vitest";
import { marketPageOf } from "./mockSnapshot";
import { binanceMacro, upbitMacro } from "./test/fixtures";
import type { MacroInfo, RunMode } from "./types";

// runner-core market_page.rs의 테스트와 같은 주소 — 미리보기도 앱과 같은 화면을 엶.
const page = (macro: Partial<MacroInfo>, mode: RunMode = "live") =>
  marketPageOf({ macro: { ...binanceMacro, ...macro }, mode })?.url ?? null;

describe("marketPageOf", () => {
  it("국내는 원화 시장 화면", () => {
    expect(page({ ...upbitMacro, symbol: "KRW-BTC", baseAsset: "BTC" })).toBe(
      "https://upbit.com/exchange?code=CRIX.UPBIT.KRW-BTC",
    );
    expect(page({ ...upbitMacro, exchange: "bithumb", symbol: "KRW-XRP", baseAsset: "XRP" })).toBe(
      "https://www.bithumb.com/react/trade/order/XRP-KRW",
    );
  });

  it("바이낸스는 시장·모드에 따라(선물 테스트넷은 데모 사이트)", () => {
    const spot = { exchange: "binance", symbol: "BTCUSDT", baseAsset: "BTC", market: "spot" } as const;
    const fut = { ...spot, market: "futures" } as const;
    expect(page(spot)).toBe("https://www.binance.com/trade/BTC_USDT?type=spot");
    expect(page(spot, "testnet")).toBe("https://www.binance.com/trade/BTC_USDT?type=spot");
    expect(page(fut, "mock")).toBe("https://www.binance.com/futures/BTCUSDT");
    expect(page(fut, "testnet")).toBe("https://demo.binance.com/futures/BTCUSDT");
  });

  it("모르는 모양의 종목은 단추를 띄우지 않음", () => {
    expect(page({ exchange: "binance", symbol: "BTCUSDT?x=1", baseAsset: "BTC" })).toBeNull();
    expect(page({ exchange: "upbit", symbol: "BTC", baseAsset: "BTC" })).toBeNull();
    expect(page({ exchange: null })).toBeNull();
  });
});
