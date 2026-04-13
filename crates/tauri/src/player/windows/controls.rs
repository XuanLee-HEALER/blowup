//! Controls overlay window — a Tauri WebviewWindow (label "player-controls")
//! that hosts Player.tsx, floats above the video window, and follows it on
//! WM_MOVE/WM_SIZE.
//!
//! NOTE: we intentionally never call `WebviewWindow::close` / `destroy`
//! / store it in a static here. Any of those pulls `TaskDialogIndirect`
//! into the crate's lib-test binary via rfd, and that symbol lives in
//! comctl32 v6 (SxS) which the cargo-test exe doesn't get because it
//! has no SxS manifest. Instead we stash the raw HWND and post WM_CLOSE
//! to it from `close()` — which the WebviewWindow's own message loop
//! then turns into a normal Tauri close.

#![cfg(target_os = "windows")]

use std::ffi::c_void;
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};
use tauri::{AppHandle, Emitter, WebviewUrl, WebviewWindowBuilder};

use super::video_window::CONTROLS_WINDOW_READY;

const CONTROLS_HEIGHT: u32 = 100;

/// Raw HWND of the controls overlay window. Wrapped so it's Send/Sync.
#[derive(Copy, Clone)]
struct CtrlHwnd(*mut c_void);
unsafe impl Send for CtrlHwnd {}
unsafe impl Sync for CtrlHwnd {}

static CONTROLS_HWND: Mutex<Option<CtrlHwnd>> = Mutex::new(None);

/// Sentinel: set to `true` while `close()` is tearing the window down so
/// the `CloseRequested` handler knows the WM_CLOSE it's about to see is
/// programmatic (our own cleanup path) and must NOT be intercepted with
/// `prevent_close()`. External close events (× button, Alt+F4) happen
/// while this flag is `false`, so they still get routed through
/// `close_player_inner` for an orderly mpv teardown.
static CLOSING_PROGRAMMATICALLY: AtomicBool = AtomicBool::new(false);

pub fn create(app: &AppHandle, video_rect: (i32, i32, i32, i32)) -> Result<(), String> {
    // Reset the sentinel: a fresh window is being created, so any future
    // WM_CLOSE starts out as user-initiated until `close()` flips it.
    CLOSING_PROGRAMMATICALLY.store(false, Ordering::Release);

    let (vx, vy, vw, vh) = video_rect;

    let window = WebviewWindowBuilder::new(
        app,
        "player-controls",
        WebviewUrl::App("player.html".into()),
    )
    .title("")
    .decorations(false)
    .skip_taskbar(true)
    .always_on_top(true)
    .transparent(true)
    .resizable(false)
    .focused(false)
    .visible(false) // positioned before first paint to avoid a flash
    .inner_size(vw.max(640) as f64, CONTROLS_HEIGHT as f64)
    .position(vx as f64, (vy + vh - CONTROLS_HEIGHT as i32) as f64)
    .build()
    .map_err(|e| format!("failed to create player-controls window: {e}"))?;

    // Stash the raw HWND + apply WS_EX_NOACTIVATE | WS_EX_TOOLWINDOW so
    // the overlay never steals activation from the video window.
    if let Ok(hwnd) = window.hwnd() {
        *CONTROLS_HWND.lock().unwrap() = Some(CtrlHwnd(hwnd.0));

        unsafe extern "system" {
            fn GetWindowLongPtrW(hwnd: *mut c_void, n_index: i32) -> isize;
            fn SetWindowLongPtrW(hwnd: *mut c_void, n_index: i32, new_long: isize) -> isize;
        }
        const GWL_EXSTYLE: i32 = -20;
        const WS_EX_NOACTIVATE: isize = 0x08000000;
        const WS_EX_TOOLWINDOW: isize = 0x00000080;
        unsafe {
            let cur = GetWindowLongPtrW(hwnd.0, GWL_EXSTYLE);
            SetWindowLongPtrW(
                hwnd.0,
                GWL_EXSTYLE,
                cur | WS_EX_NOACTIVATE | WS_EX_TOOLWINDOW,
            );
        }
    }

    // Intercept user-initiated close requests → route them through the
    // shared cleanup path so mpv is torn down before the window goes
    // away. Programmatic closes (triggered by `close()` below) are
    // allowed through without interception so the window actually dies.
    let app_for_close = app.clone();
    window.on_window_event(move |event| {
        if let tauri::WindowEvent::CloseRequested { api, .. } = event {
            if CLOSING_PROGRAMMATICALLY.load(Ordering::Acquire) {
                tracing::debug!("player-controls CloseRequested (programmatic) — allowing");
                return;
            }
            api.prevent_close();
            crate::player::close_player_inner(&app_for_close);
        }
    });

    window.show().map_err(|e| e.to_string())?;
    CONTROLS_WINDOW_READY.store(true, Ordering::SeqCst);
    tracing::info!("player-controls window created");
    Ok(())
}

