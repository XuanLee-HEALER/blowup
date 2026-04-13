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

/// Dispatch `f` to the main thread and block until it completes.
///
/// Win32 HWND operations (`DestroyWindow`, `SetWindowLongW`, `SetWindowPos`,
/// `ShowWindow`, `SetWindowPlacement`) must run on the thread that created
/// the window (main). Tauri `#[tauri::command]` handlers run on the async
/// runtime worker pool, so all command-driven FFI calls that touch the
/// video HWND must marshal through this helper first.
///
/// Paths that are already on main — the C `video_wnd_proc` callback via
/// `blowup_on_video_window_event` and Tauri's `RunEvent::Exit` — do NOT
/// need this helper and should call FFI directly.
///
/// ⚠️ Must NOT be called from the main thread itself — doing so would
/// deadlock because the queued closure would sit in the main loop's
/// pending-work queue while this function blocks on `recv()`.
pub fn run_on_main_sync<F>(app: &tauri::AppHandle, f: F)
where
    F: FnOnce() + Send + 'static,
{
    let (tx, rx) = std::sync::mpsc::channel();
    let _ = app.run_on_main_thread(move || {
        f();
        let _ = tx.send(());
    });
    let _ = rx.recv();
}

pub fn open_player(app: &AppHandle, file_path: &str) -> Result<(), String> {
    // `OnceLock::set` is thread-safe and `AppHandle` is `Send + Sync`, so
    // caching it here (synchronously, off-main) is fine. Everything else
    // that touches HWND-affine state moves into the async phases below,
    // which marshal to the main thread.
    let _ = PLAYER_APP_HANDLE.set(app.clone());

    let file_path_owned = file_path.to_string();
    let app_clone = app.clone();
    tauri::async_runtime::spawn(async move {
        if let Err(e) = run_open_phases(app_clone, file_path_owned).await {
            tracing::error!(error = %e, "windows open_player failed");
        }
    });
    Ok(())
}

async fn run_open_phases(app: AppHandle, file_path: String) -> Result<(), String> {
    // Phase 0: tear down any previous player on the main thread. Win32
    // HWND operations (`DestroyWindow` etc.) require the creation thread,
    // and `open_player` was invoked from a Tauri command running on the
    // async runtime worker pool — not main.
    let (tx0, rx0) = tokio::sync::oneshot::channel();
    let app_for_cleanup = app.clone();
    app.run_on_main_thread(move || {
        crate::player::close_player_inner(&app_for_cleanup);
        let _ = tx0.send(());
    })
    .map_err(|e| format!("dispatch phase 0: {e}"))?;
    rx0.await.map_err(|e| format!("phase 0 channel: {e}"))?;

    // Reset shared state for the new player AFTER the old cleanup has
    // run. `cleanup_player_resources` sets `EVENT_LOOP_SHUTDOWN = true`
    // and clears `CURRENT_FILE_PATH`, so we re-establish them here.
    super::EVENT_LOOP_SHUTDOWN.store(false, std::sync::atomic::Ordering::SeqCst);
    *super::CURRENT_FILE_PATH.lock().unwrap() = Some(file_path.clone());

    // Phase 1: create HWND (main thread)
    let (tx, rx) = tokio::sync::oneshot::channel();
    app.run_on_main_thread(move || {
        let hwnd = unsafe { video_window::blowup_create_video_window(1280.0, 720.0) };
        if hwnd.is_null() {
            let _ = tx.send(Err("blowup_create_video_window returned NULL".to_string()));
            return;
        }
        unsafe { video_window::blowup_apply_round_corners(hwnd) };
        *PLAYER_HWND.lock().unwrap() = Some(HwndPtr(hwnd));
        let _ = tx.send(Ok(()));
    })
    .map_err(|e| format!("dispatch phase 1: {e}"))?;
    rx.await.map_err(|e| format!("phase 1 channel: {e}"))??;

    // Phase 2: attach GL + mpv (main thread)
    let (tx2, rx2) = tokio::sync::oneshot::channel();
    let file_path_for_setup = file_path.clone();
    let app_for_setup = app.clone();
    app.run_on_main_thread(move || {
        let result = setup_gl_and_mpv_on_video_window(&app_for_setup, &file_path_for_setup);
        let _ = tx2.send(result);
    })
    .map_err(|e| format!("dispatch phase 2: {e}"))?;
    rx2.await.map_err(|e| format!("phase 2 channel: {e}"))??;

    // Phase 3: create the controls overlay window
    let (tx3, rx3) = tokio::sync::oneshot::channel();
    let app_for_ctrl = app.clone();
    app.run_on_main_thread(move || {
        let hwnd_opt = PLAYER_HWND.lock().unwrap().map(|HwndPtr(p)| p);
        let Some(hwnd) = hwnd_opt else {
            let _ = tx3.send(Err("video HWND missing at phase 3".to_string()));
            return;
        };
        let mut vx = 0i32;
        let mut vy = 0i32;
        let mut vw = 0i32;
        let mut vh = 0i32;
        unsafe {
            video_window::blowup_get_video_window_rect(hwnd, &mut vx, &mut vy, &mut vw, &mut vh);
        }
        let result = controls::create(&app_for_ctrl, (vx, vy, vw, vh));
        let _ = tx3.send(result);
    })
    .map_err(|e| format!("dispatch phase 3: {e}"))?;
    // Non-fatal: if controls creation fails we still let the video play.
    match rx3.await.map_err(|e| format!("phase 3 channel: {e}"))? {
        Ok(()) => {}
        Err(e) => tracing::warn!(error = %e, "controls window failed; video runs headless"),
    }

    Ok(())
}

