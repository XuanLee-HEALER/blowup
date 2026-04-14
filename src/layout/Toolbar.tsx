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
 * The toolbar background itself is a `data-tauri-drag-region`, but each
 * interactive child (search input, button, segmented control) must opt out
 * of dragging via `data-tauri-drag-region={false}` on the child. Mantine's
 * inputs and buttons render a real DOM element so we just leak the attribute
 * down via `getElementProps` … or simpler: rely on the fact that buttons
 * naturally swallow drag events.
 *
 * In practice we set drag on the wrapper Box and let Mantine controls inside
 * stop propagation through their default click/mousedown handlers. The
 * non-interactive empty space (between left and right clusters) stays
 * draggable.
 */
export function Toolbar({ left, right }: ToolbarProps) {
  return (
    <Box
      component="header"
      data-tauri-drag-region
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