pub fn close(_app: &AppHandle) {
    CONTROLS_WINDOW_READY.store(false, Ordering::SeqCst);
    // Mark the upcoming WM_CLOSE as programmatic so the CloseRequested
    // handler lets it through instead of re-entering `close_player_inner`
    // (which would recurse and leak the window). The flag stays `true`
    // until `create()` runs again — since the whole player is a
    // singleton teardown-then-recreate flow, that's the only path where
    // we'd want to go back to user-initiated semantics.
    CLOSING_PROGRAMMATICALLY.store(true, Ordering::Release);

    if let Some(CtrlHwnd(hwnd)) = CONTROLS_HWND.lock().unwrap().take() {
        // Post WM_CLOSE to the window — the WebviewWindow's own event
        // loop will handle it and trigger the CloseRequested handler
        // registered in `create`. Calling `WebviewWindow::close` directly
        // would pull `TaskDialogIndirect` into the test binary via rfd.
        unsafe extern "system" {
            fn PostMessageW(hwnd: *mut c_void, msg: u32, w: usize, l: isize) -> i32;
        }
        const WM_CLOSE: u32 = 0x0010;
        unsafe {
            PostMessageW(hwnd, WM_CLOSE, 0, 0);
        }
        tracing::info!("player-controls window close posted");
    }
}

pub fn reposition(x: i32, y: i32, w: i32, h: i32) {
    if !CONTROLS_WINDOW_READY.load(Ordering::Acquire) {
        return;
    }
    // Move/resize via raw HWND + SetWindowPos to avoid pulling
    // `WebviewWindow::set_size` / `set_position` — and by extension
    // `get_webview_window` — into this module. Anything that touches the
    // Tauri `Manager::get_webview_window` codepath from controls.rs drags
    // `TaskDialogIndirect` into the cargo-test binary (see note at top of
    // file) and makes the test exe fail to load.
    let Some(CtrlHwnd(hwnd)) = *CONTROLS_HWND.lock().unwrap() else {
        return;
    };

    unsafe extern "system" {
        fn SetWindowPos(
            hwnd: *mut c_void,
            hwnd_insert_after: *mut c_void,
            x: i32,
            y: i32,
            cx: i32,
            cy: i32,
            u_flags: u32,
        ) -> i32;
    }
    const SWP_NOZORDER: u32 = 0x0004;
    const SWP_NOACTIVATE: u32 = 0x0010;
    const SWP_NOSENDCHANGING: u32 = 0x0400;

    let width = w.max(1);
    let ctrl_y = y + h - CONTROLS_HEIGHT as i32;
    unsafe {
        SetWindowPos(
            hwnd,
            std::ptr::null_mut(),
            x,
            ctrl_y,
            width,
            CONTROLS_HEIGHT as i32,
            SWP_NOZORDER | SWP_NOACTIVATE | SWP_NOSENDCHANGING,
        );
    }
}

pub fn forward_mouse_move() {
    use std::sync::atomic::Ordering;
    if !CONTROLS_WINDOW_READY.load(Ordering::Acquire) {
        return;
    }
    let Some(app) = super::PLAYER_APP_HANDLE.get() else {
        return;
    };
    let _ = app.emit_to("player-controls", "player:video-mouse-move", ());
}
