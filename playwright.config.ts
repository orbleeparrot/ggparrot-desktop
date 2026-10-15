// 화면 배치 검사(e2e/) — 가짜 백엔드 미리보기(`npm run dev`)를 실제 브라우저 엔진으로 띄워 요소 위치·크기를 잼.
// - webkit: macOS 앱의 WKWebView와 같은 엔진(주 대상).
// - chromium: Windows 앱의 WebView2와 같은 엔진.
// - 픽셀 비교는 하지 않음(OS마다 글꼴이 달라 그림이 다름). 스크린샷은 사람이 볼 수 있게 결과에 붙이기만 함.
import { defineConfig, devices } from "@playwright/test";

// @types/node 없이 타입 검사가 통과하도록 쓰는 만큼만 선언함.
declare const process: { env: Record<string, string | undefined> };

const PORT = 4174;
const CI = !!process.env.CI;

export default defineConfig({
  testDir: "e2e",
  outputDir: "test-results",
  fullyParallel: true,
  forbidOnly: CI,
  retries: CI ? 1 : 0,
  reporter: [["list"], ["html", { open: "never", outputFolder: "playwright-report" }]],
  use: {
    baseURL: `http://localhost:${PORT}`,
    // 설정 목차 이동(scrollIntoView)이 부드럽게 굴러가지 않고 바로 가도록.
    reducedMotion: "reduce",
    locale: "ko-KR",
    timezoneId: "Asia/Seoul",
    trace: "retain-on-failure",
  },
  projects: [
    { name: "webkit", use: { ...devices["Desktop Safari"] } },
    { name: "chromium", use: { ...devices["Desktop Chrome"] } },
  ],
  webServer: {
    command: `npm run dev -- --port ${PORT} --strictPort`,
    url: `http://localhost:${PORT}`,
    reuseExistingServer: !CI,
    timeout: 60_000,
  },
});
