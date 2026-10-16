import { cleanup, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it } from "vitest";
import { binanceLiveRestrict, binanceLiveRestrictFutures } from "./binanceCard";
import { ExchangeScreen } from "./index";
import { stillFrame } from "./ScenePlayer";
import type { Scene } from "./types";

afterEach(cleanup);

function drawStill(scene: Scene) {
  render(<svg>{scene.draw(stillFrame(scene, "203.0.113.24"))}</svg>);
}

describe("바이낸스 실거래 허용 IP·주문 권한 그림", () => {
  it("현물은 Enable Spot & Margin & Stock Trading, 선물은 Enable Futures 칸을 누름", () => {
    drawStill(binanceLiveRestrict);
    expect(screen.queryByText("Enable Futures")).toBeNull();
    cleanup();
    drawStill(binanceLiveRestrictFutures);
    expect(screen.getByText("Enable Futures")).toBeTruthy();
    expect(binanceLiveRestrictFutures.targets.trade).not.toEqual(binanceLiveRestrict.targets.trade);
  });

  it("'Restrict access…'를 고르면 IP 입력 칸에 이 기기 주소를 넣고 Confirm", () => {
    drawStill(binanceLiveRestrictFutures);
    expect(screen.getByText("203.0.113.24")).toBeTruthy();
    expect(screen.getByText("Confirm")).toBeTruthy();
    const order = binanceLiveRestrictFutures.steps.flatMap((s) => (s.t === "move" ? [s.to] : []));
    expect(order).toEqual(["edit", "restrict", "ipin", "confirm", "trade", "save"]);
  });

  it("도우미는 시장을 넘기면 선물 그림을 고름(다른 사이트·단계는 그대로)", () => {
    const { container } = render(
      <ExchangeScreen site="binance:live" kind="restrict" ip="" label="바이낸스" market="futures" />,
    );
    expect(container.textContent).toContain("Enable Futures");
    cleanup();
    const spot = render(<ExchangeScreen site="binance:live" kind="restrict" ip="" label="바이낸스" />);
    expect(spot.container.textContent).not.toContain("Enable Futures");
  });
});
