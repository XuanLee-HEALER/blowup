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
 * Window dragging is provided by a dedicated empty <div> placed BETWEEN
 * the left and right clusters with `data-tauri-drag-region`. Tauri 2's
 * drag-region attribute does NOT bubble to descendants and CSS
 * `-webkit-app-region` is not honoured under `titleBarStyle: Overlay`,
 * so the spacer is a hard requirement — we cannot just mark the whole
 * toolbar bar.
 *
 * The spacer takes `flex: 1` to consume all space between the two
 * clusters, so wherever the user tries to grab the toolbar background
 * (anywhere not covered by an actual control) the drag works.
 */
export function Toolbar({ left, right }: ToolbarProps) {
  return (
    <Box
      component="header"
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
      <Group gap="0.5rem" style={{ flexShrink: 0, minWidth: 0 }} wrap="nowrap">
        {left}
      </Group>

      {/* Drag handle — fills the gap between clusters. */}
      <div data-tauri-drag-region style={{ flex: 1, alignSelf: "stretch" }} />

      <Group gap="0.5rem" style={{ flexShrink: 0 }} wrap="nowrap">
        {right}
      </Group>
    </Box>
  );
}
