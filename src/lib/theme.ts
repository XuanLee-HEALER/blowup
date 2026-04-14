import { createTheme, rem } from "@mantine/core";

/**
 * Application theme — Mantine v9 defaults with a few targeted overrides
 * for desktop density and macOS visual fidelity. We DO NOT define a
 * custom color palette anymore: Mantine's built-in `blue` is the
 * primary, and grays come straight from `--mantine-color-gray-*`.
 *
 * Overrides:
 *
 * - **fontFamily**: native macOS system font stack so SF Pro is picked
 *   up automatically; falls back to system sans on other platforms.
 * - **fontSizes**: scaled down ~1px from Mantine defaults so the body
 *   text reads at 13px (matches a native macOS app).
 * - **spacing**: tightened — `md` 16→12, `lg` 20→16. Mantine's default
 *   scale is comfortable for marketing pages but feels airy in a
 *   dense desktop form.
 * - **defaultRadius**: 0 — flat rectangular controls. We deliberately
 *   avoid mixing rounded and square corners; everything in the main UI
 *   is square. (Independent windows like the floating mini-player keep
 *   their own glass-style radius.)
 * - **components.{Input,TextInput,…}.defaultProps**: every text-like
 *   input defaults to `size="sm"` (~30px tall) and `variant="filled"`
 *   so forms render flat-tinted instead of bordered. Buttons default
 *   to `size="sm"` for the same density reason.
 */
export const theme = createTheme({
  primaryColor: "blue",
  fontFamily:
    '-apple-system, BlinkMacSystemFont, "SF Pro Text", "Helvetica Neue", "PingFang SC", "Noto Sans SC", sans-serif',
  fontFamilyMonospace:
    'ui-monospace, SFMono-Regular, Menlo, Monaco, Consolas, "Liberation Mono", monospace',
  headings: {
    fontFamily:
      '-apple-system, BlinkMacSystemFont, "SF Pro Display", "PingFang SC", sans-serif',
    fontWeight: "600",
  },
  fontSizes: {
    xs: rem(11),
    sm: rem(12),
    md: rem(13),
    lg: rem(15),
    xl: rem(18),
  },
  spacing: {
    xs: rem(8),
    sm: rem(10),
    md: rem(12),
    lg: rem(16),
    xl: rem(24),
  },
  defaultRadius: 0,
  cursorType: "pointer",
  focusRing: "auto",
  components: {
    TextInput: {
      defaultProps: { size: "sm", variant: "filled" },
    },
    PasswordInput: {
      defaultProps: { size: "sm", variant: "filled" },
    },
    NumberInput: {
      defaultProps: { size: "sm", variant: "filled" },
    },
    Select: {
      defaultProps: { size: "sm", variant: "filled" },
    },
    Textarea: {
      defaultProps: { size: "sm", variant: "filled" },
    },
    Button: {
      defaultProps: { size: "sm" },
    },
    Checkbox: {
      defaultProps: { size: "sm" },
    },
    Switch: {
      defaultProps: { size: "sm" },
    },
  },
});
