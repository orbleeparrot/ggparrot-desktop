// 설정 › 정보의 앱 버전 줄 아래 두 줄 — 상태(무엇이 일어났나)와 덧붙임(다음에 할 일·마지막 확인).
import type { UpdateState } from "../types";
import { formatWhen } from "./format";
import { iga } from "./particle";

/** 상태 줄 앞 아이콘. 없으면 null. */
export type UpdateIcon = "updated" | "latest" | "installing" | "error" | null;

export interface UpdateCopy {
  headline: { icon: UpdateIcon; text: string };
  /** 덧붙임 한 줄. 없으면 "". */
  detail: string;
}

/**
 * 업데이트 상태별 문구.
 * - running: 실행 중인 매크로 수(설치 준비가 끝났을 때 막힌 이유).
 * - windows: Windows는 설치 창이 잠깐 뜸.
 */
export function updateCopy(u: UpdateState, running: number, windows: boolean): UpdateCopy {
  const checked = u.checkedAt > 0 ? `마지막 확인 ${formatWhen(u.checkedAt)}` : "아직 확인하지 않았어요";
  const line = (text: string, icon: UpdateIcon = null) => ({ icon, text });
  switch (u.status) {
    case "none":
      return {
        headline: u.updatedFrom
          ? line(`v${u.updatedFrom}에서 업데이트했어요`, "updated")
          : line("최신 버전이에요", "latest"),
        detail: checked,
      };
    case "available":
      return {
        headline: line(`새 버전 ${iga(`v${u.version}`)} 나왔어요`),
        detail: "내려받는 동안에도 매크로는 계속 돌아요.",
      };
    case "downloading":
      return {
        headline: line(`v${u.version} 내려받는 중`),
        detail: "다 받으면 서명을 확인해요. 매크로는 계속 돌아요.",
      };
    case "ready":
      return {
        headline: line(`v${u.version} 설치 준비가 끝났어요`),
        detail:
          running > 0
            ? `실행 중인 매크로 ${running}개를 모두 종료하면 설치할 수 있어요. 받은 파일은 그대로 있어요.`
            : windows
              ? "다시 시작하면 설치 창이 잠깐 뜬 뒤 새 버전으로 켜져요."
              : "다시 시작하면 새 버전으로 바뀌어요.",
      };
    case "installing":
      return {
        headline: line(`v${u.version} 설치하는 중`, "installing"),
        detail: windows ? "설치 창이 뜨고 끝나면 앱이 다시 켜져요." : "곧 앱이 다시 켜져요.",
      };
    case "error":
      return { headline: line("업데이트 중 문제가 생겼어요", "error"), detail: checked };
    case "unsupported":
      return { headline: line("이 빌드는 자동 업데이트를 쓰지 않아요"), detail: "개발용 빌드예요." };
    case "checking":
      return { headline: line("새 버전을 확인하는 중…"), detail: "" };
    default:
      return { headline: line("아직 업데이트를 확인하지 않았어요"), detail: "" };
  }
}
