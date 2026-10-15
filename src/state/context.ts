// 컴포넌트가 상태 스냅샷·명령 보내기(act)·화면 이동·시작/종료 흐름을 꺼내 쓰는 곳.
import { createContext, useContext } from "react";
import type { ArgsOf, Command, CommandMap } from "../ipc";
import type { AppState, StopMode } from "../types";
import type { SettingsTarget } from "./runs";

/** 명령을 보내고, 실패하면 오류 문구를 토스트로 띄운 뒤 undefined를 돌려줌. */
export type Act = <K extends Command>(cmd: K, ...args: ArgsOf<K>) => Promise<CommandMap[K]["ret"] | undefined>;

export interface RunnerContextValue {
  state: AppState;
  act: Act;
  /** 반환값이 없는 명령용 — 성공했는지만 알려 줌(실패 문구는 act처럼 토스트로). */
  ok<K extends Command>(cmd: K, ...args: ArgsOf<K>): Promise<boolean>;
  /** 짧은 안내 토스트(4초). */
  toast(message: string, tone?: "info" | "error"): void;
  /** 설정 화면을 엶. target이 있으면 그 칸으로 굴리고 포커스를 둠. */
  openSettings(target?: SettingsTarget): void;
  /** 매크로 폴더에서 고르는 시트를 엶. */
  openLibrary(): void;
  /** 선택한 슬롯을 바꿈(한 열에서는 상세로 들어감). */
  select(runId: string): void;
  /** 시작 — 실전 확인·막힘 대화상자까지 처리함. */
  startRun(runId: string): void;
  /** 종료 — "{label} 할까요?"를 먼저 물음. */
  askStop(runId: string, mode: StopMode): void;
  /** 목록에서 빼기 — 먼저 물음. */
  askRemove(runId: string): void;
}

export const RunnerContext = createContext<RunnerContextValue | null>(null);

export function useRunnerContext(): RunnerContextValue {
  const ctx = useContext(RunnerContext);
  if (!ctx) throw new Error("RunnerContext 가 없어요");
  return ctx;
}
