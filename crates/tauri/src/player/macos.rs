//! macOS-specific player window lifecycle.
//!
//! Wraps the existing `run_open_player_phases` flow that reuses the
//! Tauri `WebviewWindowBuilder` for the player window and attaches a
//! `CAOpenGLLayer` as a subview below the `WKWebView`. See
//! `crates/tauri/native/metal_layer.m` for the ObjC side.
//!
//! Windows uses a different code path entirely — see
//! `crate::player::windows`.

#![cfg(target_os = "macos")]

use super::{
    CURRENT_FILE_PATH, EVENT_LOOP_RUNNING, EVENT_LOOP_SHUTDOWN, MPV_HANDLE, MpvHandlePtr,
    MpvPlayer, PLAYER, RENDER_CTX, RenderCtxPtr, close_player_inner, event_loop, native,
};
use crate::player::ffi::{self, MPV_FORMAT_DOUBLE, Mpv, MpvRenderCtx};
use std::sync::atomic::Ordering;
use tauri::{AppHandle, Manager, WebviewUrl, WebviewWindowBuilder};

pub fn open_player(app: &AppHandle, file_path: &str) -> Result<(), String> {
    close_player_inner(app);

    if let Some(old_window) = app.get_webview_window("player") {
        old_window.close().ok();
    }

    EVENT_LOOP_SHUTDOWN.store(false, Ordering::SeqCst);
    *CURRENT_FILE_PATH.lock().unwrap() = Some(file_path.to_string());

    let app_clone = app.clone();
    let file_path_owned = file_path.to_string();
    tauri::async_runtime::spawn(async move {
        if let Err(e) = run_open_player_phases(app_clone, file_path_owned).await {
            tracing::error!(error = %e, "failed to set up player window");
        }
    });
    Ok(())
}

async fn run_open_player_phases(app: AppHandle, file_path: String) -> Result<(), String> {
    // The player setup is split into TWO short main-thread closures with
    // an async sleep in between, rather than a single long closure that
    // does build + GL + mpv in one go. Why:
    //
    // 1. Windows WebView2 deadlock constraint. On Windows,
    //    `WebviewWindowBuilder::build()` blocks the main thread until
    //    WebView2 finishes initializing, but WebView2 init itself
    //    requires the main thread's message loop to keep pumping. If GL
    //    + mpv setup were bundled into the same main-thread closure as
    //    `.build()`, the main loop couldn't pump messages until all of
    //    that finished — a self-deadlock. Returning from phase 1 as soon
    //    as `build()` returns lets the main loop drain WebView2's
    //    pending init messages; phase 2 then attaches the GL view and
    //    starts mpv on the (again free) main thread. Phase 2 of the
    //    Windows refactor (`crate::player::windows`) keeps the same
    //    split for the same reason.
    //
    // 2. macOS doesn't technically need the split (WKWebView init is not
    //    synchronously blocking like WebView2), but keeping both
    //    platforms on the same phase structure makes the flow easier to
    //    reason about and keeps cross-platform diffs small.
    //
    // `.transparent(true)` (below) is required on BOTH platforms, for
    // different reasons:
    //   - macOS: the CAOpenGLLayer is attached as a subview of the
    //     NSView hosting the WKWebView. NSView compositing only shows
    //     the GL layer underneath the webview if the webview surface is
    //     transparent — otherwise the opaque webview paints over the
    //     video.
    //   - Windows: WebView2's Chromium renderer uses DirectComposition
    //     regardless. With an opaque window, WebView2's composition
    //     output paints a solid rectangle over the Win32 GL child and
    //     the video becomes invisible. With a transparent window +
    //     transparent HTML body, WebView2's composition output is
    //     transparent in the video region, so the GL child (sitting at
    //     HWND_TOP) shows through. The bottom control strip reserved by
    //     `CONTROLS_HEIGHT` in `win_gl_layer.c` is only covered by
    //     WebView2's opaque controls bar, which stays visible.
    //
    // Phase 1: build the player window on the main thread. The closure
    // is kept short (just `.build()`) so the main event loop returns to
    // pumping messages quickly — see the WebView2 note above.
    let window = {
        let (tx, rx) = tokio::sync::oneshot::channel();
        let app_for_build = app.clone();
        app.run_on_main_thread(move || {
            let _ = tx.send(
                WebviewWindowBuilder::new(
                    &app_for_build,
                    "player",
                    WebviewUrl::App("player.html".into()),
                )
                .title("blowup player")
                .inner_size(1280.0, 720.0)
                .min_inner_size(640.0, 360.0)
                .transparent(true)
                .build()
                .map_err(|e| format!("failed to create player window: {e}")),
            );
        })
        .map_err(|e| format!("dispatch phase 1: {e}"))?;
        rx.await
            .map_err(|e| format!("phase 1 channel dropped: {e}"))??
    };

    #[cfg(debug_assertions)]
    {
        let _ = window.run_on_main_thread({
            let w = window.clone();
            move || {
                w.open_devtools();
            }
        });
    }

    // Give WebView2 / WKWebView room to finish initializing before we
    // start hammering the main thread with GL + mpv work. The main
    // thread is free during this await — it's pumping its message loop
    // and dispatching any pending webview init messages.
    tokio::time::sleep(std::time::Duration::from_millis(500)).await;

    // Phase 2: attach GL view, create mpv render context, load file,
    // and spawn the mpv event-loop thread. All GL + Tauri window
    // operations must run on the main thread.
    let (tx, rx) = tokio::sync::oneshot::channel();
    let app_for_setup = app.clone();
    let window_for_setup = window.clone();
    let file_path_for_setup = file_path.clone();
    app.run_on_main_thread(move || {
        let result =
            setup_gl_and_mpv_on_main(&app_for_setup, &window_for_setup, &file_path_for_setup);
        let _ = tx.send(result);
    })
    .map_err(|e| format!("dispatch phase 2: {e}"))?;
    rx.await
        .map_err(|e| format!("phase 2 channel dropped: {e}"))??;

    Ok(())
}

fn setup_gl_and_mpv_on_main(
    app: &AppHandle,
    window: &tauri::WebviewWindow,
    file_path: &str,
) -> Result<(), String> {
    let _gl_view = native::create_and_attach_gl_view(window)?;

    let mpv = Mpv::new()?;
    mpv.set_option("vo", "libmpv")?;
    mpv.set_option("hwdec", "auto")?;
    mpv.set_option("keep-open", "yes")?;
    mpv.initialize()?;

    native::make_gl_context_current();
    let get_proc_addr = native::get_gl_proc_address_fn();
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
    tracing::info!(file_path, "player opened with render API");

    let player = MpvPlayer {
        _render_ctx: render_ctx,
        mpv,
    };
    {
        let mut guard = PLAYER.lock().unwrap();
        *guard = Some(player);
    }

    window.on_window_event(move |event| {
        if let tauri::WindowEvent::Destroyed = event {
            tracing::info!("player window destroyed, cleaning up");
            super::cleanup_player_resources();
        }
    });

    let app_handle = app.clone();
    std::thread::spawn(move || {
        EVENT_LOOP_RUNNING.store(true, Ordering::SeqCst);
        event_loop(&app_handle);
        EVENT_LOOP_RUNNING.store(false, Ordering::SeqCst);
        tracing::info!("event loop thread exited");
    });

    Ok(())
}
