//! Minimal FFI bindings for libmpv C API.
//! Only the functions we actually need are bound here.

use std::ffi::{CStr, CString, c_char, c_int, c_void};
use std::ptr;

// Opaque types
pub enum MpvHandle {}
pub enum MpvRenderContext {}

// Render API param types
pub const MPV_RENDER_PARAM_INVALID: c_int = 0;
pub const MPV_RENDER_PARAM_API_TYPE: c_int = 1;
pub const MPV_RENDER_PARAM_OPENGL_INIT_PARAMS: c_int = 2;
pub const MPV_RENDER_PARAM_OPENGL_FBO: c_int = 3;
pub const MPV_RENDER_PARAM_FLIP_Y: c_int = 4;

#[repr(C)]
pub struct MpvRenderParam {
    pub param_type: c_int,
    pub data: *mut c_void,
}

#[repr(C)]
pub struct MpvOpenGLInitParams {
    pub get_proc_address: Option<unsafe extern "C" fn(*mut c_void, *const c_char) -> *mut c_void>,
    pub get_proc_address_ctx: *mut c_void,
}

#[repr(C)]
pub struct MpvOpenGLFbo {
    pub fbo: c_int,
    pub w: c_int,
    pub h: c_int,
    pub internal_format: c_int,
}

pub type MpvRenderUpdateFn = Option<unsafe extern "C" fn(*mut c_void)>;

// Format constants
pub const MPV_FORMAT_NONE: c_int = 0;
pub const MPV_FORMAT_STRING: c_int = 1;
pub const MPV_FORMAT_DOUBLE: c_int = 5;
pub const MPV_FORMAT_FLAG: c_int = 3;
pub const MPV_FORMAT_INT64: c_int = 4;

// Event IDs
pub const MPV_EVENT_NONE: c_int = 0;
pub const MPV_EVENT_SHUTDOWN: c_int = 1;
pub const MPV_EVENT_LOG_MESSAGE: c_int = 6;
pub const MPV_EVENT_END_FILE: c_int = 7;
pub const MPV_EVENT_FILE_LOADED: c_int = 8;
pub const MPV_EVENT_PROPERTY_CHANGE: c_int = 22;

#[repr(C)]
pub struct MpvEvent {
    pub event_id: c_int,
    pub error: c_int,
    pub reply_userdata: u64,
    pub data: *mut c_void,
}

#[repr(C)]
pub struct MpvEventProperty {
    pub name: *const c_char,
    pub format: c_int,
    pub data: *mut c_void,
}

#[link(name = "mpv")]
unsafe extern "C" {
    fn mpv_create() -> *mut MpvHandle;
    fn mpv_initialize(ctx: *mut MpvHandle) -> c_int;
    fn mpv_terminate_destroy(ctx: *mut MpvHandle);

    fn mpv_set_option_string(
        ctx: *mut MpvHandle,
        name: *const c_char,
        data: *const c_char,
    ) -> c_int;
    fn mpv_set_property_string(
        ctx: *mut MpvHandle,
        name: *const c_char,
        data: *const c_char,
    ) -> c_int;
    fn mpv_set_property(
        ctx: *mut MpvHandle,
        name: *const c_char,
        format: c_int,
        data: *mut c_void,
    ) -> c_int;
    fn mpv_get_property(
        ctx: *mut MpvHandle,
        name: *const c_char,
        format: c_int,
        data: *mut c_void,
    ) -> c_int;
    fn mpv_get_property_string(ctx: *mut MpvHandle, name: *const c_char) -> *mut c_char;
    fn mpv_free(data: *mut c_void);

    fn mpv_command(ctx: *mut MpvHandle, args: *const *const c_char) -> c_int;

    fn mpv_observe_property(
        ctx: *mut MpvHandle,
        reply_userdata: u64,
        name: *const c_char,
        format: c_int,
    ) -> c_int;
    fn mpv_wait_event(ctx: *mut MpvHandle, timeout: f64) -> *mut MpvEvent;
    fn mpv_wakeup(ctx: *mut MpvHandle);

    // Render API
    fn mpv_render_context_create(
        res: *mut *mut MpvRenderContext,
        mpv: *mut MpvHandle,
        params: *mut MpvRenderParam,
    ) -> c_int;
    fn mpv_render_context_render(ctx: *mut MpvRenderContext, params: *mut MpvRenderParam) -> c_int;
    fn mpv_render_context_set_update_callback(
        ctx: *mut MpvRenderContext,
        callback: MpvRenderUpdateFn,
        callback_ctx: *mut c_void,
    );
    fn mpv_render_context_free(ctx: *mut MpvRenderContext);
    fn mpv_render_context_report_swap(ctx: *mut MpvRenderContext);
}

