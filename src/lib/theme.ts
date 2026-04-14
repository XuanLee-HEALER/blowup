import { createTheme, type MantineColorsTuple } from "@mantine/core";

// Apple systemBlue 10-step palette.
// #007AFF sits at shade 5 (the solid accent); the lighter shades are used
// for hover / subtle backgrounds, the darker ones for pressed states.
const accent: MantineColorsTuple = [
  "#e7f2ff",
  "#cce0ff",
  "#99c0ff",
  "#66a1ff",
  "#3381ff",
  "#007AFF",
  "#0062cc",
  "#004999",
  "#003166",
  "#001833",
];

// Apple systemRed / systemGreen / systemOrange / systemYellow — only the
// shade-5 anchors matter; the rest are linearly interpolated.
const danger: MantineColorsTuple = [
  "#ffeceb",
  "#ffd1cf",
  "#ffa39f",
  "#ff7570",
  "#ff4740",
  "#FF3B30",
  "#cc2f26",
  "#99231d",
  "#661813",
  "#330c09",
];

const success: MantineColorsTuple = [
  "#ebfaef",
  "#d1f3d9",
  "#a3e7b3",
  "#75db8d",
  "#47cf67",
  "#34C759",
  "#2a9f47",
  "#1f7735",
  "#155024",
  "#0a2812",
];

const warning: MantineColorsTuple = [
  "#fff4e5",
  "#ffe0b8",
  "#ffc17a",
  "#ffa23d",
  "#ff8a0a",
  "#FF9500",
  "#cc7700",
  "#995900",
  "#663c00",
  "#331e00",
];

export const theme = createTheme({
  primaryColor: "accent",
  primaryShade: 5,
  colors: {
    accent,
    danger,
    success,
    warning,
  },
  fontFamily: "var(--font-sans)",
  fontFamilyMonospace:
    'ui-monospace, SFMono-Regular, Menlo, Monaco, Consolas, "Liberation Mono", monospace',
  headings: { fontFamily: "var(--font-sans)" },
  defaultRadius: "md",
  cursorType: "pointer",
  focusRing: "auto",
});
