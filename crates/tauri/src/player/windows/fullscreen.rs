//! Fullscreen state machine for the Windows native video window.
//!
//! The actual style/placement manipulation lives in C (see
//! blowup_enter_fullscreen / blowup_leave_fullscreen). Rust owns the
//! logical state enum and the toggle decision, plus the broadcast to
//! the frontend.

#![cfg(target_os = "windows")]

use serde::Serialize;
use std::sync::Mutex;
use tauri::Emitter;

use super::PLAYER_APP_HANDLE;
use super::video_window::{HwndPtr, PLAYER_HWND};

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum VideoWindowState {
    Normal,
    Maximized,
    Fullscreen,
}

impl VideoWindowState {
    pub fn from_code(code: i32) -> Self {
        match code {
            1 => Self::Maximized,
            2 => Self::Fullscreen,
            _ => Self::Normal,
        }
    }
    pub fn to_code(self) -> i32 {
        match self {
            Self::Normal => 0,
            Self::Maximized => 1,
            Self::Fullscreen => 2,
        }
    }
}

static LOGICAL_STATE: Mutex<VideoWindowState> = Mutex::new(VideoWindowState::Normal);

#[derive(Serialize, Clone)]
struct WindowStatePayload {
    state: i32,
}

pub fn toggle() {
    let hwnd_opt = PLAYER_HWND.lock().unwrap().map(|HwndPtr(p)| p);
    let Some(hwnd) = hwnd_opt else { return };

    let is_fs = unsafe { super::video_window::blowup_is_fullscreen(hwnd) } != 0;
    let ret = unsafe {
        if is_fs {
            super::video_window::blowup_leave_fullscreen(hwnd)
        } else {
            super::video_window::blowup_enter_fullscreen(hwnd)
        }
    };
    if ret != 0 {
        tracing::warn!(is_fs, "fullscreen toggle failed");
    }
    // The C layer broadcasts WM_SIZE-driven window-state-changed (event 6)
    // afterwards; we don't need to emit here directly.
}

/// Called from the C → Rust event callback with the new state code.
pub fn on_state_changed(code: i32) {
    let state = VideoWindowState::from_code(code);
    *LOGICAL_STATE.lock().unwrap() = state;

    if let Some(app) = PLAYER_APP_HANDLE.get() {
        let _ = app.emit_to(
            "player-controls",
            "player:window-state",
            WindowStatePayload {
                state: state.to_code(),
            },
        );
    }
    tracing::info!(?state, "broadcast window-state");
}

pub fn current_state() -> VideoWindowState {
    *LOGICAL_STATE.lock().unwrap()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn state_code_roundtrip() {
        for s in [
            VideoWindowState::Normal,
            VideoWindowState::Maximized,
            VideoWindowState::Fullscreen,
        ] {
            assert_eq!(VideoWindowState::from_code(s.to_code()), s);
        }
    }

    #[test]
    fn from_code_defaults_to_normal() {
        assert_eq!(VideoWindowState::from_code(99), VideoWindowState::Normal);
        assert_eq!(VideoWindowState::from_code(-1), VideoWindowState::Normal);
    }

    #[test]
    fn from_code_known_values() {
        assert_eq!(VideoWindowState::from_code(0), VideoWindowState::Normal);
        assert_eq!(VideoWindowState::from_code(1), VideoWindowState::Maximized);
        assert_eq!(VideoWindowState::from_code(2), VideoWindowState::Fullscreen);
    }
}
