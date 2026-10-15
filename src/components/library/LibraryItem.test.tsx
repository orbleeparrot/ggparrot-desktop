import { cleanup, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import type { LibraryFile } from "../../types";
import { LibraryItem } from "./LibraryItem";

afterEach(cleanup);

const file: LibraryFile = {
  fileName: "binance_BTCUSDT_rsi_long_20261013.ggm.json",
  valid: true,
  error: "",
  title: "binance_BTCUSDT_rsi_long_20261013",
  exchange: "binance",
  exchangeLabel: "바이낸스",
  symbol: "BTCUSDT",
  side: "long",
  summary: "",
  strategy: "RSI 조건 매매",
  unsupported: "",
  signed: true,
  modifiedAt: 0,
  inUse: 0,
};

describe("매크로 폴더 한 줄", () => {
  it("전략 이름을 보이고 고를 수 있다", () => {
    render(<LibraryItem file={file} busy={false} onPick={vi.fn()} />);
    expect(screen.getByText("RSI 조건 매매")).toBeTruthy();
    expect((screen.getByRole("button") as HTMLButtonElement).disabled).toBe(false);
  });

  it("이 앱이 실행할 수 없는 매크로는 이유와 함께 막는다", () => {
    const why = "이 앱은 아직 이 매크로 유형(정기 분할매수 · 하락 방어 전환)을 지원하지 않아요.";
    render(
      <LibraryItem
        file={{ ...file, strategy: "정기 분할매수(DCA)", unsupported: why }}
        busy={false}
        onPick={vi.fn()}
      />,
    );
    expect((screen.getByRole("button") as HTMLButtonElement).disabled).toBe(true);
    expect(screen.getByText(why)).toBeTruthy();
  });
});
