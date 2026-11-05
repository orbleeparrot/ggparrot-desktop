import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import type { Act } from "../../state/context";
import { binanceMacro, makeSlot, makeState, upbitMacro } from "../../test/fixtures";
import type { EntryPreview } from "../../types";
import { REMOVE_NAMES_SHOWN, removeManyText, useRunDialogs } from "./useRunDialogs";

vi.mock("../../ipc", () => ({ on: () => Promise.resolve(() => undefined) }));

afterEach(cleanup);

function Harness({ act }: { act: Act }) {
  const state = makeState({ runs: [makeSlot()] });
  const flows = useRunDialogs({ state, act, toast: vi.fn(), openSettings: vi.fn() });
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
    // 알림은 설정 '알림' 하나로 켜고 끔 — 시작 확인 창에 실행마다 고르는 체크가 없음
    expect(screen.queryByRole("checkbox")).toBeNull();
    fireEvent.click(screen.getByText("기다리며 시작"));
    await waitFor(() => expect(act).toHaveBeenCalledWith("start_run", { runId: "run-1" }));
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

describe("목록에서 빼기 — 여러 개", () => {
  function ManyHarness({ act, toast, ids }: { act: Act; toast: (m: string) => void; ids: string[] }) {
    const runs = [
      makeSlot({ id: "a", macro: { ...binanceMacro, symbol: "BTCUSDT" } }),
      makeSlot({ id: "b", macro: { ...upbitMacro, symbol: "KRW-ETH" } }),
      ...["c", "d", "e", "f", "g"].map((id) =>
        makeSlot({ id, macro: { ...binanceMacro, symbol: `${id.toUpperCase()}USDT` } }),
      ),
    ];
    const flows = useRunDialogs({ state: makeState({ runs }), act, toast, openSettings: vi.fn() });
    return (
      <>
        <button type="button" onClick={() => flows.askRemoveMany(ids)}>
          빼기
        </button>
        {flows.dialogs}
      </>
    );
  }

  it("여럿이면 '{N}개를 목록에서 뺄까요?' — 한 번에 remove_runs, 남긴 것은 토스트", async () => {
    const act = vi.fn(async () => ({ removed: 1, kept: 1 })) as unknown as Act & ReturnType<typeof vi.fn>;
    const toast = vi.fn();
    render(<ManyHarness act={act} toast={toast} ids={["a", "b"]} />);
    fireEvent.click(screen.getByText("빼기"));
    expect(screen.getByText("2개를 목록에서 뺄까요?")).toBeTruthy();
    expect(screen.getByText("BTCUSDT · 바이낸스")).toBeTruthy();
    fireEvent.click(screen.getByRole("button", { name: "목록에서 빼기" }));
    await waitFor(() => expect(act).toHaveBeenCalledWith("remove_runs", { runIds: ["a", "b"] }));
    await waitFor(() => expect(toast).toHaveBeenCalledWith("실행 중인 1개는 남겼어요."));
  });

  it("다 뺐으면 토스트 없음", async () => {
    const act = vi.fn(async () => ({ removed: 2, kept: 0 })) as unknown as Act & ReturnType<typeof vi.fn>;
    const toast = vi.fn();
    render(<ManyHarness act={act} toast={toast} ids={["a", "c"]} />);
    fireEvent.click(screen.getByText("빼기"));
    fireEvent.click(screen.getByRole("button", { name: "목록에서 빼기" }));
    await waitFor(() => expect(act).toHaveBeenCalledTimes(1));
    expect(toast).not.toHaveBeenCalled();
  });

  it("하나면 지금의 '목록에서 뺄까요?' 창 그대로(remove_run)", async () => {
    const act = vi.fn(async () => undefined) as unknown as Act & ReturnType<typeof vi.fn>;
    render(<ManyHarness act={act} toast={vi.fn()} ids={["a"]} />);
    fireEvent.click(screen.getByText("빼기"));
    expect(screen.getByText("목록에서 뺄까요?")).toBeTruthy();
    expect(screen.getByText("매크로 파일은 지우지 않아요. 이 매크로의 활동 기록은 지워져요.")).toBeTruthy();
    fireEvent.click(screen.getByRole("button", { name: "목록에서 빼기" }));
    await waitFor(() => expect(act).toHaveBeenCalledWith("remove_run", { runId: "a" }));
  });

  it(`이름은 ${REMOVE_NAMES_SHOWN}줄까지, 넘으면 '외 N개' · 없어진 매크로는 셈하지 않음`, () => {
    render(
      <ManyHarness act={vi.fn() as unknown as Act} toast={vi.fn()} ids={["a", "b", "c", "d", "e", "f", "g", "gone"]} />,
    );
    fireEvent.click(screen.getByText("빼기"));
    expect(screen.getByText("7개를 목록에서 뺄까요?")).toBeTruthy();
    expect(screen.getByText("외 2개")).toBeTruthy();
    expect(screen.queryByText("GUSDT · 바이낸스")).toBeNull();
  });
});

describe("removeManyText", () => {
  it("고른 매크로의 출처에 맞는 문장만", () => {
    expect(removeManyText(["file", "file"])).toBe("매크로 파일은 지우지 않아요. 고른 매크로의 활동 기록은 지워져요.");
    expect(removeManyText(["web"])).toBe("웹에 저장한 매크로는 그대로예요. 고른 매크로의 활동 기록은 지워져요.");
    expect(removeManyText(["file", "web"])).toBe(
      "매크로 파일은 지우지 않아요. 웹에 저장한 매크로는 그대로예요. 고른 매크로의 활동 기록은 지워져요.",
    );
  });
});