/// Safe wrapper around mpv_handle.
pub struct Mpv {
    ctx: *mut MpvHandle,
}

// mpv is thread-safe per its documentation
unsafe impl Send for Mpv {}
unsafe impl Sync for Mpv {}

impl Mpv {
    /// Create and initialize a new mpv instance.
    pub fn new() -> Result<Self, String> {
        let ctx = unsafe { mpv_create() };
        if ctx.is_null() {
            return Err("mpv_create returned null".into());
        }
        Ok(Self { ctx })
    }

    /// Set an option before initialization.
    pub fn set_option(&self, name: &str, value: &str) -> Result<(), String> {
        let name = CString::new(name).map_err(|e| e.to_string())?;
        let value = CString::new(value).map_err(|e| e.to_string())?;
        let ret = unsafe { mpv_set_option_string(self.ctx, name.as_ptr(), value.as_ptr()) };
        if ret < 0 {
            Err(format!("mpv_set_option_string failed: {ret}"))
        } else {
            Ok(())
        }
    }

    /// Initialize the mpv instance. Must be called after setting options.
    pub fn initialize(&self) -> Result<(), String> {
        let ret = unsafe { mpv_initialize(self.ctx) };
        if ret < 0 {
            Err(format!("mpv_initialize failed: {ret}"))
        } else {
            Ok(())
        }
    }

    /// Set a property (string value).
    pub fn set_property_string(&self, name: &str, value: &str) -> Result<(), String> {
        let name = CString::new(name).map_err(|e| e.to_string())?;
        let value = CString::new(value).map_err(|e| e.to_string())?;
        let ret = unsafe { mpv_set_property_string(self.ctx, name.as_ptr(), value.as_ptr()) };
        if ret < 0 {
            Err(format!("mpv_set_property_string failed: {ret}"))
        } else {
            Ok(())
        }
    }

    /// Set a property (double value).
    pub fn set_property_double(&self, name: &str, value: f64) -> Result<(), String> {
        let name = CString::new(name).map_err(|e| e.to_string())?;
        let mut val = value;
        let ret = unsafe {
            mpv_set_property(
                self.ctx,
                name.as_ptr(),
                MPV_FORMAT_DOUBLE,
                &mut val as *mut f64 as *mut c_void,
            )
        };
        if ret < 0 {
            Err(format!("mpv_set_property double failed: {ret}"))
        } else {
            Ok(())
        }
    }

    /// Set a property (flag/bool).
    pub fn set_property_flag(&self, name: &str, value: bool) -> Result<(), String> {
        let name = CString::new(name).map_err(|e| e.to_string())?;
        let mut val: c_int = if value { 1 } else { 0 };
        let ret = unsafe {
            mpv_set_property(
                self.ctx,
                name.as_ptr(),
                MPV_FORMAT_FLAG,
                &mut val as *mut c_int as *mut c_void,
            )
        };
        if ret < 0 {
            Err(format!("mpv_set_property flag failed: {ret}"))
        } else {
            Ok(())
        }
    }

    /// Get a property as f64.
    pub fn get_property_double(&self, name: &str) -> Option<f64> {
        let name = CString::new(name).ok()?;
        let mut val: f64 = 0.0;
        let ret = unsafe {
            mpv_get_property(
                self.ctx,
                name.as_ptr(),
                MPV_FORMAT_DOUBLE,
                &mut val as *mut f64 as *mut c_void,
            )
        };
        if ret < 0 { None } else { Some(val) }
    }

