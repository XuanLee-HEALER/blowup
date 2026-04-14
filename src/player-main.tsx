import "@mantine/core/styles.css";
import "./index.css";

import React from "react";
import ReactDOM from "react-dom/client";
import { MantineProvider } from "@mantine/core";
import { Player } from "./Player";
import { theme } from "./lib/theme";

ReactDOM.createRoot(document.getElementById("root")!).render(
  <React.StrictMode>
    <MantineProvider theme={theme} defaultColorScheme="light">
      <Player />
    </MantineProvider>
  </React.StrictMode>
);
