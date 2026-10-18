// 레이아웃 셸 — 머리줄·실행 화면 또는 설정·매크로 추가 시트·연결 도우미(맨 위)·대화상자·알림.
import { CircleX, TriangleAlert } from "lucide-react";
import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { Dialog } from "./components/Dialog";
import { useMediaQuery } from "./components/focus";
import { Header } from "./components/Header";
import { DropOverlay } from "./components/library/DropOverlay";
import { LibrarySheet } from "./components/library/LibrarySheet";
import { RunScreen } from "./components/run/RunScreen";
import { useRunDialogs } from "./components/run/useRunDialogs";
import { SettingsScreen } from "./components/settings/SettingsScreen";
import { Toasts } from "./components/Toasts";
import { call, demoParam, errorText, type ArgsOf, type Command } from "./ipc";
import { RunnerContext, type Act, type RunnerContextValue } from "./state/context";
import { useNavigation } from "./state/useNavigation";
import { useNotices } from "./state/notices";
import { euro } from "./state/particle";
import { useRunner } from "./state/useRunner";
import { Wizard } from "./wizard/Wizard";

export default function App() {
  const notices = useNotices();
  const runner = useRunner(notices.push);
  const { state, loadError } = runner;
  const { toast } = notices;
  const wide = useMediaQuery("(min-width: 900px)");
  const nav = useNavigation(state, demoParam(), demoParam("open") === "1");
  const [libraryOpen, setLibraryOpen] = useState(() => demoParam() === "library");

  const act = useCallback<Act>(
    async (cmd, ...args) => {
      try {
        return await call(cmd, ...args);
      } catch (err) {
        toast(errorText(err), "error");
        return undefined;
      }
    },
    [toast],
  );

  const ok = useCallback(
    async <K extends Command>(cmd: K, ...args: ArgsOf<K>) => {
      try {
        await call(cmd, ...args);
        return true;
      } catch (err) {
        toast(errorText(err), "error");
        return false;
      }
    },
    [toast],
  );

  // 새 버전으로 막 바뀌었으면 한 번 알림.
  const greeted = useRef("");
  const updatedFrom = state?.update.updatedFrom ?? "";
  const currentVersion = state?.update.current ?? "";
  useEffect(() => {
    if (!updatedFrom || greeted.current === updatedFrom) return;
    greeted.current = updatedFrom;
    toast(`v${updatedFrom}에서 ${euro(`v${currentVersion}`)} 업데이트했어요.`, "info", "업데이트 완료");
  }, [updatedFrom, currentVersion, toast]);

  const openLibrary = useCallback(() => setLibraryOpen(true), []);
  const { setLibraryHandler } = nav;
  useEffect(() => {
    setLibraryHandler(openLibrary);
    return () => setLibraryHandler(null);
  }, [setLibraryHandler, openLibrary]);
  const flows = useRunDialogs({ state, act, toast, openSettings: nav.openSettings });

  const ctx = useMemo<RunnerContextValue | null>(
    () =>
      state
        ? {
            state,
            act,
            ok,
            toast,
            openSettings: nav.openSettings,
            openLibrary,
            select: nav.select,
            startRun: flows.startRun,
            askStop: flows.askStop,
            askRemove: flows.askRemove,
          }
        : null,
    [state, act, ok, toast, nav.openSettings, openLibrary, nav.select, flows.startRun, flows.askStop, flows.askRemove],
  );

  if (!ctx || !state) {
    return (
      <div className="boot" role="status">
        <img src="/brand/mark-ggparrot.svg" width={40} height={40} alt="" />
        {loadError ? (
          <p role="alert">
            앱 상태를 읽지 못했어요. 앱을 다시 시작해 보세요.
            <br />
            <span className="muted">{loadError}</span>
          </p>
        ) : (
          <p>껄무새를 준비하는 중…</p>
        )}
      </div>
    );
  }

  const dialog = notices.dialog;
  // 머리줄 바로가기의 대상 — 상세가 화면에 보일 때만(두 열이거나 한 열에서 상세를 연 때).
  const shownSlot =
    nav.view === "run" && (wide || nav.detailOpen) ? (state.runs.find((r) => r.id === nav.selectedId) ?? null) : null;

  return (
    <RunnerContext.Provider value={ctx}>
      <div className={`app view-${nav.view}`}>
        <Header
          view={nav.view}
          slot={shownSlot}
          onGear={nav.view === "settings" ? nav.openRun : () => nav.openSettings()}
        />
        <main className="app-main">
          {nav.view === "settings" ? (
            <SettingsScreen target={nav.target} onBack={nav.openRun} />
          ) : (
            <RunScreen
              runner={runner}
              selectedId={nav.selectedId}
              wide={wide}
              detailOpen={nav.detailOpen}
              onBack={nav.closeDetail}
            />
          )}
        </main>
      </div>
      {libraryOpen && (
        <LibrarySheet
          onClose={() => setLibraryOpen(false)}
          onAdded={(id) => {
            setLibraryOpen(false);
            nav.select(id);
          }}
        />
      )}
      {state.wizard && <Wizard wizard={state.wizard} />}
      {flows.dialogs}
      <DropOverlay />
      <Toasts
        toasts={notices.toasts}
        onDismiss={notices.dismissToast}
        onDetail={(id) => nav.openSettings({ kind: "log", id })}
      />
      {dialog && (
        <Dialog
          title={dialog.title}
          tone={dialog.level === "error" ? "danger" : "warn"}
          icon={dialog.level === "error" ? <CircleX size={20} /> : <TriangleAlert size={20} />}
          onClose={notices.closeDialog}
          actions={
            <button type="button" className="btn btn-primary" data-autofocus onClick={notices.closeDialog}>
              확인
            </button>
          }
        >
          <p className="pre">{dialog.message}</p>
        </Dialog>
      )}
    </RunnerContext.Provider>
  );
}
