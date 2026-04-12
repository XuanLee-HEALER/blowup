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

pub mod controls;
pub mod fullscreen;
pub mod video_window;

use std::sync::OnceLock;
use tauri::AppHandle;
use video_window::{HwndPtr, PLAYER_HWND};

/// Cached AppHandle for C → Rust callbacks that need to reach Tauri
/// (reposition controls window, emit events). Set during the first
/// successful `open_player` call.
pub static PLAYER_APP_HANDLE: OnceLock<AppHandle> = OnceLock::new();

pub fn open_player(app: &AppHandle, file_path: &str) -> Result<(), String> {
    super::close_player_inner(app);

    super::EVENT_LOOP_SHUTDOWN.store(false, std::sync::atomic::Ordering::SeqCst);
    *super::CURRENT_FILE_PATH.lock().unwrap() = Some(file_path.to_string());

    let _ = PLAYER_APP_HANDLE.set(app.clone());

    // Phase 2: only create the bare video window. No GL, no mpv, no
    // controls. Subsequent phases add the rest.
    //
    // Phase 2: synchronous main-thread dispatch via std::sync::mpsc.
    // NOTE: must NOT be called from the main thread itself — the
    // blocking rx.recv() would deadlock because the queued closure
    // would never be pumped. Currently safe because Tauri commands
    // run on the async runtime worker pool, never on the main thread.
    // Phase 3 replaces this with tokio::sync::oneshot + async spawn
    // which sidesteps the deadlock entirely.
    let (tx, rx) = std::sync::mpsc::channel();
    let _ = app.run_on_main_thread(move || {
        let hwnd = unsafe { video_window::blowup_create_video_window(1280.0, 720.0) };
        if hwnd.is_null() {
            let _ = tx.send(Err("blowup_create_video_window returned NULL".to_string()));
            return;
        }
        unsafe { video_window::blowup_apply_round_corners(hwnd) };
        *PLAYER_HWND.lock().unwrap() = Some(HwndPtr(hwnd));
        let _ = tx.send(Ok(()));
    });
    rx.recv().map_err(|e| format!("phase 1 channel: {e}"))??;

    tracing::info!(file_path, "[phase 2] video window created (mpv not yet wired)");
    Ok(())
}

pub(crate) fn close_player_windows(app: &AppHandle) {
    controls::close(app);

    if let Some(HwndPtr(hwnd)) = PLAYER_HWND.lock().unwrap().take() {
        unsafe { video_window::blowup_destroy_video_window(hwnd) };
        tracing::info!("video window destroyed");
    }
}

/// Called by the C → Rust callback on WM_CLOSE. Triggers the shared
/// cleanup path so mpv is torn down first, then the window is destroyed.
pub fn on_video_close() {
    if let Some(app) = PLAYER_APP_HANDLE.get() {
        super::close_player_inner(app);
    }
}
