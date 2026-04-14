import { Outlet } from "react-router-dom";
import { Box } from "@mantine/core";
import { IconSidebar } from "./IconSidebar";
import { useGlobalHotkeys } from "../lib/useGlobalHotkeys";

/**
 * Root window shell — two vertical regions side by side:
 *
 *   [ 78px IconSidebar ] [ flex: 1 main column                         ]
 *                        ┌─ 40px drag stripe (vertical alignment) ─────┐
 *                        ├──────── Outlet → SpaceShell ────────────────┤
 *                        │  Toolbar (40px, y=40)                       │
 *                        │  main + ContextPanel (row)                  │
 *                        └─────────────────────────────────────────────┘
 *
 * The right column starts with a 40px drag stripe so the toolbar's top
 * edge aligns vertically with the sidebar's first space icon (which
 * sits 40px below the window top to clear the macOS traffic lights).
 * The stripe is also a `data-tauri-drag-region` so the user can drag
 * the window from above the toolbar — particularly important for
 * spaces (like Settings) whose main content area doesn't have its own
 * draggable spacer.
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
      <Box
        style={{
          flex: 1,
          display: "flex",
          flexDirection: "column",
          minWidth: 0,
          overflow: "hidden",
        }}
      >
        <div
          data-tauri-drag-region
          style={{ height: 40, flexShrink: 0, width: "100%" }}
        />
        <Box
          style={{
            flex: 1,
            display: "flex",
            flexDirection: "column",
            minWidth: 0,
            minHeight: 0,
            overflow: "hidden",
          }}
        >
          <Outlet />
        </Box>
      </Box>
    </Box>
  );
}
