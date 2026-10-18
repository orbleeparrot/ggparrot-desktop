import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it } from "vitest";
import { CoinMark, coinLogoSources, DEFAULT_SERVER_BASE } from "./CoinMark";

afterEach(cleanup);

const SERVER = "https://api.example.com";

function renderMark(base: string, over: Partial<Parameters<typeof CoinMark>[0]> = {}) {
  const props = {
    base,
    exchange: "upbit" as const,
    exchangeLabel: "업비트",
    size: 32,
    badge: 14,
    serverBase: SERVER,
    ...over,
  };
  const view = render(<CoinMark {...props} />);
  const logo = () => view.container.querySelector<HTMLImageElement>(".coin-mark-logo");
  return { ...view, props, logo };
}

describe("코인 표시 — 로고 출처 차례", () => {
  it("바이낸스 CDN(리퍼러 없음) → 서버 사본 → 글자 배지, 깨진 이미지는 보이지 않음", () => {
    const { container, logo } = renderMark("b3");
    expect(logo()?.getAttribute("src")).toBe("https://bin.bnbstatic.com/static/assets/logos/B3.png");
    expect(logo()?.getAttribute("referrerpolicy")).toBe("no-referrer");
    // 옆에 종목 글자가 있어 코인 로고는 장식
    expect(logo()?.getAttribute("alt")).toBe("");

    fireEvent.error(logo()!);
    expect(logo()?.getAttribute("src")).toBe(`${SERVER}/api/coin-logo/B3.png`);

    fireEvent.error(logo()!);
    expect(logo()).toBeNull();
    const letters = container.querySelector(".coin-mark-letters")!;
    expect(letters.textContent).toBe("B3");
    expect(letters.getAttribute("aria-hidden")).toBe("true");
  });

  it("글자 배지는 BASE 앞 세 글자", () => {
    const { container, logo } = renderMark("1000shib");
    fireEvent.error(logo()!);
    fireEvent.error(logo()!);
    expect(container.querySelector(".coin-mark-letters")?.textContent).toBe("100");
  });

  it("같은 자리에서 종목이 바뀌면 앞 종목의 실패를 물려받지 않고 CDN부터", () => {
    const { rerender, props, logo, container } = renderMark("B3");
    fireEvent.error(logo()!);
    fireEvent.error(logo()!);
    expect(container.querySelector(".coin-mark-letters")).not.toBeNull();

    rerender(<CoinMark {...props} base="BTC" />);
    expect(logo()?.getAttribute("src")).toBe("https://bin.bnbstatic.com/static/assets/logos/BTC.png");
    expect(container.querySelector(".coin-mark-letters")).toBeNull();
  });

  it("서버 주소가 비면 기본 서버, 끝의 / 는 뗌", () => {
    expect(coinLogoSources("BTC", "")[1]).toBe(`${DEFAULT_SERVER_BASE}/api/coin-logo/BTC.png`);
    expect(coinLogoSources("BTC", "http://127.0.0.1:8000/")[1]).toBe("http://127.0.0.1:8000/api/coin-logo/BTC.png");
  });
});

describe("코인 표시 — 거래소 작은 로고", () => {
  it("거래소 이름으로 읽힘", () => {
    renderMark("BTC");
    const ex = screen.getByRole("img", { name: "업비트" });
    expect(ex.className).toContain("coin-mark-ex");
    // 읽히는 그림은 거래소 로고 하나뿐(코인 로고는 장식)
    expect(screen.getAllByRole("img")).toHaveLength(1);
  });

  it("거래소를 모르면 작은 로고가 없음", () => {
    const { container } = renderMark("BTC", { exchange: null, exchangeLabel: "kucoin" });
    expect(container.querySelector(".coin-mark-ex")).toBeNull();
    expect(screen.queryAllByRole("img")).toHaveLength(0);
  });

  it("거래소 이름이 옆 글자에 이미 있으면(exchangeLabel '') 작은 로고도 장식", () => {
    const { container } = renderMark("BTC", { exchangeLabel: "" });
    const ex = container.querySelector(".coin-mark-ex")!;
    expect(ex.getAttribute("alt")).toBe("");
    expect(ex.getAttribute("aria-hidden")).toBe("true");
  });
});
