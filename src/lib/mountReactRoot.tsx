import "@mantine/core/styles.css";
import "../index.css";

import React, { type ReactNode } from "react";
import ReactDOM from "react-dom/client";
import { MantineProvider } from "@mantine/core";
import { theme } from "./theme";

/**
 * Mount a React tree into the `#root` element with the standard
 * provider stack (StrictMode + MantineProvider + shared theme).
 *
 * Each Tauri window entry point (main, player, subtitle viewer,
 * waveform) used to repeat this boilerplate verbatim — they now all
 * call this helper instead.
 */
export function mountReactRoot(children: ReactNode) {
  const root = document.getElementById("root");
  if (!root) throw new Error("missing #root element");

  ReactDOM.createRoot(root).render(
    <React.StrictMode>
      <MantineProvider theme={theme} defaultColorScheme="light">
        {children}
      </MantineProvider>
    </React.StrictMode>
  );
}
