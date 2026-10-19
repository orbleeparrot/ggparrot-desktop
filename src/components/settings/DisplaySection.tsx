// 설정 › 창 · 화면 — 화면 테마(시스템 · 라이트 · 다크) · 백그라운드 실행 · 알림 · 개발자 로그 보기.
import { useRunnerContext } from "../../state/context";
import { THEME_LABELS, useThemePref, type ThemePref } from "../../state/theme";
import { Section } from "./Section";
import { Toggle } from "./Toggle";

const isMac = typeof navigator !== "undefined" && /Mac/i.test(navigator.platform || navigator.userAgent);

export function DisplaySection() {
  const { state, act } = useRunnerContext();
  const where = isMac ? "메뉴 막대" : "작업 표시줄 알림 영역";
  const [theme, setTheme] = useThemePref();
  return (
    <Section id="display" title="창 · 화면">
      <div className="plain-card">
        <div className="toggle-row">
          <div className="toggle-text">
            <span id="theme-label" className="strong">
              화면 테마
            </span>
            <span className="muted">시스템은 OS의 라이트 · 다크 설정을 따라가요.</span>
          </div>
          <div className="theme-pick" role="radiogroup" aria-labelledby="theme-label">
            {(Object.keys(THEME_LABELS) as ThemePref[]).map((k) => (
              <button
                key={k}
                type="button"
                role="radio"
                aria-checked={theme === k}
                className={`theme-pick-item ${theme === k ? "is-on" : ""}`}
                onClick={() => setTheme(k)}
              >
                {THEME_LABELS[k]}
              </button>
            ))}
          </div>
        </div>
        <Toggle
          label="창을 닫아도 계속 실행"
          hint={`창을 닫으면 ${where}의 껄무새 아이콘으로 숨어요. 매크로는 계속 돌고, 아이콘을 눌러 다시 열 수 있어요.${
            isMac ? " Dock에서도 사라져요." : ""
          } 끄면 매크로가 실행 중일 때는 창을 닫을 수 없어요.`}
          checked={state.settings.closeToTray}
          onChange={(value) => act("set_close_to_tray", { value })}
        />
        <Toggle
          label="알림"
          hint="사고판 체결 · 평가손익 ±3%부터 구간마다 · 주문 오류와 잔고 부족 · 서버 끊김 · 실행이 멈춤을 알림음과 함께 화면 구석(Windows는 오른쪽 아래)에 띄워요."
          checked={state.settings.notify}
          onChange={(value) => act("set_notify", { value })}
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
