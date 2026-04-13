//! Video window lifecycle + event dispatch (Win32 side).
//!
//! The C layer (`native/win_gl_layer.c`) creates the HWND, runs its
//! WndProc, and calls `blowup_on_video_window_event` when something
//! happens. Rust decides what to do with each event.

#![cfg(target_os = "windows")]

use std::ffi::c_void;
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};

// ---------------------------------------------------------------------------
// FFI declarations — implemented in native/win_gl_layer.c
// ---------------------------------------------------------------------------

unsafe extern "C" {
    pub fn blowup_create_video_window(width: f64, height: f64) -> *mut c_void;
    pub fn blowup_destroy_video_window(hwnd: *mut c_void);
    pub fn blowup_get_video_window_rect(
        hwnd: *mut c_void,
        x: *mut i32,
        y: *mut i32,
        w: *mut i32,
        h: *mut i32,
    );
    pub fn blowup_set_video_window_rect(hwnd: *mut c_void, x: i32, y: i32, w: i32, h: i32);

    pub fn blowup_enter_fullscreen(hwnd: *mut c_void) -> i32;
    pub fn blowup_leave_fullscreen(hwnd: *mut c_void) -> i32;
    pub fn blowup_is_fullscreen(hwnd: *mut c_void) -> i32;

    pub fn blowup_window_minimize(hwnd: *mut c_void);
    pub fn blowup_window_toggle_maximize(hwnd: *mut c_void);
    pub fn blowup_window_start_drag(hwnd: *mut c_void);
    pub fn blowup_apply_round_corners(hwnd: *mut c_void);
}

// ---------------------------------------------------------------------------
// HWND wrapper — `*mut c_void` isn't Send/Sync, so we wrap it.
// ---------------------------------------------------------------------------

#[derive(Copy, Clone)]
pub struct HwndPtr(pub *mut c_void);
unsafe impl Send for HwndPtr {}
unsafe impl Sync for HwndPtr {}

pub static PLAYER_HWND: Mutex<Option<HwndPtr>> = Mutex::new(None);

/// Set to `true` the first time the controls window finishes building
/// so event handlers can safely call `app.get_webview_window(...)`.
pub static CONTROLS_WINDOW_READY: AtomicBool = AtomicBool::new(false);

// ---------------------------------------------------------------------------
// C → Rust event callback — strong symbol overriding the weak C stub.
//
// event_type:
//   0 = move (x,y = new screen position, w,h = current size)
//   1 = size (x,y = screen position, w,h = new size)
//   2 = mousemove (x,y = client-space cursor)
//   3 = dblclick (x,y = client-space cursor)
//   4 = close
//   5 = keydown (x = Win32 virtual-key code)
//   6 = window-state-changed (x = 0 normal, 1 max, 2 fullscreen)
// ---------------------------------------------------------------------------

#[unsafe(no_mangle)]
pub extern "C" fn blowup_on_video_window_event(event_type: i32, x: i32, y: i32, w: i32, h: i32) {
    match event_type {
        0 | 1 => {
            // move / size — Phase 5 fills in the controls reposition
            tracing::trace!(event_type, x, y, w, h, "video window move/size");
            super::controls::reposition(x, y, w, h);
        }
        2 => {
            // mousemove — Phase 10 forwards to controls window
            tracing::trace!(x, y, "video mousemove");
            super::controls::forward_mouse_move();
        }
        3 => {
            // dblclick — Phase 8 toggles fullscreen
            tracing::info!("video dblclick → toggle fullscreen");
            super::fullscreen::toggle();
        }
        4 => {
            // close — Phase 3 triggers cleanup
            tracing::info!("video window WM_CLOSE");
            super::on_video_close();
        }
        5 => {
            // keydown — Phase 9 dispatches to player commands
            tracing::trace!(vk = x, "video keydown");
            keyboard::dispatch(x);
        }
        6 => {
            // window-state-changed — Phase 8 broadcasts to frontend
            tracing::info!(state = x, "video window state changed");
            super::fullscreen::on_state_changed(x);
        }
        other => {
            tracing::warn!(event_type = other, "unknown video window event");
        }
    }
}