    /// Get a property as i64.
    pub fn get_property_i64(&self, name: &str) -> Option<i64> {
        let name = CString::new(name).ok()?;
        let mut val: i64 = 0;
        let ret = unsafe {
            mpv_get_property(
                self.ctx,
                name.as_ptr(),
                MPV_FORMAT_INT64,
                &mut val as *mut i64 as *mut c_void,
            )
        };
        if ret < 0 { None } else { Some(val) }
    }

    /// Get a property as String.
    pub fn get_property_string(&self, name: &str) -> Option<String> {
        let name = CString::new(name).ok()?;
        let ptr = unsafe { mpv_get_property_string(self.ctx, name.as_ptr()) };
        if ptr.is_null() {
            return None;
        }
        let s = unsafe { CStr::from_ptr(ptr) }
            .to_string_lossy()
            .into_owned();
        unsafe { mpv_free(ptr as *mut c_void) };
        Some(s)
    }

    /// Execute a command.
    pub fn command(&self, args: &[&str]) -> Result<(), String> {
        let c_args: Vec<CString> = args.iter().map(|s| CString::new(*s).unwrap()).collect();
        let mut ptrs: Vec<*const c_char> = c_args.iter().map(|s| s.as_ptr()).collect();
        ptrs.push(ptr::null());
        let ret = unsafe { mpv_command(self.ctx, ptrs.as_ptr()) };
        if ret < 0 {
            Err(format!("mpv_command failed: {ret}"))
        } else {
            Ok(())
        }
    }

    /// Observe a property for changes.
    pub fn observe_property(&self, name: &str, format: c_int, userdata: u64) -> Result<(), String> {
        let name = CString::new(name).map_err(|e| e.to_string())?;
        let ret = unsafe { mpv_observe_property(self.ctx, userdata, name.as_ptr(), format) };
        if ret < 0 {
            Err(format!("mpv_observe_property failed: {ret}"))
        } else {
            Ok(())
        }
    }

    /// Wait for the next event (blocking up to timeout seconds).
    /// Returns (event_id, event_data_ptr). Pointer is valid until next wait_event call.
    pub fn wait_event(&self, timeout: f64) -> (c_int, *mut MpvEvent) {
        let event = unsafe { mpv_wait_event(self.ctx, timeout) };
        if event.is_null() {
            (MPV_EVENT_NONE, ptr::null_mut())
        } else {
            (unsafe { (*event).event_id }, event)
        }
    }

    /// Read a property change from an event pointer.
    ///
    /// # Safety
    /// Only call this when event_id == MPV_EVENT_PROPERTY_CHANGE and the event
    /// pointer is valid (i.e., from the most recent `wait_event` call).
    pub unsafe fn read_event_property_double(event: *mut MpvEvent) -> Option<(String, f64)> {
        if event.is_null() {
            return None;
        }
        let data = unsafe { (*event).data as *const MpvEventProperty };
        if data.is_null() {
            return None;
        }
        let prop = unsafe { &*data };
        let name = unsafe { CStr::from_ptr(prop.name) }
            .to_string_lossy()
            .into_owned();
        if prop.format == MPV_FORMAT_DOUBLE && !prop.data.is_null() {
            let val = unsafe { *(prop.data as *const f64) };
            Some((name, val))
        } else {
            None
        }
    }

    /// Get the raw mpv_handle pointer (for render context creation).
    pub fn raw_handle(&self) -> *mut MpvHandle {
        self.ctx
    }

