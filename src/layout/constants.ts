/** Layout dimensions referenced by AppLayout / IconSidebar / Toolbar /
 *  ContextPanel / SpaceShell. Keep them named so the spec doc and the
 *  code don't drift. */

/** Icon sidebar width — wide enough to fully contain the macOS
 *  traffic-light cluster (8px left margin + ~58px buttons + ~12px right). */
export const SIDEBAR_WIDTH = 78;

/** Top stripe height. Matches the macOS title-bar overlay so the
 *  toolbar lines up with the sidebar's first space icon. */
export const TOPBAR_HEIGHT = 40;

/** Per-space toolbar height. */
export const TOOLBAR_HEIGHT = 40;

/** Right-side context panel width when expanded. */
export const CONTEXT_PANEL_WIDTH = 320;

/** Below this viewport width the context panel switches to overlay
 *  mode instead of squeezing the main area. */
export const CONTEXT_PANEL_OVERLAY_BREAKPOINT = 850;

/** Hard floor on the main content area so the three-column layout
 *  stays usable at the configured Tauri minWidth. */
export const MAIN_MIN_WIDTH = 480;
