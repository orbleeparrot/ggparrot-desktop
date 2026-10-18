import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import type { Act } from "../../state/context";
import { makeSlot, makeState } from "../../test/fixtures";
import type { EntryPreview } from "../../types";
import { useRunDialogs } from "./useRunDialogs";

vi.mock("../../ipc", () => ({ on: () => Promise.resolve(() => undefined) }));

afterEach(cleanup);

function Harness({ act }: { act: Act }) {
  const flows = useRunDialogs({ state: makeState({ runs: [makeSlot()] }), act, toast: vi.fn(), openSettings: vi.fn() });
  return (
    <>
      <button type="button" onClick={() => flows.startRun("run-1")}>
        시작
      </button>
      {flows.dialogs}
    </>
  );
}

function fakeAct(preview: EntryPreview | Error) {
  return vi.fn(async (cmd: string) => {
    if (cmd === "entry_preview") {
      if (preview instanceof Error) throw preview;
      return preview;
    }
    if (cmd === "start_run") return { kind: "started" };
    return undefined;
  }) as unknown as Act & ReturnType<typeof vi.fn>;
}

const waiting: EntryPreview = {
  waiting: true,
  sentence: "가격이 102.00 위로 올라가면 사요.",
  interval_label: "1시간",
  typical: { episodes: 21, span_ms: 30 * 86_400_000, avg_ms: 9 * 3_600_000, longest_ms: 2 * 86_400_000 },
};

describe("시작 전 진입 대기 안내", () => {
  it("기다리는 매크로면 바로 시작하지 않고 시트로 조건 · 평소 기다린 시간을 보여 준다", async () => {
    const act = fakeAct(waiting);
    render(<Harness act={act} />);
    fireEvent.click(screen.getByText("시작"));
    expect(await screen.findByText("이 매크로는 바로 사지 않아요")).toBeTruthy();
    expect(screen.getByText("가격이 102.00 위로 올라가면 사요. 조건이 맞을 때만 사요.")).toBeTruthy();
    expect(screen.getByText("약 9시간")).toBeTruthy();
    expect(screen.getByText("21번 진입")).toBeTruthy();
    expect(screen.getByText("약 2일")).toBeTruthy();
    expect(act).not.toHaveBeenCalledWith("start_run", expect.anything());
    // 알림 끄고 시작
    fireEvent.click(screen.getByLabelText("사면 소리 · 알림으로 알려 주기"));
    fireEvent.click(screen.getByText("기다리며 시작"));
    await waitFor(() => expect(act).toHaveBeenCalledWith("start_run", { runId: "run-1" }));
    expect(act).toHaveBeenCalledWith("set_entry_notify", { runId: "run-1", on: false });
  });

  it("기다리지 않는 매크로나 서버가 답하지 않으면 안내 없이 바로 시작", async () => {
    for (const preview of [{ waiting: false }, new Error("offline")]) {
      const act = fakeAct(preview);
      render(<Harness act={act} />);
      fireEvent.click(screen.getByText("시작"));
      await waitFor(() => expect(act).toHaveBeenCalledWith("start_run", { runId: "run-1" }));
      expect(screen.queryByText("이 매크로는 바로 사지 않아요")).toBeNull();
      cleanup();
    }
  });
});
