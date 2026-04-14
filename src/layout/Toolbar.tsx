import type { ReactNode } from "react";
import { Box, Group } from "@mantine/core";

interface ToolbarProps {
  /** Left cluster (typically search box, tabs, segmented control) */
  left?: ReactNode;
  /** Right cluster (typically view-mode switch, more menu) */
  right?: ReactNode;
}

/**
 * 40px tall horizontal toolbar pinned to the top of every space's main area.
 *
 * The whole bar is marked as a window drag region via `data-app-drag` (see
 * the global CSS in src/index.css). All interactive descendants — buttons,
 * inputs, selects, segmented controls, etc. — are automatically opted out
 * via the role-based selector list there, so the empty space between the
 * left and right clusters becomes the actual drag handle without each
 * caller having to think about it.
 */
export function Toolbar({ left, right }: ToolbarProps) {
  return (
    <Box
      component="header"
      data-app-drag
      style={{
        height: 40,
        flexShrink: 0,
        borderBottom: "0.5px solid var(--color-separator)",
        background: "var(--color-bg-primary)",
        display: "flex",
        alignItems: "center",
        paddingInline: "0.75rem",
        gap: "0.5rem",
      }}
    >
      <Group gap="0.5rem" style={{ flex: 1, minWidth: 0 }} wrap="nowrap">
        {left}
      </Group>
      <Group gap="0.5rem" style={{ flexShrink: 0 }} wrap="nowrap">
        {right}
      </Group>
    </Box>
  );
}
