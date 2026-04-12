pub mod commands;
pub mod ffi;
pub mod native;

use ffi::{
    MPV_EVENT_END_FILE, MPV_EVENT_NONE, MPV_EVENT_PROPERTY_CHANGE, MPV_EVENT_SHUTDOWN,
    MPV_FORMAT_DOUBLE, Mpv, MpvRenderCtx,
};
use serde::Serialize;
use std::ffi::c_void;
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};
use tauri::{AppHandle, Emitter, Manager, WebviewUrl, WebviewWindowBuilder};

static PLAYER: Mutex<Option<MpvPlayer>> = Mutex::new(None);

// Stored globally so mpv's update callback can trigger re-render
static RENDER_CTX: Mutex<Option<RenderCtxPtr>> = Mutex::new(None);

// Raw mpv handle for event loop (mpv is thread-safe, no mutex needed for wait_event)
static MPV_HANDLE: Mutex<Option<MpvHandlePtr>> = Mutex::new(None);

// Current file path being played (for playback state checks)
static CURRENT_FILE_PATH: Mutex<Option<String>> = Mutex::new(None);

// Signal for the event loop to exit cleanly
static EVENT_LOOP_SHUTDOWN: AtomicBool = AtomicBool::new(false);
// Indicates event loop thread has exited
static EVENT_LOOP_RUNNING: AtomicBool = AtomicBool::new(false);

struct RenderCtxPtr(*mut ffi::MpvRenderContext);
unsafe impl Send for RenderCtxPtr {}
unsafe impl Sync for RenderCtxPtr {}

struct MpvHandlePtr(*mut ffi::MpvHandle);
unsafe impl Send for MpvHandlePtr {}
unsafe impl Sync for MpvHandlePtr {}

pub struct MpvPlayer {
    // IMPORTANT: _render_ctx MUST be declared before mpv.
    // Rust drops fields in declaration order, and mpv requires
    // mpv_render_context_free() before mpv_terminate_destroy().
    _render_ctx: MpvRenderCtx,
    mpv: Mpv,
}

unsafe impl Send for MpvPlayer {}

#[derive(Debug, Clone, Serialize)]
pub struct TrackInfo {
    pub id: i64,
    pub track_type: String, // "video", "audio", "sub"
    pub title: Option<String>,
    pub lang: Option<String>,
    pub selected: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct PlayerState {
    pub playing: bool,
    pub position: f64,
    pub duration: f64,
    pub volume: f64,
    pub paused: bool,
    pub title: String,
}

impl MpvPlayer {
    fn get_tracks(&self) -> Vec<TrackInfo> {
        let count = self.mpv.get_property_i64("track-list/count").unwrap_or(0);
        let mut tracks = Vec::new();
        for i in 0..count {
            let prefix = format!("track-list/{i}");
            let id = self
                .mpv
                .get_property_i64(&format!("{prefix}/id"))
                .unwrap_or(0);
            let track_type = self
                .mpv
                .get_property_string(&format!("{prefix}/type"))
                .unwrap_or_default();
            let title = self.mpv.get_property_string(&format!("{prefix}/title"));
            let lang = self.mpv.get_property_string(&format!("{prefix}/lang"));
            let selected = self
                .mpv
                .get_property_string(&format!("{prefix}/selected"))
                .is_some_and(|v| v == "yes");
            tracks.push(TrackInfo {
                id,
                track_type,
                title,
                lang,
                selected,
            });
        }
        tracks
    }

