import { Outlet } from "react-router-dom";
import { Box } from "@mantine/core";
import { IconSidebar } from "./IconSidebar";
import { useGlobalHotkeys } from "../lib/useGlobalHotkeys";

/**
 * Root window shell — two vertical regions side by side:
 *
 *   [ 74px IconSidebar ] [ flex: 1 main column ]
 *                        ┌──────── Outlet → SpaceShell ──────┐
 *                        │  Toolbar (40px, y=0)              │
 *                        │  main + ContextPanel (row)        │
 *                        └───────────────────────────────────┘
 *
 * The 74px IconSidebar fully contains the macOS traffic-light cluster
 * (8px left margin + 58px buttons + 8px symmetric right margin), so
 * the buttons are horizontally centered and never leak into the main
 * content area. This means the right column can start at y=0 — the
 * toolbar sits flush with the window top edge, like Finder or VS Code.
 *
 * Vertical alignment between the toolbar and the sidebar's first space
 * icon is intentionally NOT enforced here; the sidebar's first icon is
 * offset 40px from the top for the traffic lights, while the toolbar
 * occupies the right column's top 40px directly. This is the same
 * asymmetry every native macOS document app uses.
 *
 * Window dragging is provided by:
 *   - the IconSidebar's top 40px gap (above the first space icon)
 *   - the Toolbar's middle spacer (between left and right clusters)
 * Both are dedicated empty <div>s with `data-tauri-drag-region`, since
 * Tauri 2 with `titleBarStyle: Overlay` does not honour the CSS
 * `-webkit-app-region` contract and the attribute does not bubble.
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
        <Outlet />
      </Box>
    </Box>
  );
}
