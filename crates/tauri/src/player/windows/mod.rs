//! Windows-specific player window lifecycle.
//!
//! Windows cannot reuse the Tauri `WebviewWindowBuilder` for the player
//! window because WebView2's DirectComposition surface cannot z-order
//! against a child OpenGL HWND in the same window. Instead we create a
//! native top-level HWND ourselves (`blowup_create_video_window`) and
//! host the controls in a separate `WebviewWindowBuilder` window with
//! label `player-controls`.
//!
//! See `docs/superpowers/specs/2026-04-12-windows-player-native-window-design.md`.

#![cfg(target_os = "windows")]

use tauri::AppHandle;

pub fn open_player(_app: &AppHandle, _file_path: &str) -> Result<(), String> {
    Err("Windows native player not yet implemented (Phase 2)".into())
}

pub(crate) fn close_player_windows(_app: &AppHandle) {
    // Filled in during Phase 3
}
