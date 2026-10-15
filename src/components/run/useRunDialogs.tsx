// 시작·종료·빼기 흐름과 그 대화상자. start_run의 답(실전 확인·막힘·확인 중)을 여기서 처리함.
// 답이 도우미(wizard)면 Rust가 이미 도우미를 열었으므로 여기서는 하지 않음.
// 시작 전 검사가 끝난 뒤 Rust가 보내는 runner:confirm도 같은 실전 확인 시트를 띄움.
import { ShieldAlert, Trash2, TriangleAlert } from "lucide-react";
import { useCallback, useEffect, useRef, useState, type ReactNode } from "react";
import { on, type ConfirmPayload } from "../../ipc";
import type { Act } from "../../state/context";
import { settingsTargetOf, stopChoices, type SettingsTarget } from "../../state/runs";
import { EV_CONFIRM, type AppState, type StartReply, type StopMode } from "../../types";
import { Dialog } from "../Dialog";

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
  const sending = useRef(new Set<string>());

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

  const startRun = useCallback((runId: string) => void send(runId, "start_run"), [send]);
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

  const dialogs = (
    <>
      {live && (
        <Dialog
          variant="sheet"
          tone="live"
          icon={<ShieldAlert size={20} />}
          title={live.title}
          onClose={() => setLive(null)}
          actions={
            <>
              <button type="button" className="btn btn-ghost btn-lg" data-autofocus onClick={() => setLive(null)}>
                취소
              </button>
              <button
                type="button"
                className="btn btn-live btn-lg"
                onClick={() => {
                  const id = live.runId;
                  setLive(null);
                  void send(id, "start_run_confirmed");
                }}
              >
                실전으로 시작
              </button>
            </>
          }
        >
          <p className="pre">{live.message}</p>
        </Dialog>
      )}

      {blocked && (
        <Dialog
          title="시작할 수 없어요"
          tone="warn"
          icon={<TriangleAlert size={20} />}
          onClose={() => setBlocked(null)}
          actions={
            <>
              {blockedTarget && (
                <button
                  type="button"
                  className="btn btn-ghost"
                  onClick={() => {
                    setBlocked(null);
                    openSettings(blockedTarget);
                  }}
                >
                  설정에서 입력
                </button>
              )}
              <button type="button" className="btn btn-primary" data-autofocus onClick={() => setBlocked(null)}>
                확인
              </button>
            </>
          }
        >
          <p className="pre">{blocked.message}</p>
        </Dialog>
      )}

      {stopAsk && (
        <Dialog
          title={`${stopChoice?.label ?? "종료"}할까요?`}
          tone={stopChoice?.danger ? "danger" : "default"}
          onClose={() => setStopAsk(null)}
          actions={
            <>
              <button type="button" className="btn btn-ghost" data-autofocus onClick={() => setStopAsk(null)}>
                취소
              </button>
              <button
                type="button"
                className={`btn ${stopChoice?.danger ? "btn-danger-solid" : "btn-primary"}`}
                onClick={() => {
                  const { runId, mode } = stopAsk;
                  setStopAsk(null);
                  void act("stop_run", { runId, mode });
                }}
              >
                {stopChoice?.label ?? "종료"}
              </button>
            </>
          }
        >
          <p>{stopChoice?.hint ?? "매크로를 끝내요."}</p>
          <p className="dialog-meta num">{name(stopAsk.runId)}</p>
        </Dialog>
      )}

      {removeAsk && (
        <Dialog
          title="목록에서 뺄까요?"
          icon={<Trash2 size={18} />}
          onClose={() => setRemoveAsk(null)}
          actions={
            <>
              <button type="button" className="btn btn-ghost" data-autofocus onClick={() => setRemoveAsk(null)}>
                취소
              </button>
              <button
                type="button"
                className="btn btn-danger-solid"
                onClick={() => {
                  const runId = removeAsk;
                  setRemoveAsk(null);
                  void act("remove_run", { runId });
                }}
              >
                목록에서 빼기
              </button>
            </>
          }
        >
          <p>
            {find(removeAsk)?.macro.source === "web"
              ? "웹에 저장한 매크로는 그대로예요. 이 매크로의 활동 기록은 지워져요."
              : "매크로 파일은 지우지 않아요. 이 매크로의 활동 기록은 지워져요."}
          </p>
          <p className="dialog-meta num">{name(removeAsk)}</p>
        </Dialog>
      )}
    </>
  );

  return { startRun, askStop, askRemove, dialogs };
}
