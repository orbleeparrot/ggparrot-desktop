import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import App from "./App";
import { lockAppShell } from "./appShell";
import { isTauri } from "./ipc";
import { markPlatform } from "./platform";
import "./styles/theme.css";
import "./styles/base.css";
import "./styles/layout.css";
import "./styles/cards.css";
import "./styles/run.css";
import "./styles/activity.css";
import "./styles/settings.css";
import "./styles/library.css";
import "./styles/wizard.css";
import "./styles/dialog.css";

markPlatform();
// 배포 빌드의 앱 창에서만 — 개발 중에는 새로고침·검사 도구를 씀.
if (isTauri() && import.meta.env.PROD) lockAppShell();

createRoot(document.getElementById("root")!).render(
  <StrictMode>
    <App />
  </StrictMode>,
);