// ---------------------------------------------------------------------------
// Mouse move throttling (Phase 10)
// ---------------------------------------------------------------------------

pub static LAST_MOUSEMOVE_MS: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
const MOUSEMOVE_THROTTLE_MS: u64 = 50;

pub fn should_forward_mouse_move(state: &std::sync::atomic::AtomicU64, now_ms: u64) -> bool {
    let last = state.load(Ordering::Relaxed);
    if now_ms.saturating_sub(last) >= MOUSEMOVE_THROTTLE_MS {
        state.store(now_ms, Ordering::Relaxed);
        true
    } else {
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::AtomicU64;

    #[test]
    fn throttle_allows_first_call() {
        let state = AtomicU64::new(0);
        assert!(should_forward_mouse_move(&state, 100));
    }

    #[test]
    fn throttle_blocks_under_50ms() {
        let state = AtomicU64::new(1000);
        assert!(!should_forward_mouse_move(&state, 1025));
        assert!(!should_forward_mouse_move(&state, 1049));
    }

    #[test]
    fn throttle_allows_at_50ms_boundary() {
        let state = AtomicU64::new(1000);
        assert!(should_forward_mouse_move(&state, 1050));
    }
}

// ---------------------------------------------------------------------------
// Keyboard dispatch — the native video window captures WM_KEYDOWN and
// forwards the VK code here via blowup_on_video_window_event(5, vk, ...).
// We map VK codes to existing player commands.
// ---------------------------------------------------------------------------

pub mod keyboard {
    // Win32 virtual-key constants. Letter keys map to ASCII uppercase values;
    // there is no VK_F or VK_M macro in the Windows SDK.
    const VK_SPACE: i32 = 0x20;
    const VK_LEFT: i32 = 0x25;
    const VK_UP: i32 = 0x26;
    const VK_RIGHT: i32 = 0x27;
    const VK_DOWN: i32 = 0x28;
    const VK_ESCAPE: i32 = 0x1B;
    const VK_F: i32 = 0x46;
    const VK_M: i32 = 0x4D;

    pub fn dispatch(vk: i32) {
        match vk {
            VK_SPACE => {
                let _ = crate::player::commands::cmd_player_play_pause();
            }
            VK_LEFT => {
                let _ = crate::player::commands::cmd_player_seek_relative(-5.0);
            }
            VK_RIGHT => {
                let _ = crate::player::commands::cmd_player_seek_relative(5.0);
            }
            VK_UP => {
                let cur = crate::player::with_player(|p| {
                    Ok(p.mpv.get_property_double("volume").unwrap_or(100.0))
                })
                .unwrap_or(100.0);
                let _ = crate::player::commands::cmd_player_set_volume((cur + 5.0).min(100.0));
            }
            VK_DOWN => {
                let cur = crate::player::with_player(|p| {
                    Ok(p.mpv.get_property_double("volume").unwrap_or(0.0))
                })
                .unwrap_or(0.0);
                let _ = crate::player::commands::cmd_player_set_volume((cur - 5.0).max(0.0));
            }
            VK_F => {
                crate::player::windows::fullscreen::toggle();
            }
            VK_ESCAPE => {
                let hwnd_opt = super::PLAYER_HWND
                    .lock()
                    .unwrap()
                    .map(|super::HwndPtr(p)| p);
                if let Some(hwnd) = hwnd_opt
                    && unsafe { super::blowup_is_fullscreen(hwnd) } != 0
                {
                    unsafe { super::blowup_leave_fullscreen(hwnd) };
                }
            }
            VK_M => {
                let cur = crate::player::with_player(|p| {
                    Ok(p.mpv.get_property_double("volume").unwrap_or(100.0))
                })
                .unwrap_or(100.0);
                let new = if cur > 0.0 { 0.0 } else { 100.0 };
                let _ = crate::player::commands::cmd_player_set_volume(new);
            }
            _ => {}
        }
    }
}
