import { act, cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { FILLED_MASK, SECRET_DEBOUNCE_MS, SecretField } from "./SecretField";

beforeEach(() => vi.useFakeTimers());
afterEach(() => {
  cleanup();
  vi.useRealTimers();
});

describe("SecretField", () => {
  it("찬 칸은 점을 보이고 값은 없다 · 입력은 디바운스해 보낸다", () => {
    const onCommit = vi.fn();
    render(<SecretField id="f" label="API Key" filled onCommit={onCommit} onReveal={async () => "x"} />);
    const input = screen.getByLabelText("API Key") as HTMLInputElement;
    expect(input.value).toBe("");
    expect(input.placeholder).toBe(FILLED_MASK);
    fireEvent.change(input, { target: { value: "ab" } });
    fireEvent.change(input, { target: { value: "abc" } });
    expect(onCommit).not.toHaveBeenCalled();
    act(() => {
      vi.advanceTimersByTime(SECRET_DEBOUNCE_MS);
    });
    expect(onCommit).toHaveBeenCalledTimes(1);
    expect(onCommit).toHaveBeenCalledWith("abc");
  });

  it("눈을 누르면 reveal 로 한 번 받아 보이고, 다시 가리면 지운다", async () => {
    const onReveal = vi.fn(async () => "secret-value");
    render(<SecretField id="f" label="Secret Key" filled onCommit={vi.fn()} onReveal={onReveal} />);
    const input = screen.getByLabelText("Secret Key") as HTMLInputElement;
    await act(async () => fireEvent.click(screen.getByRole("button", { name: "Secret Key 보기" })));
    expect(onReveal).toHaveBeenCalledTimes(1);
    expect(input.type).toBe("text");
    expect(input.value).toBe("secret-value");
    await act(async () => fireEvent.click(screen.getByRole("button", { name: "Secret Key 가리기" })));
    expect(input.type).toBe("password");
    expect(input.value).toBe("");
    expect(input.placeholder).toBe(FILLED_MASK);
  });
});
