import { cleanup, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import type { LibraryFile } from "../../types";
import { isProblem, LibraryItem, LibraryProblem } from "./LibraryItem";

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

  it("실행 목록에 있으면 '실행 목록 N개', 서명은 아이콘으로", () => {
    render(<LibraryItem file={{ ...file, inUse: 2 }} busy={false} onPick={vi.fn()} />);
    expect(screen.getByText("실행 목록 2개")).toBeTruthy();
    expect(screen.getByLabelText("서명됨")).toBeTruthy();
  });

  it("이 앱이 실행할 수 없는 매크로 · 읽지 못한 파일은 목록 대신 '이름 · 이유' 한 줄로", () => {
    const why = "이 앱은 아직 이 매크로 유형(정기 분할매수 · 하락 방어 전환)을 지원하지 않아요.";
    const blocked = { ...file, strategy: "정기 분할매수(DCA)", unsupported: why };
    expect(isProblem(blocked)).toBe(true);
    expect(isProblem({ ...file, valid: false })).toBe(true);
    expect(isProblem(file)).toBe(false);
    render(
      <ul>
        <LibraryProblem file={blocked} />
        <LibraryProblem file={{ ...file, title: "", fileName: "깨진.ggm.json", valid: false, error: "" }} />
      </ul>,
    );
    expect(screen.getByText(why)).toBeTruthy();
    expect(screen.getByText("깨진.ggm.json")).toBeTruthy();
    expect(screen.getByText("매크로 파일로 읽지 못했어요.")).toBeTruthy();
  });
});
