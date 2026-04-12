#ifndef BLOWUP_NATIVE_LAYER_H
#define BLOWUP_NATIVE_LAYER_H

#include <stdint.h>

// Create a CAOpenGLLayer-backed NSView for mpv render API.
// Returns an NSView* (retained).
void* blowup_create_gl_view(double width, double height);

// Attach the GL view to an NSWindow's contentView below the WKWebView.
// Makes WKWebView transparent and sets dark appearance.
// Returns 0 on success, -1 on failure.
int blowup_attach_to_window(void* ns_window_ptr, void* view_ptr);

// Get the CGL context from the GL view, for mpv_render_context_create.
// Returns a CGLContextObj pointer, or NULL.
void* blowup_get_cgl_context(void* view_ptr);

// Get the OpenGL get_proc_address function, for mpv_render_context_create.
// Returns a function pointer: void*(*)(void* ctx, const char* name).
void* blowup_get_gl_proc_address(void);

// Notify the GL layer that mpv has a new frame to render.
// Call this from mpv's update callback.
void blowup_request_render(void* view_ptr);

// Make the GL view's CGL context current on the calling thread.
// Must be called before mpv_render_context_create.
void blowup_make_gl_context_current(void* view_ptr);

// Remove a view from its superview and release it.
void blowup_remove_view(void* view_ptr);

#endif
