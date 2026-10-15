// 설정 › 화면 — 백그라운드 실행·개발자 로그 보기.
import { useRunnerContext } from "../../state/context";
import { Section } from "./Section";
import { Toggle } from "./Toggle";

const isMac = typeof navigator !== "undefined" && /Mac/i.test(navigator.platform || navigator.userAgent);

export function DisplaySection() {
  const { state, act } = useRunnerContext();
  const where = isMac ? "메뉴 막대" : "작업 표시줄 알림 영역";
  return (
    <Section id="display" title="창 · 화면">
      <div className="plain-card">
        <Toggle
          label="창을 닫아도 계속 실행"
          hint={`창을 닫으면 ${where}의 껄무새 아이콘으로 숨어요. 매크로는 계속 돌고, 아이콘을 눌러 다시 열 수 있어요.${
            isMac ? " Dock에서도 사라져요." : ""
          } 끄면 매크로가 실행 중일 때는 창을 닫을 수 없어요.`}
          checked={state.settings.closeToTray}
          onChange={(value) => act("set_close_to_tray", { value })}
        />
        <Toggle
          label="개발자 로그 보기"
          hint="활동 기록에 원문 로그 줄까지 보여요. 문제를 알릴 때 켜 두면 좋아요."
          checked={state.settings.showDevLog}
          onChange={(show) => act("set_show_dev_log", { show })}
        />
      </div>
    </Section>
  );
}
