// 시작·종료·빼기 흐름과 그 대화상자. start_run의 답(실전 확인·막힘·확인 중)을 여기서 처리함.
// 답이 도우미(wizard)면 Rust가 이미 도우미를 열었으므로 여기서는 하지 않음.
// 시작 전 검사가 끝난 뒤 Rust가 보내는 runner:confirm도 같은 실전 확인 시트를 띄움.
// 조건을 기다리는 매크로는 [시작] 직후 서버에 물어(entry_preview) 둠:
// - 실전: 실전 확인 시트 한 장에 '바로 사지 않아요' 칸 · 돈 · 안전 칸을 함께(두 장으로 끊지 않음)
// - 모의 · 테스트넷: '바로 사지 않아요' 시트([기다리며 시작])
// 시트는 제목 바로 아래 테두리 칸 — 제목 밑 회색 설명 글을 두지 않음.
import { Hourglass, ShieldAlert, Trash2, TriangleAlert } from "lucide-react";
import { useCallback, useEffect, useRef, useState, type ReactNode } from "react";
import { on, type ConfirmPayload } from "../../ipc";
import type { Act } from "../../state/context";
import { settingsTargetOf, stopChoices, type SettingsTarget } from "../../state/runs";
import { EV_CONFIRM, type AppState, type EntryPreview, type StartReply, type StopMode } from "../../types";
import { ConfirmDialog } from "../ConfirmDialog";
import { MoneyCells } from "./MoneyCells";
import { StopFacts, WaitBox, WaitFacts } from "./SheetParts";

interface Deps {
  state: AppState | null;
  act: Act;
  toast(message: string): void;
  openSettings(target?: SettingsTarget): void;
}

export interface RunDialogs {
  startRun(runId: string): void;
  askStop(runId: string, mode: StopMode): void;
  askRemove(runId: string): void;
  dialogs: ReactNode;
}