    fn get_state(&self) -> PlayerState {
        let pause_str = self.mpv.get_property_string("pause");
        let paused = pause_str.as_deref() == Some("yes");
        PlayerState {
            playing: !paused,
            position: self.mpv.get_property_double("time-pos").unwrap_or(0.0),
            duration: self.mpv.get_property_double("duration").unwrap_or(0.0),
            volume: self.mpv.get_property_double("volume").unwrap_or(100.0),
            paused,
            title: self
                .mpv
                .get_property_string("media-title")
                .unwrap_or_default(),
        }
    }
}

/// mpv calls this when a new frame is ready to render.
static UPDATE_CB_COUNT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

unsafe extern "C" fn on_mpv_render_update(_ctx: *mut c_void) {
    let count = UPDATE_CB_COUNT.fetch_add(1, Ordering::Relaxed);
    if count < 5 || count.is_multiple_of(100) {
        tracing::debug!(count, "on_mpv_render_update callback");
    }
    native::request_render();
}

pub fn open_player(app: &AppHandle, file_path: &str) -> Result<(), String> {
    close_player_inner(app);

    // Ensure previous player window is fully gone
    if let Some(old_window) = app.get_webview_window("player") {
        old_window.close().ok();
    }

    EVENT_LOOP_SHUTDOWN.store(false, Ordering::SeqCst);
    *CURRENT_FILE_PATH.lock().unwrap() = Some(file_path.to_string());

    // The player setup is split into TWO short main-thread closures with an
    // async sleep in between. A single long closure (build + sleep + GL +
    // mpv) deadlocks WebView2 on Windows because `WebviewWindowBuilder::build()`
    // blocks the main thread until WebView2 is fully initialized, but
    // WebView2 init needs the main thread's message loop to keep pumping —
    // which can't happen while the closure is running. By returning from
    // the phase-1 closure immediately after `build()`, the main loop pumps
    // the pending init messages, WebView2 comes up, and phase 2 then
    // attaches the GL view and starts mpv on the (again free) main thread.
    //
    // We use `tauri::async_runtime::spawn` to drive the phase chain so the
    // `tokio::time::sleep` between phases happens on a worker instead of
    // blocking the main thread.
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
    // Phase 1: build the player window on the main thread. The closure is
    // kept short (just `.build()`) so the main event loop returns to
    // pumping messages quickly enough for WebView2 to finish initializing
    // without the deadlock that would happen if GL + mpv setup were
    // bundled into the same closure.
    //
    // `.transparent(true)` is required on both platforms:
    //   - macOS: NSView subview compositing lets CAOpenGLLayer render
    //     behind the transparent WKWebView.
    //   - Windows: WebView2's Chromium renderer uses DirectComposition
    //     regardless. With an opaque window, WebView2's composition
    //     output paints a solid background rectangle over our Win32 GL
    //     child window and the video becomes invisible. With transparent
    //     window + transparent HTML body, WebView2's composition output
    //     is transparent in the video region, so the GL child (at
    //     HWND_TOP) shows the video. The bottom strip reserved by
    //     `CONTROLS_HEIGHT` in `win_gl_layer.c` is only covered by
    //     WebView2's opaque controls bar, which stays visible.
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

    // Give WebView2 room to finish initializing before we start hammering
    // the main thread with GL + mpv work. The main thread is free during
    // this await — it's pumping its message loop and dispatching WebView2
    // init messages.
    tokio::time::sleep(std::time::Duration::from_millis(500)).await;

    // Phase 2: attach GL view, create mpv render context, load file, and
    // spawn the mpv event-loop thread. All GL + Tauri window operations
    // must run on the main thread.
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

    // Create mpv instance (no window — render API mode)
    let mpv = Mpv::new()?;
    mpv.set_option("vo", "libmpv")?;
    mpv.set_option("hwdec", "auto")?;
    mpv.set_option("keep-open", "yes")?;
    mpv.initialize()?;

    // Make GL context current, then create render context
    native::make_gl_context_current();
    let get_proc_addr = native::get_gl_proc_address_fn();
    let render_ctx_raw = mpv.create_render_context(get_proc_addr)?;
    let render_ctx = MpvRenderCtx {
        ctx: render_ctx_raw,
    };

    // Store render context globally for the ObjC / Win32 draw callback
    *RENDER_CTX.lock().unwrap() = Some(RenderCtxPtr(render_ctx_raw));
    // Store raw mpv handle for event loop (mpv is thread-safe for wait_event)
    *MPV_HANDLE.lock().unwrap() = Some(MpvHandlePtr(mpv.raw_handle()));

    // Safety: null context, callback is a static function
    unsafe { render_ctx.set_update_callback(Some(on_mpv_render_update), std::ptr::null_mut()) };

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

    // When the user closes the window, clean up player resources
    window.on_window_event(move |event| {
        if let tauri::WindowEvent::Destroyed = event {
            tracing::info!("player window destroyed, cleaning up");
            cleanup_player_resources();
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

/// Called from ObjC's drawInCGLContext — renders mpv frame to current GL context.
/// Exported as C function so ObjC can call it.
static RENDER_FRAME_COUNT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

#[unsafe(no_mangle)]
pub extern "C" fn blowup_render_mpv_frame(fbo: i32, width: i32, height: i32) {
    let count = RENDER_FRAME_COUNT.fetch_add(1, Ordering::Relaxed);
    if count < 5 || count.is_multiple_of(100) {
        tracing::debug!(fbo, width, height, count, "blowup_render_mpv_frame called");
    }

    let guard = RENDER_CTX.lock().unwrap();
    if let Some(RenderCtxPtr(ctx)) = guard.as_ref() {
        let render_ctx = MpvRenderCtx { ctx: *ctx };
        render_ctx.render(fbo, width, height);
        render_ctx.report_swap();
        std::mem::forget(render_ctx);
    } else if count < 5 {
        tracing::warn!("blowup_render_mpv_frame: no render context");
    }
}

/// Push-model event loop: mpv pushes property changes, we emit to frontend.
///
/// Uses raw mpv handle for wait_event (mpv is thread-safe) so we don't
/// hold the PLAYER mutex during the blocking wait. Commands can execute
/// concurrently without being blocked by the event loop.
fn event_loop(app: &AppHandle) {
    // Grab raw handle once — valid until cleanup destroys mpv,
    // but cleanup waits for us to exit first.
    let mpv_raw = {
        let guard = MPV_HANDLE.lock().unwrap();
        match guard.as_ref() {
            Some(MpvHandlePtr(h)) => *h,
            None => return,
        }
    };

    static EMIT_COUNT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

    loop {
        if EVENT_LOOP_SHUTDOWN.load(Ordering::SeqCst) {
            tracing::info!("event loop received shutdown signal");
            break;
        }

        // Block until mpv pushes an event (or timeout after 1s).
        // No mutex held here — commands can run concurrently.
        let (event_id, _event_ptr) = unsafe { ffi::wait_event_raw(mpv_raw, 1.0) };

        // Re-check after waking (cleanup may have called mpv_wakeup)
        if EVENT_LOOP_SHUTDOWN.load(Ordering::SeqCst) {
            tracing::info!("event loop woke up to shutdown");
            break;
        }

        match event_id {
            MPV_EVENT_NONE => continue,

            MPV_EVENT_PROPERTY_CHANGE => {
                // mpv pushed a property change — read full state under brief lock
                let state = {
                    let guard = PLAYER.lock().unwrap();
                    let Some(player) = guard.as_ref() else {
                        break;
                    };
                    player.get_state()
                };

                let count = EMIT_COUNT.fetch_add(1, Ordering::Relaxed);
                if let Err(e) = app.emit("player-state", &state) {
                    tracing::error!(error = %e, "failed to emit player-state");
                } else if count < 5 || count.is_multiple_of(200) {
                    tracing::debug!(
                        count,
                        playing = state.playing,
                        paused = state.paused,
                        pos = state.position,
                        dur = state.duration,
                        "emitted player-state"
                    );
                }
            }

            MPV_EVENT_END_FILE => {
                tracing::info!("mpv end-file event");
                break;
            }

            MPV_EVENT_SHUTDOWN => {
                tracing::info!("mpv shutdown event");
                break;
            }

            other => {
                tracing::debug!(event_id = other, "mpv event (unhandled)");
            }
        }
    }
}

pub fn close_player(app: &AppHandle) -> Result<(), String> {
    close_player_inner(app);
    Ok(())
}

/// Clean up mpv + GL resources without touching the Tauri window.
/// Called from window Destroyed event.
fn cleanup_player_resources() {
    // 1. Signal event loop to stop
    EVENT_LOOP_SHUTDOWN.store(true, Ordering::SeqCst);

    // 2. Interrupt wait_event so event loop exits immediately
    {
        let guard = MPV_HANDLE.lock().unwrap();
        if let Some(MpvHandlePtr(h)) = guard.as_ref() {
            unsafe { ffi::wakeup_raw(*h) };
        }
    }

    // 3. Wait for event loop thread to exit (it will see the shutdown flag)
    for _ in 0..50 {
        if !EVENT_LOOP_RUNNING.load(Ordering::SeqCst) {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }

    // 4. Stop render callbacks (ObjC draw will see None and skip)
    *RENDER_CTX.lock().unwrap() = None;

    // 5. Remove GL view — no more CAOpenGLLayer draw callbacks
    native::remove_gl_view();

    // 6. Clear raw handle (mpv is about to be destroyed)
    *MPV_HANDLE.lock().unwrap() = None;

    // 7. Clear current file path
    *CURRENT_FILE_PATH.lock().unwrap() = None;

    // 8. Destroy mpv (_render_ctx drops first due to field order)
    let mut guard = PLAYER.lock().unwrap();
    if let Some(player) = guard.take() {
        drop(player);
        tracing::info!("player resources cleaned up");
    }
}

pub fn get_current_file_path() -> Option<String> {
    CURRENT_FILE_PATH.lock().unwrap().clone()
}

fn close_player_inner(app: &AppHandle) {
    cleanup_player_resources();

    // Also close the Tauri window if still open
    if let Some(window) = app.get_webview_window("player") {
        window.close().ok();
        tracing::info!("player window closed");
    }
}

pub fn with_player<F, R>(f: F) -> Result<R, String>
where
    F: FnOnce(&MpvPlayer) -> Result<R, String>,
{
    let guard = PLAYER.lock().unwrap();
    let player = guard.as_ref().ok_or("no active player")?;
    f(player)
}
