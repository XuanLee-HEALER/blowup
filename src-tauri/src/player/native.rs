//! Native OpenGL view bridge for macOS player.
//! Calls into ObjC code compiled from native/metal_layer.m.

use std::ffi::{c_char, c_void};
use std::sync::Mutex;

struct ViewPtr(*mut c_void);
unsafe impl Send for ViewPtr {}
unsafe impl Sync for ViewPtr {}

static GL_VIEW_PTR: Mutex<Option<ViewPtr>> = Mutex::new(None);

#[cfg(target_os = "macos")]
unsafe extern "C" {
    fn blowup_create_gl_view(width: f64, height: f64) -> *mut c_void;
    fn blowup_attach_to_window(ns_window: *mut c_void, view: *mut c_void) -> i32;
    fn blowup_get_gl_proc_address() -> *mut c_void;
    fn blowup_make_gl_context_current(view: *mut c_void);
    fn blowup_request_render(view: *mut c_void);
    fn blowup_remove_view(view: *mut c_void);
}

/// Create a CAOpenGLLayer-backed NSView and attach it below the WKWebView.
/// Returns the view pointer (needed for render callbacks).
#[cfg(target_os = "macos")]
pub fn create_and_attach_gl_view(window: &tauri::WebviewWindow) -> Result<*mut c_void, String> {
    remove_gl_view();

    let ns_window = window.ns_window().map_err(|e| e.to_string())?;
    let size = window.inner_size().map_err(|e| e.to_string())?;

    unsafe {
        let view = blowup_create_gl_view(size.width as f64, size.height as f64);
        if view.is_null() {
            return Err("failed to create GL view".into());
        }

        let ret = blowup_attach_to_window(ns_window, view);
        if ret != 0 {
            blowup_remove_view(view);
            return Err("failed to attach GL view".into());
        }

        *GL_VIEW_PTR.lock().unwrap() = Some(ViewPtr(view));
        tracing::info!("GL view attached below webview");
        Ok(view)
    }
}

/// Make the GL context current on the calling thread.
/// Must be called before mpv_render_context_create.
#[cfg(target_os = "macos")]
pub fn make_gl_context_current() {
    let guard = GL_VIEW_PTR.lock().unwrap();
    if let Some(ViewPtr(view)) = guard.as_ref() {
        unsafe { blowup_make_gl_context_current(*view) };
    }
}

/// Get the OpenGL get_proc_address function pointer for mpv render context.
#[cfg(target_os = "macos")]
pub fn get_gl_proc_address_fn() -> unsafe extern "C" fn(*mut c_void, *const c_char) -> *mut c_void {
    unsafe {
        let ptr = blowup_get_gl_proc_address();
        std::mem::transmute(ptr)
    }
}

/// Notify the GL layer that mpv has a new frame.
#[cfg(target_os = "macos")]
pub fn request_render() {
    let guard = GL_VIEW_PTR.lock().unwrap();
    if let Some(ViewPtr(view)) = guard.as_ref() {
        unsafe {
            blowup_request_render(*view);
        }
    }
}

pub fn remove_gl_view() {
    let ptr = GL_VIEW_PTR.lock().unwrap().take();
    if let Some(ViewPtr(view)) = ptr {
        #[cfg(target_os = "macos")]
        unsafe {
            blowup_remove_view(view);
        }
        tracing::info!("GL view removed");
    }
}

#[cfg(not(target_os = "macos"))]
pub fn create_and_attach_gl_view(_window: &tauri::WebviewWindow) -> Result<*mut c_void, String> {
    Err("embedded player not supported on this platform yet".into())
}

#[cfg(not(target_os = "macos"))]
pub fn get_gl_proc_address_fn() -> unsafe extern "C" fn(*mut c_void, *const c_char) -> *mut c_void {
    panic!("not supported on this platform")
}

#[cfg(not(target_os = "macos"))]
pub fn make_gl_context_current() {}

#[cfg(not(target_os = "macos"))]
pub fn request_render() {}
