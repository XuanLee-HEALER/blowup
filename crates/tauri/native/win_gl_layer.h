#ifndef BLOWUP_WIN_GL_LAYER_H
#define BLOWUP_WIN_GL_LAYER_H

#include <stdint.h>

// Create a GlView struct holding dimensions (HWND created during attach).
// Returns a GlView* (caller-owned).
void* blowup_create_gl_view(double width, double height);

// Create a child HWND inside the given parent HWND, set up WGL OpenGL,
// and position it behind the WebView2 child.
// Returns 0 on success, -1 on failure.
int blowup_attach_to_window(void* parent_hwnd, void* view_ptr);

// Get the OpenGL get_proc_address function for mpv_render_context_create.
// Returns a function pointer: void*(*)(void* ctx, const char* name).
void* blowup_get_gl_proc_address(void);

// Make the WGL context current on the calling thread.
// Must be called before mpv_render_context_create.
void blowup_make_gl_context_current(void* view_ptr);

// Request a redraw — posts a message to the GL window.
// Safe to call from any thread.
void blowup_request_render(void* view_ptr);

// Destroy the GL window and free resources.
void blowup_remove_view(void* view_ptr);

// ---------------------------------------------------------------------------
// Top-level video window (Rust-created, not Tauri-managed)
// ---------------------------------------------------------------------------

// Create a top-level HWND with initial size width x height and return it.
// The window uses WS_POPUP | WS_THICKFRAME | WS_VISIBLE so it has no
// title bar but can still be resized from its edges. Returns NULL on
// failure.
void* blowup_create_video_window(double width, double height);

// Destroy a video window created by blowup_create_video_window. Safe to
// call with NULL.
void blowup_destroy_video_window(void* hwnd);

// Get the window's current screen-space rectangle (position + size).
void blowup_get_video_window_rect(void* hwnd, int* x, int* y, int* w, int* h);

// Move/resize the window. Used when we need to programmatically place it
// (e.g. initial position near screen center).
void blowup_set_video_window_rect(void* hwnd, int x, int y, int w, int h);

// ---------------------------------------------------------------------------
// Fullscreen and window state
// ---------------------------------------------------------------------------

// Enter/leave borderless fullscreen on the monitor containing the window.
// Saves window placement + style before entering so blowup_leave_fullscreen
// can restore them. Returns 0 on success, -1 on failure.
int blowup_enter_fullscreen(void* hwnd);
int blowup_leave_fullscreen(void* hwnd);

// 1 if the window is currently in fullscreen (tracked in C state), 0 otherwise.
int blowup_is_fullscreen(void* hwnd);

// ---------------------------------------------------------------------------
// Window control (called from Tauri commands via FFI)
// ---------------------------------------------------------------------------

void blowup_window_minimize(void* hwnd);
void blowup_window_toggle_maximize(void* hwnd);

// Begin a user-initiated drag — sends WM_NCLBUTTONDOWN + HTCAPTION so
// Windows enters its modal move loop. Rust calls this from the controls
// window's top-strip onMouseDown handler.
void blowup_window_start_drag(void* hwnd);

// Apply Windows 11 rounded corners via DwmSetWindowAttribute. Silent
// no-op on older Windows.
void blowup_apply_round_corners(void* hwnd);

// ---------------------------------------------------------------------------
// Rust → C event forwarding callback (implemented in Rust)
// ---------------------------------------------------------------------------
//
// The C WndProc calls this to hand events up to Rust. The Rust side
// decides what to do (reposition controls, emit Tauri event, invoke
// player command, etc).
//
// event_type values:
//   0 = move     (x, y = new client-area top-left, w, h = new size)
//   1 = size     (x, y = new position, w, h = new size)
//   2 = mousemove  (x, y = client-area cursor, w, h = 0; already 50ms-throttled)
//   3 = dblclick   (x, y = client-area cursor, w, h = 0)
//   4 = close      (all zero)
//   5 = keydown    (x = Win32 virtual-key code, y/w/h = 0)
//   6 = window-state-changed (x = 0 normal, 1 maximized, 2 fullscreen; y/w/h = 0)
extern void blowup_on_video_window_event(int event_type, int x, int y, int w, int h);

#endif
