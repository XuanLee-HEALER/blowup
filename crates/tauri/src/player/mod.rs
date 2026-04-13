pub mod commands;
pub mod ffi;
pub mod macos;
pub mod native;
pub mod windows;

use ffi::{
    MPV_EVENT_END_FILE, MPV_EVENT_NONE, MPV_EVENT_PROPERTY_CHANGE, MPV_EVENT_SHUTDOWN, Mpv,
    MpvRenderCtx,
};
use serde::Serialize;
use std::ffi::c_void;
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};
#[cfg(target_os = "macos")]
use tauri::Manager;
use tauri::{AppHandle, Emitter};

pub(crate) static PLAYER: Mutex<Option<MpvPlayer>> = Mutex::new(None);

// Stored globally so mpv's update callback can trigger re-render
pub(crate) static RENDER_CTX: Mutex<Option<RenderCtxPtr>> = Mutex::new(None);

// Raw mpv handle for event loop (mpv is thread-safe, no mutex needed for wait_event)
pub(crate) static MPV_HANDLE: Mutex<Option<MpvHandlePtr>> = Mutex::new(None);

// Current file path being played (for playback state checks)
pub(crate) static CURRENT_FILE_PATH: Mutex<Option<String>> = Mutex::new(None);

// Signal for the event loop to exit cleanly
pub(crate) static EVENT_LOOP_SHUTDOWN: AtomicBool = AtomicBool::new(false);
// Indicates event loop thread has exited
pub(crate) static EVENT_LOOP_RUNNING: AtomicBool = AtomicBool::new(false);

pub(crate) struct RenderCtxPtr(pub(crate) *mut ffi::MpvRenderContext);
unsafe impl Send for RenderCtxPtr {}
unsafe impl Sync for RenderCtxPtr {}

pub(crate) struct MpvHandlePtr(pub(crate) *mut ffi::MpvHandle);
unsafe impl Send for MpvHandlePtr {}
unsafe impl Sync for MpvHandlePtr {}

pub struct MpvPlayer {
    // IMPORTANT: _render_ctx MUST be declared before mpv.
    // Rust drops fields in declaration order, and mpv requires
    // mpv_render_context_free() before mpv_terminate_destroy().
    pub(crate) _render_ctx: MpvRenderCtx,
    pub(crate) mpv: Mpv,
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
// Consumed by macos.rs today; the Windows path (Phase 2) will consume it too.
#[cfg_attr(target_os = "windows", allow(dead_code))]
static UPDATE_CB_COUNT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

#[cfg_attr(target_os = "windows", allow(dead_code))]
pub(crate) unsafe extern "C" fn on_mpv_render_update(_ctx: *mut c_void) {
    let count = UPDATE_CB_COUNT.fetch_add(1, Ordering::Relaxed);
    if count < 5 || count.is_multiple_of(100) {
        tracing::debug!(count, "on_mpv_render_update callback");
    }
    native::request_render();
}

#[cfg(target_os = "macos")]
pub use macos::open_player;

#[cfg(target_os = "windows")]
pub use windows::open_player;

#[cfg(not(any(target_os = "macos", target_os = "windows")))]
pub fn open_player(_app: &tauri::AppHandle, _file_path: &str) -> Result<(), String> {
    Err("embedded player not supported on this platform yet".into())
}

pub fn close_player(app: &tauri::AppHandle) -> Result<(), String> {
    close_player_inner(app);
    Ok(())
}

pub(crate) fn close_player_inner(app: &tauri::AppHandle) {
    cleanup_player_resources();

    #[cfg(target_os = "macos")]
    if let Some(window) = app.get_webview_window("player") {
        window.close().ok();
        tracing::info!("player window closed");
    }

    #[cfg(target_os = "windows")]
    windows::close_player_windows(app);
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
#[cfg_attr(target_os = "windows", allow(dead_code))]
pub(crate) fn event_loop(app: &AppHandle) {
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

/// Clean up mpv + GL resources without touching the Tauri window.
/// Called from window Destroyed event.
pub(crate) fn cleanup_player_resources() {
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

pub fn with_player<F, R>(f: F) -> Result<R, String>
where
    F: FnOnce(&MpvPlayer) -> Result<R, String>,
{
    let guard = PLAYER.lock().unwrap();
    let player = guard.as_ref().ok_or("no active player")?;
    f(player)
}
