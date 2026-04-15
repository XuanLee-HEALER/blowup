import type { ReactNode } from "react";
import { Box } from "@mantine/core";
import { Toolbar } from "./Toolbar";
import { ContextPanel } from "./ContextPanel";
import { MAIN_MIN_WIDTH } from "./constants";

interface SpaceShellProps {
  /** Toolbar left cluster */
  toolbarLeft?: ReactNode;
  /** Toolbar right cluster */
  toolbarRight?: ReactNode;
  /** Main content area (full-width list / canvas / form) */
  main: ReactNode;
  /** Context panel content (rendered when contextOpened=true) */
  context?: ReactNode;
  contextOpened?: boolean;
  onContextClose?: () => void;
}

/**
 * Per-space wrapper combining a toolbar on top of a flex row of
 * { main content area, optional 320px ContextPanel }.
 *
 *   ┌───────────────────────────── 40px Toolbar ─┐
 *   ├──────────────────────────────────┬─────────┤
 *   │           main (flex: 1)         │ context │
 *   │                                  │ (320px) │
 *   └──────────────────────────────────┴─────────┘
 */
export function SpaceShell({
  toolbarLeft,
  toolbarRight,
  main,
  context,
  contextOpened = false,
  onContextClose,
}: SpaceShellProps) {
  return (
    // `minHeight: 0` on every flex layer breaks the default
    // `min-height: auto` chain, which otherwise lets each container
    // grow to its content height and silently defeats any nested
    // `flex: 1` sizing attempts downstream.
    <Box
      component="section"
      style={{
        flex: 1,
        display: "flex",
        flexDirection: "column",
        minWidth: 0,
        minHeight: 0,
        overflow: "hidden",
        position: "relative",
      }}
    >
      <Toolbar left={toolbarLeft} right={toolbarRight} />
      <Box
        style={{
          flex: 1,
          display: "flex",
          minHeight: 0,
          overflow: "hidden",
          position: "relative",
        }}
      >
        <Box
          component="main"
          style={{
            flex: 1,
            minWidth: MAIN_MIN_WIDTH,
            minHeight: 0,
            overflow: "hidden",
            display: "flex",
            flexDirection: "column",
          }}
        >
          {main}
        </Box>
        {context !== undefined && (
          <ContextPanel opened={contextOpened} onClose={onContextClose ?? (() => {})}>
            {context}
          </ContextPanel>
        )}
      </Box>
    </Box>
  );
}
