import { useLocation, useNavigate } from "react-router-dom";
import { ActionIcon, Box, Stack, Tooltip } from "@mantine/core";
import { IconSettings } from "@tabler/icons-react";
import { SPACES, activeSpaceFor } from "../lib/space";
import { SIDEBAR_WIDTH, TOPBAR_HEIGHT } from "./constants";

/** Vertical sidebar with three space icons + a settings cog at the
 *  bottom. The first icon sits below a fixed-height top stripe so the
 *  macOS traffic-light overlay (titleBarStyle: Overlay) doesn't cover
 *  any interactive element. */
export function IconSidebar() {
  const navigate = useNavigate();
  const { pathname } = useLocation();
  const activeSpace = activeSpaceFor(pathname);
  const settingsActive = pathname === "/settings" || pathname.startsWith("/settings/");

  return (
    <Box
      component="nav"
      style={{
        width: SIDEBAR_WIDTH,
        flexShrink: 0,
        background: "var(--color-bg-secondary)",
        borderRight: "0.5px solid rgba(60, 60, 67, 0.12)",
        display: "flex",
        flexDirection: "column",
        position: "relative",
      }}
    >
      {/* Top stripe sits behind the macOS traffic lights and acts as a
          drag region. Must be a plain <div> with data-tauri-drag-region
          — the attribute does not bubble to descendants. */}
      <div
        data-tauri-drag-region
        style={{ height: TOPBAR_HEIGHT, width: "100%", flexShrink: 0 }}
      />

      <Stack gap={8} align="center" style={{ flexShrink: 0 }}>
        {SPACES.map((space) => {
          const isActive = activeSpace?.id === space.id && !settingsActive;
          return (
            <Tooltip
              key={space.id}
              label={`${space.label}  ⌘${space.shortcutDigit}`}
              position="right"
              withArrow
              openDelay={400}
            >
              <ActionIcon
                variant={isActive ? "light" : "subtle"}
                color={isActive ? "accent" : "gray"}
                size={40}
                radius="md"
                onClick={() => navigate(space.route)}
                aria-label={space.label}
              >
                <space.Icon size={20} stroke={1.5} />
              </ActionIcon>
            </Tooltip>
          );
        })}
      </Stack>

      <Box style={{ flex: 1 }} />

      {/* Bottom: settings cog */}
      <Stack gap={8} align="center" pb={12} style={{ flexShrink: 0 }}>
        <Tooltip label="设置  ⌘," position="right" withArrow openDelay={400}>
          <ActionIcon
            variant={settingsActive ? "light" : "subtle"}
            color={settingsActive ? "accent" : "gray"}
            size={40}
            radius="md"
            onClick={() => navigate("/settings")}
            aria-label="设置"
          >
            <IconSettings size={20} stroke={1.5} />
          </ActionIcon>
        </Tooltip>
      </Stack>
    </Box>
  );
}
