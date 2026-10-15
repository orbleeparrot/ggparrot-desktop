import { act, cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { SCENES } from ".";
import { ScenePlayer } from "./ScenePlayer";

beforeEach(() => vi.useFakeTimers());
afterEach(() => {
  cleanup();
  vi.useRealTimers();
});

describe("ScenePlayer", () => {
  it("거래소 그림에 [예시 화면] 표시와 설명을 붙인다", () => {
    render(<ScenePlayer scene={SCENES.upbit.perm} ip="1.2.3.4" label="업비트" />);
    expect(screen.getByText("예시 화면")).toBeTruthy();
    expect(screen.getByText("업비트 사이트에서 할 일을 그림으로 보여 줘요")).toBeTruthy();
  });

  it("멈추면 커서가 사라지고 완성된 화면을 보이며, 다시 재생할 수 있다", () => {
    const { container } = render(<ScenePlayer scene={SCENES.upbit.ip} ip="1.2.3.4" label="업비트" />);
    act(() => void vi.advanceTimersByTime(200));
    expect(container.querySelector(".ex-cursor")).not.toBeNull();

    const toggle = screen.getByRole("button", { name: /멈춤/ });
    fireEvent.click(toggle);
    expect(container.querySelector(".ex-cursor")).toBeNull();
    expect(container.textContent).toContain("1.2.3.4");

    fireEvent.click(screen.getByRole("button", { name: /재생/ }));
    expect(container.querySelector(".ex-cursor")).not.toBeNull();
    expect(container.textContent).not.toContain("1.2.3.4");
  });
});
