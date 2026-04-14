import { Outlet } from "react-router-dom";
import { Box } from "@mantine/core";
import { IconSidebar } from "./IconSidebar";
import { useGlobalHotkeys } from "../lib/useGlobalHotkeys";

/**
 * Root window shell — three vertical regions side by side:
 *
 *   [ 48px IconSidebar ] [ flex: 1 main column ]
 *                        ┌─ 40px top drag region (avoids traffic lights) ─┐
 *                        ├──────────── Outlet → SpaceShell ───────────────┤
 *                        │  Toolbar (40px)                                │
 *                        │  main + ContextPanel (row)                     │
 *                        └────────────────────────────────────────────────┘
 *
 * The 40px top stripe in the right column is critical: macOS draws the
 * traffic-light buttons (titleBarStyle: Overlay + decorations: true) over
 * the top-left of the window, and they extend ~78px to the right —
 * past the 48px IconSidebar and into the main content area. Without
 * the top stripe, the toolbar would render directly under the traffic
 * lights and any search box / button there would be visually clipped.
 *
 * The IconSidebar handles its own 40px traffic-light gap internally, so
 * both columns are vertically aligned (sidebar's first space icon and
 * the right column's Toolbar both start at y=40).
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
        {/* Top 40px stripe: covers the macOS traffic-light overlay area
            and acts as a drag handle. Every space below renders from
            y=40, vertically aligned with the IconSidebar's first icon. */}
        <Box data-app-drag style={{ height: 40, flexShrink: 0 }} />
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
    </Box>
  );
}
