import { Box, ScrollArea } from "@mantine/core";
import Settings from "../pages/Settings";

/**
 * Settings is not a "space" — it doesn't occupy a slot in the icon sidebar.
 * Instead it overlays the entire main+context region while the icon sidebar
 * stays visible (settings cog highlighted).
 *
 * The page content is the legacy Settings.tsx, centered in a 600px column
 * per docs/blowup-layout-spec.md §218. Window dragging is handled by the
 * 40px drag stripe at the top of AppLayout's right column, so this view
 * doesn't need its own toolbar.
 *
 * `minHeight: 0` on the root flex item is required so the inner ScrollArea
 * actually constrains its height and scrolls — without it, the flex item
 * defaults to `min-height: auto` (content-driven) and the ScrollArea
 * silently falls back to the page's full height.
 */
export function SettingsOverlay() {
  return (
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
