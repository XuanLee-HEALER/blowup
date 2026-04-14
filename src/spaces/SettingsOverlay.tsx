import { Box, ScrollArea } from "@mantine/core";
import Settings from "../pages/Settings";

/**
 * Settings is not a "space" — it doesn't occupy a slot in the icon sidebar.
 * Instead it overlays the entire main+context region while the icon sidebar
 * stays visible (settings cog highlighted).
 *
 * The page content itself is the legacy Settings.tsx, centered in a 600px
 * column per docs/blowup-layout-spec.md §218.
 */
export function SettingsOverlay() {
  return (
    <Box style={{ flex: 1, display: "flex", flexDirection: "column", minWidth: 0 }}>
      <ScrollArea style={{ flex: 1 }}>
        <Box maw={600} mx="auto" px="1.5rem" py="2rem">
          <Settings />
        </Box>
      </ScrollArea>
    </Box>
  );
}
