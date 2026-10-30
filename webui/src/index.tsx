import "./styles/base.css";

import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import { App } from "./app/App";
import { installLogCapture } from "./console/log";
import { hydrateAppStorage } from "./core/storage";

installLogCapture();

// Settings and save state are read synchronously by the first render, so the
// stores they come from are loaded before the app mounts.
void hydrateAppStorage().finally(() => {
  createRoot(document.getElementById("root")!).render(
    <StrictMode>
      <App />
    </StrictMode>,
  );
});
