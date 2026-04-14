import { Box, ScrollArea } from "@mantine/core";
import Settings from "../pages/Settings";

export function SettingsOverlay() {
  return (
    // minHeight: 0 lets the ScrollArea inside actually constrain its
    // height — without it the flex item defaults to content-driven
    // min-height and the scroll area silently falls back to full height.
    <Box
      style={{
        flex: 1,
        display: "flex",
        flexDirection: "column",
        minWidth: 0,
        minHeight: 0,
      }}
    >
      <ScrollArea style={{ flex: 1, minHeight: 0 }}>
        <Box maw={480} mx="auto" px="1.5rem" py="2rem">
          <Settings />
        </Box>
      </ScrollArea>
    </Box>
  );
}
