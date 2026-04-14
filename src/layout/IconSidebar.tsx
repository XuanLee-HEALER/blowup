import { useLocation, useNavigate } from "react-router-dom";
import { ActionIcon, Box, Stack, Tooltip } from "@mantine/core";
import { IconSettings } from "@tabler/icons-react";
import { SPACES, activeSpaceFor } from "../lib/space";

/**
 * Fixed 48px-wide vertical sidebar with three space icons + settings cog.
 *
 *   - Top 40px is reserved as a drag region so macOS traffic-light buttons
 *     (decorations: true + titleBarStyle: Overlay) sit on top without
 *     overlapping any interactive element.
 *   - First space icon is offset 40px from the top for the same reason.
 *   - Settings cog at the bottom is pinned via flex spacer.
 *
 * No text labels per docs/blowup-layout-spec.md §53.
 */
export function IconSidebar() {
  const navigate = useNavigate();
  const { pathname } = useLocation();
  const activeSpace = activeSpaceFor(pathname);
  const settingsActive = pathname === "/settings" || pathname.startsWith("/settings/");

  return (
    <Box
      component="nav"
      style={{
        width: 48,
        flexShrink: 0,
        background: "var(--color-bg-secondary)",
        borderRight: "0.5px solid var(--color-separator)",
        display: "flex",
        flexDirection: "column",
        position: "relative",
      }}
    >
      {/* Top 40px drag region — sits behind the macOS traffic lights. */}
      <Box
        data-app-drag
        style={{
          height: 40,
          width: "100%",
          flexShrink: 0,
        }}
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
