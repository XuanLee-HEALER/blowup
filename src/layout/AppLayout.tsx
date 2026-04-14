import { Outlet } from "react-router-dom";
import { Box } from "@mantine/core";
import { IconSidebar } from "./IconSidebar";
import { TOPBAR_HEIGHT } from "./constants";
import { useGlobalHotkeys } from "../lib/useGlobalHotkeys";

/** Root window shell. The right column starts with a drag stripe
 *  matching the macOS traffic-light overlay height so the per-space
 *  toolbar lines up with the sidebar's first icon and Settings (which
 *  has no toolbar of its own) still has a draggable area. */
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
          minHeight: 0,
          overflow: "hidden",
        }}
      >
        <div
          data-tauri-drag-region
          style={{ height: TOPBAR_HEIGHT, flexShrink: 0, width: "100%" }}
        />
        <Outlet />
      </Box>
    </Box>
  );
}