export function useRunDialogs({ state, act, toast, openSettings }: Deps): RunDialogs {
  const [live, setLive] = useState<ConfirmPayload | null>(null);
  const [blocked, setBlocked] = useState<{ runId: string; message: string } | null>(null);
  const [stopAsk, setStopAsk] = useState<{ runId: string; mode: StopMode } | null>(null);
  const [removeAsk, setRemoveAsk] = useState<string | null>(null);
  const [waitAsk, setWaitAsk] = useState<{ runId: string; preview: EntryPreview; notify: boolean } | null>(null);
  // 실전 시작 확인 시트에 함께 보일 '바로 사지 않아요' — [시작] 때 물어 둔 답(매크로마다).
  const [previews, setPreviews] = useState<Record<string, EntryPreview>>({});
  const [liveNotify, setLiveNotify] = useState(true);
  const sending = useRef(new Set<string>());
  // 비동기 시작 흐름이 지금 모드를 읽을 곳 — 최신 상태는 effect에서 담음.
  const latest = useRef(state);
  useEffect(() => {
    latest.current = state;
  }, [state]);

  useEffect(() => {
    let alive = true;
    let off: (() => void) | undefined;
    on(EV_CONFIRM, (p) => alive && setLive(p))
      .then((u) => (alive ? (off = u) : u()))
      .catch((err) => {
        // 연결 확인 뒤 뜨는 실전 확인 창을 받지 못함 — 조용히 넘어가지 않게 알림.
        console.error("confirm listener failed", err);
        if (alive) toast("실전 시작 확인 창을 준비하지 못했어요. 앱을 다시 시작해 주세요.");
      });
    return () => {
      alive = false;
      off?.();
    };
  }, [toast]);

  const handle = useCallback(
    (runId: string, reply: StartReply | undefined) => {
      if (!reply) return;
      if (reply.kind === "confirm_live") setLive({ runId, title: reply.title, message: reply.message });
      else if (reply.kind === "blocked")
        setBlocked({ runId, message: reply.message || "지금은 시작할 수 없어요. 활동 기록에서 이유를 확인하세요." });
      else if (reply.kind === "checking") toast("연결을 확인하고 있어요. 확인이 끝나면 이어서 시작해요.");
    },
    [toast],
  );

  const send = useCallback(
    async (runId: string, cmd: "start_run" | "start_run_confirmed") => {
      if (sending.current.has(runId)) return;
      sending.current.add(runId);
      try {
        handle(runId, await act(cmd, { runId }));
      } finally {
        sending.current.delete(runId);
      }
    },
    [act, handle],
  );

  // 기다리는 매크로면 안내를 먼저 물어 둠 — 서버가 답하지 않으면 안내 없이 바로 시작(막지 않음).
  // 실전은 안내를 실전 확인 시트에 함께 넣고, 모의 · 테스트넷은 안내 시트를 먼저 띄움.
  const startRun = useCallback(
    (runId: string) =>
      void (async () => {
        if (sending.current.has(runId)) return;
        let preview: EntryPreview | undefined;
        try {
          preview = await act("entry_preview", { runId });
        } catch {
          preview = undefined;
        }
        const waiting = preview?.waiting && preview.sentence ? preview : null;
        const live = latest.current?.runs.find((r) => r.id === runId)?.mode === "live";
        setPreviews((prev) => {
          const next = { ...prev };
          if (waiting) next[runId] = waiting;
          else delete next[runId];
          return next;
        });
        if (waiting && !live) setWaitAsk({ runId, preview: waiting, notify: true });
        else {
          setLiveNotify(true);
          void send(runId, "start_run");
        }
      })(),
    [act, send],
  );
  const askStop = useCallback((runId: string, mode: StopMode) => setStopAsk({ runId, mode }), []);
  const askRemove = useCallback((runId: string) => setRemoveAsk(runId), []);

  const find = (id: string | undefined) => state?.runs.find((r) => r.id === id);
  const name = (id: string | undefined) => {
    const m = find(id)?.macro;
    return m ? `${m.symbol} · ${m.exchangeLabel || m.exchangeName}` : "";
  };
  // 확인 창은 지금 상태로 다시 고름 — 창을 연 사이 포지션이 생기거나 없어져도 문구가 맞게.
  const stopSlot = stopAsk ? find(stopAsk.runId) : undefined;
  const stopChoice = stopAsk && stopSlot ? stopChoices(stopSlot).find((c) => c.mode === stopAsk.mode) : undefined;
  const blockedTarget = blocked ? settingsTargetOf(find(blocked.runId)?.readiness.needs ?? null) : null;

  const liveSlot = live ? find(live.runId) : undefined;
  const livePreview = live ? previews[live.runId] : undefined;
  const dialogs = (
    <>
      {waitAsk && (
        <ConfirmDialog
          variant="sheet"
          icon={<Hourglass size={20} />}
          title="이 매크로는 바로 사지 않아요"
          onClose={() => setWaitAsk(null)}
          cancel={{ label: "취소" }}
          confirm={{
            label: "기다리며 시작",
            className: "btn-primary",
            run: () => {
              void act("set_entry_notify", { runId: waitAsk.runId, on: waitAsk.notify });
              void send(waitAsk.runId, "start_run");
            },
          }}
          large
        >
          <p className="dialog-meta num">{name(waitAsk.runId)}</p>
          <p>{waitAsk.preview.sentence} 조건이 맞을 때만 사요.</p>
          <WaitFacts typical={waitAsk.preview.typical} />
          <label className="check-row">
            <input
              type="checkbox"
              checked={waitAsk.notify}
              onChange={(e) => setWaitAsk({ ...waitAsk, notify: e.target.checked })}
            />
            <span>사면 소리 · 알림으로 알려 주기</span>
          </label>
        </ConfirmDialog>
      )}

      {live && (
        <ConfirmDialog
          variant="sheet"
          tone="live"
          icon={<ShieldAlert size={20} />}
          title={live.title}
          onClose={() => setLive(null)}
          cancel={{ label: "취소" }}
          confirm={{
            label: "실전으로 시작",
            className: "btn-live",
            run: () => {
              if (livePreview) void act("set_entry_notify", { runId: live.runId, on: liveNotify });
              void send(live.runId, "start_run_confirmed");
            },
          }}
          large
        >
          {livePreview && <WaitBox preview={livePreview} />}
          {liveSlot && <MoneyCells macro={liveSlot.macro} compact />}
          {livePreview && (
            <label className="check-row">
              <input type="checkbox" checked={liveNotify} onChange={(e) => setLiveNotify(e.target.checked)} />
              <span>사면 소리 · 알림으로 알려 주기</span>
            </label>
          )}
        </ConfirmDialog>
      )}

      {blocked && (
        <ConfirmDialog
          title="시작할 수 없어요"
          tone="warn"
          icon={<TriangleAlert size={20} />}
          onClose={() => setBlocked(null)}
          cancel={blockedTarget ? { label: "설정에서 입력", run: () => openSettings(blockedTarget) } : undefined}
          confirm={{ label: "확인", className: "btn-primary" }}
          focus="confirm"
        >
          <p className="pre">{blocked.message}</p>
        </ConfirmDialog>
      )}

      {stopAsk && (
        <ConfirmDialog
          variant="sheet"
          title={`${stopChoice?.label ?? "종료"}할까요?`}
          tone={stopChoice?.danger ? "danger" : "default"}
          onClose={() => setStopAsk(null)}
          cancel={{ label: "취소" }}
          confirm={{
            label: stopChoice?.label ?? "종료",
            className: "btn-danger-solid",
            run: () => void act("stop_run", { runId: stopAsk.runId, mode: stopAsk.mode }),
          }}
          large
        >
          <StopFacts slot={stopSlot} choice={stopChoice} />
        </ConfirmDialog>
      )}

      {removeAsk && (
        <ConfirmDialog
          title="목록에서 뺄까요?"
          icon={<Trash2 size={18} />}
          onClose={() => setRemoveAsk(null)}
          cancel={{ label: "취소" }}
          confirm={{
            label: "목록에서 빼기",
            className: "btn-danger-solid",
            run: () => void act("remove_run", { runId: removeAsk }),
          }}
        >
          <p>
            {find(removeAsk)?.macro.source === "web"
              ? "웹에 저장한 매크로는 그대로예요. 이 매크로의 활동 기록은 지워져요."
              : "매크로 파일은 지우지 않아요. 이 매크로의 활동 기록은 지워져요."}
          </p>
          <p className="dialog-meta num">{name(removeAsk)}</p>
        </ConfirmDialog>
      )}
    </>
  );

  return { startRun, askStop, askRemove, dialogs };
}
