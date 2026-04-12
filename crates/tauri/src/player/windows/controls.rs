//! Controls overlay window — a Tauri WebviewWindow (label "player-controls")
//! that hosts Player.tsx, floats above the video window, and follows it on
//! WM_MOVE/WM_SIZE.

#![cfg(target_os = "windows")]

// Phase 4 fills in create() / close().
pub fn create(_app: &tauri::AppHandle, _video_rect: (i32, i32, i32, i32)) -> Result<(), String> {
    Ok(())
}

pub fn close(_app: &tauri::AppHandle) {
    // no-op until Phase 4
}

// Phase 5 fills in reposition().
pub fn reposition(_x: i32, _y: i32, _w: i32, _h: i32) {
    // no-op until Phase 5
}

// Phase 10 fills in forward_mouse_move().
pub fn forward_mouse_move() {
    // no-op until Phase 10
}
