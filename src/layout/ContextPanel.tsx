import type { ReactNode } from "react";
import { Box, Transition } from "@mantine/core";
import { useViewportSize } from "@mantine/hooks";

interface ContextPanelProps {
  opened: boolean;
  onClose: () => void;
  children: ReactNode;
}

/**
 * 320px right-side panel that slides in from the right when an item is
 * selected in the main area. Always rendered (DOM persistent) and animated
 * via Mantine's `Transition` so children keep their internal state across
 * close/reopen cycles.
 *
 * Below 850px viewport width the panel switches to overlay mode (absolute
 * positioning stacked on top of the main area) so it doesn't squeeze the
 * list view to nothing.
 */
export function ContextPanel({ opened, onClose, children }: ContextPanelProps) {
  const { width } = useViewportSize();
  const isOverlay = width > 0 && width < 850;

  return (
    <Transition mounted={opened} transition="slide-left" duration={180} timingFunction="ease-out">
      {(transitionStyles) => (
        <Box
          aria-label="上下文面板"
          role="complementary"
          onKeyDown={(e) => {
            if (e.key === "Escape") onClose();
          }}
          style={{
            ...transitionStyles,
            width: 320,
            flexShrink: 0,
            borderLeft: "0.5px solid var(--color-separator)",
            background: "var(--color-bg-primary)",
            display: "flex",
            flexDirection: "column",
            overflow: "hidden",
            // Overlay mode below 850px: absolute on top of main area
            ...(isOverlay
              ? {
                  position: "absolute",
                  right: 0,
                  top: 0,
                  bottom: 0,
                  zIndex: 20,
                  boxShadow: "-8px 0 24px rgba(0, 0, 0, 0.15)",
                }
              : {}),
          }}
        >
          {children}
        </Box>
      )}
    </Transition>
  );
}
