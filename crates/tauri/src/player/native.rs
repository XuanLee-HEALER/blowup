//! Native OpenGL view bridge for the embedded player.
//!
//! macOS: calls into ObjC code compiled from native/metal_layer.m
//! Windows: calls into C code compiled from native/win_gl_layer.c

use parking_lot::Mutex;
use std::ffi::{c_char, c_void};

struct ViewPtr(*mut c_void);
unsafe impl Send for ViewPtr {}
unsafe impl Sync for ViewPtr {}

static GL_VIEW_PTR: Mutex<Option<ViewPtr>> = parking_lot::const_mutex(None);

// ---------------------------------------------------------------------------
// macOS FFI
// ---------------------------------------------------------------------------

#[cfg(target_os = "macos")]
unsafe extern "C" {
    fn blowup_create_gl_view(width: f64, height: f64) -> *mut c_void;
    fn blowup_attach_to_window(ns_window: *mut c_void, view: *mut c_void) -> i32;
    fn blowup_get_gl_proc_address() -> *mut c_void;
    fn blowup_make_gl_context_current(view: *mut c_void);
    fn blowup_request_render(view: *mut c_void);
    fn blowup_remove_view(view: *mut c_void);
}

// ---------------------------------------------------------------------------
// Windows FFI
// ---------------------------------------------------------------------------

#[cfg(target_os = "windows")]
unsafe extern "C" {
    fn blowup_create_gl_view(width: f64, height: f64) -> *mut c_void;
    fn blowup_attach_to_window(parent_hwnd: *mut c_void, view: *mut c_void) -> i32;
    fn blowup_get_gl_proc_address() -> *mut c_void;
    fn blowup_make_gl_context_current(view: *mut c_void);
    fn blowup_request_render(view: *mut c_void);
    fn blowup_remove_view(view: *mut c_void);
}

// ---------------------------------------------------------------------------
// macOS: create + attach using ns_window()
// ---------------------------------------------------------------------------

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

        *GL_VIEW_PTR.lock() = Some(ViewPtr(view));
        tracing::info!("GL view attached below webview");
        Ok(view)
    }
}

// ---------------------------------------------------------------------------
// Windows: create + attach using hwnd()
// ---------------------------------------------------------------------------

#[cfg(target_os = "windows")]
pub fn create_and_attach_gl_view(window: &tauri::WebviewWindow) -> Result<*mut c_void, String> {
    remove_gl_view();

    let hwnd = window.hwnd().map_err(|e| e.to_string())?;
    let size = window.inner_size().map_err(|e| e.to_string())?;

    unsafe {
        let view = blowup_create_gl_view(size.width as f64, size.height as f64);
        if view.is_null() {
            return Err("failed to create GL view".into());
        }

        let ret = blowup_attach_to_window(hwnd.0, view);
        if ret != 0 {
            blowup_remove_view(view);
            return Err("failed to attach GL view".into());
        }

        *GL_VIEW_PTR.lock() = Some(ViewPtr(view));
        tracing::info!("GL view attached");
        Ok(view)
    }
}

// ---------------------------------------------------------------------------
// Shared implementations (macOS + Windows)
// ---------------------------------------------------------------------------

#[cfg(any(target_os = "macos", target_os = "windows"))]
pub fn make_gl_context_current() {
    let guard = GL_VIEW_PTR.lock();
    if let Some(ViewPtr(view)) = guard.as_ref() {
        unsafe { blowup_make_gl_context_current(*view) };
    }
}

#[cfg(any(target_os = "macos", target_os = "windows"))]
pub fn get_gl_proc_address_fn() -> unsafe extern "C" fn(*mut c_void, *const c_char) -> *mut c_void {
    unsafe {
        let ptr = blowup_get_gl_proc_address();
        std::mem::transmute(ptr)
    }
}

#[cfg(any(target_os = "macos", target_os = "windows"))]
pub fn request_render() {
    let guard = GL_VIEW_PTR.lock();
    if let Some(ViewPtr(view)) = guard.as_ref() {
        unsafe {
            blowup_request_render(*view);
        }
    }
}

pub fn remove_gl_view() {
    let ptr = GL_VIEW_PTR.lock().take();
    if let Some(ViewPtr(view)) = ptr {
        #[cfg(any(target_os = "macos", target_os = "windows"))]
        unsafe {
            blowup_remove_view(view);
        }
        tracing::info!("GL view removed");
    }
}

// ---------------------------------------------------------------------------
// Fallback stubs for unsupported platforms (Linux, etc.)
// ---------------------------------------------------------------------------

#[cfg(not(any(target_os = "macos", target_os = "windows")))]
pub fn create_and_attach_gl_view(_window: &tauri::WebviewWindow) -> Result<*mut c_void, String> {
    Err("embedded player not supported on this platform yet".into())
}

#[cfg(not(any(target_os = "macos", target_os = "windows")))]
pub fn get_gl_proc_address_fn() -> unsafe extern "C" fn(*mut c_void, *const c_char) -> *mut c_void {
    panic!("not supported on this platform")
}

#[cfg(not(any(target_os = "macos", target_os = "windows")))]
pub fn make_gl_context_current() {}

#[cfg(not(any(target_os = "macos", target_os = "windows")))]
pub fn request_render() {}
