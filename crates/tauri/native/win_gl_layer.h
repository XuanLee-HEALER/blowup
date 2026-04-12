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

#endif
