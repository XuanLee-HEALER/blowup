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
use tauri::{AppHandle, Emitter, Manager, WebviewUrl, WebviewWindowBuilder};

static PLAYER: Mutex<Option<MpvPlayer>> = Mutex::new(None);

// Stored globally so mpv's update callback can trigger re-render
static RENDER_CTX: Mutex<Option<RenderCtxPtr>> = Mutex::new(None);

struct RenderCtxPtr(*mut ffi::MpvRenderContext);
unsafe impl Send for RenderCtxPtr {}
unsafe impl Sync for RenderCtxPtr {}

pub struct MpvPlayer {
    mpv: Mpv,
    _render_ctx: MpvRenderCtx,
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
        // mpv's track-list is a structured property; read count then each track
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
        // pause is FLAG type — read as string ("yes"/"no") for reliability
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
    let count = UPDATE_CB_COUNT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
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
        std::thread::sleep(std::time::Duration::from_millis(200));
    }

    // 1. Create Tauri transparent webview window (controls overlay)
    let window = WebviewWindowBuilder::new(app, "player", WebviewUrl::App("player.html".into()))
        .title("blowup player")
        .inner_size(1280.0, 720.0)
        .min_inner_size(640.0, 360.0)
        .transparent(true)
        .build()
        .map_err(|e| format!("failed to create player window: {e}"))?;

    // Wait for WKWebView to fully attach
    std::thread::sleep(std::time::Duration::from_millis(300));

    // 2. Attach CAOpenGLLayer view below WKWebView
    let _gl_view = native::create_and_attach_gl_view(&window)?;

    // 3. Create mpv instance (no window — render API mode)
    let mpv = Mpv::new()?;
    mpv.set_option("vo", "libmpv")?;
    mpv.set_option("hwdec", "auto")?;
    mpv.set_option("keep-open", "yes")?;
    mpv.initialize()?;

    // 4. Make CGL context current, then create render context
    native::make_gl_context_current();
    tracing::debug!("creating mpv render context...");
    let get_proc_addr = native::get_gl_proc_address_fn();
    let render_ctx_raw = mpv.create_render_context(get_proc_addr)?;
    tracing::info!("mpv render context created successfully");
    let render_ctx = MpvRenderCtx {
        ctx: render_ctx_raw,
    };

    // Store render context globally for the ObjC draw callback
    *RENDER_CTX.lock().unwrap() = Some(RenderCtxPtr(render_ctx_raw));

    // 5. Set update callback — mpv notifies us when a new frame is ready
    // Safety: null context, callback is a static function
    unsafe { render_ctx.set_update_callback(Some(on_mpv_render_update), std::ptr::null_mut()) };

    // 6. Observe properties + load file
    mpv.observe_property("time-pos", MPV_FORMAT_DOUBLE, 1)?;
    mpv.observe_property("duration", MPV_FORMAT_DOUBLE, 2)?;
    mpv.observe_property("pause", ffi::MPV_FORMAT_FLAG, 3)?;
    mpv.observe_property("volume", MPV_FORMAT_DOUBLE, 4)?;

    mpv.command(&["loadfile", file_path])?;

    tracing::info!(file_path, "player opened with render API");

    let player = MpvPlayer {
        mpv,
        _render_ctx: render_ctx,
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
        event_loop(&app_handle);
    });

    Ok(())
}

/// Called from ObjC's drawInCGLContext — renders mpv frame to current GL context.
/// Exported as C function so ObjC can call it.
static RENDER_FRAME_COUNT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

#[unsafe(no_mangle)]
pub extern "C" fn blowup_render_mpv_frame(fbo: i32, width: i32, height: i32) {
    let count = RENDER_FRAME_COUNT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
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

fn event_loop(app: &AppHandle) {
    loop {
        let should_break;
        let state = {
            let guard = PLAYER.lock().unwrap();
            let Some(player) = guard.as_ref() else {
                break;
            };
            let (event_id, _) = player.mpv.wait_event(0.05);

            should_break = event_id == MPV_EVENT_SHUTDOWN || event_id == MPV_EVENT_END_FILE;

            if event_id == MPV_EVENT_PROPERTY_CHANGE || event_id == MPV_EVENT_NONE || should_break {
                // read state
            }

            player.get_state()
        };

        // Emit to the player window specifically
        app.emit_to("player", "player-state", &state).ok();

        if should_break {
            tracing::info!("mpv event loop ending");
            close_player_inner(app);
            break;
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
    // 1. Stop render callbacks
    *RENDER_CTX.lock().unwrap() = None;

    // 2. Brief wait for in-flight render
    std::thread::sleep(std::time::Duration::from_millis(50));

    // 3. Remove GL view
    native::remove_gl_view();

    // 4. Destroy mpv
    let mut guard = PLAYER.lock().unwrap();
    if let Some(player) = guard.take() {
        drop(player);
        tracing::info!("player resources cleaned up");
    }
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
