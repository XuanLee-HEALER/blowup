import type { ReactNode } from "react";
import { Box, Group } from "@mantine/core";
import { TOOLBAR_HEIGHT } from "./constants";

interface ToolbarProps {
  /** Left cluster (typically search box, tabs, segmented control) */
  left?: ReactNode;
  /** Right cluster (typically view-mode switch, more menu) */
  right?: ReactNode;
}

/** Per-space toolbar. The drag spacer between left/right clusters is
 *  a dedicated empty <div> because data-tauri-drag-region does not
 *  bubble to descendants. */
export function Toolbar({ left, right }: ToolbarProps) {
  return (
    <Box
      component="header"
      style={{
        height: TOOLBAR_HEIGHT,
        flexShrink: 0,
        borderBottom: "0.5px solid var(--color-separator)",
        background: "var(--color-bg-primary)",
        display: "flex",
        alignItems: "center",
        paddingInline: "0.75rem",
        gap: "0.5rem",
      }}
    >
      <Group gap="0.5rem" style={{ flexShrink: 0, minWidth: 0 }} wrap="nowrap">
        {left}
      </Group>

      <div data-tauri-drag-region style={{ flex: 1, alignSelf: "stretch" }} />

      <Group gap="0.5rem" style={{ flexShrink: 0 }} wrap="nowrap">
        {right}
      </Group>
    </Box>
  );
}
