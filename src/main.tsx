import "@mantine/core/styles.css";
import "./index.css";
import "./lib/mantine-overrides.css";

import React from "react";
import ReactDOM from "react-dom/client";
import { HashRouter } from "react-router-dom";
import { MantineProvider } from "@mantine/core";
import App from "./App";
import { theme } from "./lib/theme";

console.log("[blowup] main.tsx executing", performance.now().toFixed(0) + "ms");

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <MantineProvider theme={theme} defaultColorScheme="light">
      <HashRouter>
        <App />
      </HashRouter>
    </MantineProvider>
  </React.StrictMode>
);

// Fade out splash screen once React has rendered
requestAnimationFrame(() => {
  const splash = document.getElementById("splash");
  if (splash) {
    splash.classList.add("fade");
    setTimeout(() => splash.remove(), 350);
  }
});