fn setup_gl_and_mpv_on_video_window(app: &AppHandle, file_path: &str) -> Result<(), String> {
    use crate::player::ffi::{self, MPV_FORMAT_DOUBLE, Mpv, MpvRenderCtx};
    use crate::player::{
        EVENT_LOOP_RUNNING, MPV_HANDLE, MpvHandlePtr, MpvPlayer, PLAYER, RENDER_CTX, RenderCtxPtr,
    };
    use std::sync::atomic::Ordering;

    let hwnd = PLAYER_HWND
        .lock()
        .unwrap()
        .map(|HwndPtr(p)| p)
        .ok_or_else(|| "video window missing".to_string())?;

    // Grab the actual client size from the HWND (in case DPI stretched it)
    let (mut w, mut h) = (1280i32, 720i32);
    unsafe {
        video_window::blowup_get_video_window_rect(
            hwnd,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            &mut w,
            &mut h,
        );
    }

    let view =
        unsafe { super::native::create_and_attach_gl_view_win_hwnd(hwnd, w as f64, h as f64)? };
    let _ = view;

    let mpv = Mpv::new()?;
    mpv.set_option("vo", "libmpv")?;
    mpv.set_option("hwdec", "auto")?;
    mpv.set_option("keep-open", "yes")?;
    mpv.initialize()?;

    super::native::make_gl_context_current();
    let get_proc_addr = super::native::get_gl_proc_address_fn();
    let render_ctx_raw = mpv.create_render_context(get_proc_addr)?;
    let render_ctx = MpvRenderCtx {
        ctx: render_ctx_raw,
    };

    *RENDER_CTX.lock().unwrap() = Some(RenderCtxPtr(render_ctx_raw));
    *MPV_HANDLE.lock().unwrap() = Some(MpvHandlePtr(mpv.raw_handle()));

    unsafe {
        render_ctx.set_update_callback(Some(super::on_mpv_render_update), std::ptr::null_mut())
    };

    mpv.observe_property("time-pos", MPV_FORMAT_DOUBLE, 1)?;
    mpv.observe_property("duration", MPV_FORMAT_DOUBLE, 2)?;
    mpv.observe_property("pause", ffi::MPV_FORMAT_FLAG, 3)?;
    mpv.observe_property("volume", MPV_FORMAT_DOUBLE, 4)?;

    mpv.command(&["loadfile", file_path])?;
    tracing::info!(file_path, "windows player: video + mpv ready");

    let player = MpvPlayer {
        _render_ctx: render_ctx,
        mpv,
    };
    *PLAYER.lock().unwrap() = Some(player);

    let app_handle = app.clone();
    std::thread::spawn(move || {
        EVENT_LOOP_RUNNING.store(true, Ordering::SeqCst);
        super::event_loop(&app_handle);
        EVENT_LOOP_RUNNING.store(false, Ordering::SeqCst);
        tracing::info!("event loop thread exited");
    });

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
