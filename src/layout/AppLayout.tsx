import { Outlet } from "react-router-dom";
import { Box } from "@mantine/core";
import { IconSidebar } from "./IconSidebar";
import { useGlobalHotkeys } from "../lib/useGlobalHotkeys";

/**
 * Root window shell — three vertical regions side by side:
 *
 *   [ 48px IconSidebar ] [ flex: 1 main content area ] [ 320px ContextPanel ]
 *
 * The main content area is rendered by per-space components via React
 * Router's Outlet, and each space owns its own SpaceShell which wraps a
 * Toolbar + main + (optional) ContextPanel.
 *
 * The ContextPanel is per-space rather than global: knowledge graph and
 * library detail need different content models, and the panel's open state
 * is part of the space's local UI state, not a global concern.
 */
export function AppLayout() {
  useGlobalHotkeys();

  return (
    <Box
      style={{
        display: "flex",
        flexDirection: "row",
        height: "100vh",
        width: "100vw",
        overflow: "hidden",
      }}
    >
      <IconSidebar />
      <Outlet />
    </Box>
  );
}