    /// Create a render context for OpenGL rendering.
    pub fn create_render_context(
        &self,
        get_proc_address: unsafe extern "C" fn(*mut c_void, *const c_char) -> *mut c_void,
    ) -> Result<*mut MpvRenderContext, String> {
        let api_type = CString::new("opengl").unwrap();
        let mut gl_init = MpvOpenGLInitParams {
            get_proc_address: Some(get_proc_address),
            get_proc_address_ctx: ptr::null_mut(),
        };

        let mut params = [
            MpvRenderParam {
                param_type: MPV_RENDER_PARAM_API_TYPE,
                data: api_type.as_ptr() as *mut c_void,
            },
            MpvRenderParam {
                param_type: MPV_RENDER_PARAM_OPENGL_INIT_PARAMS,
                data: &mut gl_init as *mut MpvOpenGLInitParams as *mut c_void,
            },
            MpvRenderParam {
                param_type: MPV_RENDER_PARAM_INVALID,
                data: ptr::null_mut(),
            },
        ];

        let mut render_ctx: *mut MpvRenderContext = ptr::null_mut();
        let ret =
            unsafe { mpv_render_context_create(&mut render_ctx, self.ctx, params.as_mut_ptr()) };
        if ret < 0 {
            Err(format!("mpv_render_context_create failed: {ret}"))
        } else {
            Ok(render_ctx)
        }
    }
}

/// Thread-safe wait_event using a raw handle pointer.
///
/// # Safety
/// The handle must be a valid mpv_handle that has not been destroyed.
pub unsafe fn wait_event_raw(handle: *mut MpvHandle, timeout: f64) -> (c_int, *mut MpvEvent) {
    let event = unsafe { mpv_wait_event(handle, timeout) };
    if event.is_null() {
        (MPV_EVENT_NONE, ptr::null_mut())
    } else {
        (unsafe { (*event).event_id }, event)
    }
}

/// Interrupt a blocking `mpv_wait_event` call from another thread.
///
/// # Safety
/// The handle must be a valid mpv_handle that has not been destroyed.
pub unsafe fn wakeup_raw(handle: *mut MpvHandle) {
    unsafe { mpv_wakeup(handle) };
}

/// Safe wrapper for the render context.
pub struct MpvRenderCtx {
    pub ctx: *mut MpvRenderContext,
}

unsafe impl Send for MpvRenderCtx {}
unsafe impl Sync for MpvRenderCtx {}

impl MpvRenderCtx {
    /// Render a frame to the given FBO.
    pub fn render(&self, fbo: c_int, width: c_int, height: c_int) {
        let mut fbo_params = MpvOpenGLFbo {
            fbo,
            w: width,
            h: height,
            internal_format: 0,
        };
        let mut flip: c_int = 1;

        let mut params = [
            MpvRenderParam {
                param_type: MPV_RENDER_PARAM_OPENGL_FBO,
                data: &mut fbo_params as *mut MpvOpenGLFbo as *mut c_void,
            },
            MpvRenderParam {
                param_type: MPV_RENDER_PARAM_FLIP_Y,
                data: &mut flip as *mut c_int as *mut c_void,
            },
            MpvRenderParam {
                param_type: MPV_RENDER_PARAM_INVALID,
                data: ptr::null_mut(),
            },
        ];

        unsafe {
            mpv_render_context_render(self.ctx, params.as_mut_ptr());
        }
    }

    /// Set the update callback (called by mpv when new frame is ready).
    ///
    /// # Safety
    /// `ctx` must be valid for the lifetime of the render context, or null.
    pub unsafe fn set_update_callback(&self, callback: MpvRenderUpdateFn, ctx: *mut c_void) {
        unsafe {
            mpv_render_context_set_update_callback(self.ctx, callback, ctx);
        }
    }

    /// Report that a frame was swapped to display.
    pub fn report_swap(&self) {
        unsafe {
            mpv_render_context_report_swap(self.ctx);
        }
    }
}

impl Drop for MpvRenderCtx {
    fn drop(&mut self) {
        if !self.ctx.is_null() {
            unsafe { mpv_render_context_free(self.ctx) };
            self.ctx = ptr::null_mut();
        }
    }
}

impl Drop for Mpv {
    fn drop(&mut self) {
        if !self.ctx.is_null() {
            unsafe { mpv_terminate_destroy(self.ctx) };
            self.ctx = ptr::null_mut();
        }
    }
}
