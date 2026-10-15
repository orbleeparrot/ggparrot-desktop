import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import App from "./App";
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

createRoot(document.getElementById("root")!).render(
  <StrictMode>
    <App />
  </StrictMode>,
);
