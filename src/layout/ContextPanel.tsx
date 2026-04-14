import type { ReactNode } from "react";
import { Box } from "@mantine/core";
import { useViewportSize } from "@mantine/hooks";
import { CONTEXT_PANEL_OVERLAY_BREAKPOINT, CONTEXT_PANEL_WIDTH } from "./constants";

interface ContextPanelProps {
  opened: boolean;
  onClose: () => void;
  children: ReactNode;
}

/**
 * 320px right-side panel that slides in/out without unmounting its
 * children, so internal state (open menus, scroll position, in-flight
 * fetches) survives close/reopen cycles. The outer wrapper animates
 * width 0 ↔ 320 and the inner cell stays at a fixed 320 to prevent
 * content reflow during the transition.
 *
 * Below 850px the panel is absolutely positioned so it overlays the
 * main area instead of squeezing it.
 */
export function ContextPanel({ opened, onClose, children }: ContextPanelProps) {
  const { width } = useViewportSize();
  const isOverlay = width > 0 && width < CONTEXT_PANEL_OVERLAY_BREAKPOINT;

  return (
    <Box
      aria-label="上下文面板"
      role="complementary"
      onKeyDown={(e) => {
        if (e.key === "Escape") onClose();
      }}
      style={{
        width: opened ? CONTEXT_PANEL_WIDTH : 0,
        flexShrink: 0,
        overflow: "hidden",
        transition: "width 180ms ease-out",
        display: "flex",
        ...(isOverlay
          ? {
              position: "absolute",
              right: 0,
              top: 0,
              bottom: 0,
              zIndex: 20,
              boxShadow: opened ? "-8px 0 24px rgba(0, 0, 0, 0.15)" : "none",
            }
          : {}),
      }}
    >
      <Box
        style={{
          width: CONTEXT_PANEL_WIDTH,
          flexShrink: 0,
          borderLeft: "0.5px solid var(--color-separator)",
          background: "var(--color-bg-primary)",
          display: "flex",
          flexDirection: "column",
          overflow: "hidden",
        }}
      >
        {children}
      </Box>
    </Box>
  );
}
