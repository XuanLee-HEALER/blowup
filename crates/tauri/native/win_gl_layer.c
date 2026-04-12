#define WIN32_LEAN_AND_MEAN
#include <windows.h>
#include <commctrl.h>
#include <gl/gl.h>
#include <stdio.h>
#include <string.h>
#include "win_gl_layer.h"

// Height in pixels of the WebView2 controls bar at the bottom of the player
// window. The GL video view is sized to (client_width, client_height -
// CONTROLS_HEIGHT) so the bottom strip remains uncovered and the controls
// rendered by WebView2 stay visible and interactive. Player.tsx on Windows
// mirrors this value via a matching CSS rule for its bottom bar.
#define CONTROLS_HEIGHT 100

#define WM_BLOWUP_RENDER (WM_USER + 1)

typedef struct {
    HWND   hwnd;          // child GL window
    HWND   parent_hwnd;   // Tauri top-level window
    HDC    hdc;           // device context (CS_OWNDC — persistent)
    HGLRC  hglrc;         // WGL render context
    double init_width;
    double init_height;
} GlView;

// Declared in Rust — renders mpv frame to the given FBO
extern void blowup_render_mpv_frame(int fbo, int width, int height);

static const wchar_t* WND_CLASS_NAME = L"BlowupGLView";
static ATOM           wnd_class_atom = 0;

// Static storage for the single GL view. Only one player is active at a
// time (enforced by the PLAYER mutex on the Rust side), so a single slot
// suffices and side-steps any CRT heap interop with Rust's allocator.
static GlView g_static_view;

// ---------------------------------------------------------------------------
// Parent-window subclass — keeps the GL child sized to the video area
// (client rect minus the controls bar) on WM_SIZE.
// ---------------------------------------------------------------------------
static LRESULT CALLBACK parent_subclass_proc(
    HWND hwnd, UINT msg, WPARAM wp, LPARAM lp,
    UINT_PTR subclass_id, DWORD_PTR ref_data)
{
    GlView* view = (GlView*)ref_data;

    if (msg == WM_SIZE && view && view->hwnd) {
        RECT rc;
        GetClientRect(hwnd, &rc);
        int video_h = rc.bottom - CONTROLS_HEIGHT;
        if (video_h < 0) video_h = 0;
        MoveWindow(view->hwnd, 0, 0, rc.right, video_h, TRUE);
    }

    if (msg == WM_NCDESTROY) {
        RemoveWindowSubclass(hwnd, parent_subclass_proc, subclass_id);
    }

    return DefSubclassProc(hwnd, msg, wp, lp);
}

// ---------------------------------------------------------------------------
// GL child window proc
// ---------------------------------------------------------------------------
static LRESULT CALLBACK gl_wnd_proc(HWND hwnd, UINT msg, WPARAM wp, LPARAM lp)
{
    GlView* view = (GlView*)GetWindowLongPtrW(hwnd, GWLP_USERDATA);

    switch (msg) {
    case WM_BLOWUP_RENDER: {
        if (view && view->hglrc) {
            wglMakeCurrent(view->hdc, view->hglrc);
            RECT rc;
            GetClientRect(hwnd, &rc);
            blowup_render_mpv_frame(0, rc.right, rc.bottom);
            SwapBuffers(view->hdc);
        }
        return 0;
    }

    case WM_PAINT: {
        PAINTSTRUCT ps;
        BeginPaint(hwnd, &ps);
        if (view && view->hglrc) {
            wglMakeCurrent(view->hdc, view->hglrc);
            RECT rc;
            GetClientRect(hwnd, &rc);
            blowup_render_mpv_frame(0, rc.right, rc.bottom);
            SwapBuffers(view->hdc);
        }
        EndPaint(hwnd, &ps);
        return 0;
    }

    case WM_ERASEBKGND:
        return 1;   // prevent flicker

    case WM_NCHITTEST:
        // Click-through: events that land on the GL view pass through to
        // the parent (Tauri) HWND, which routes them to WebView2. Without
        // this, clicking on the video area would be captured by the GL
        // window instead of reaching the play/pause click handler in
        // Player.tsx.
        return HTTRANSPARENT;

    default:
        return DefWindowProcW(hwnd, msg, wp, lp);
    }
}

// ---------------------------------------------------------------------------
// Register window class (once)
// ---------------------------------------------------------------------------
static void ensure_wnd_class(void)
{
    if (wnd_class_atom) return;

    WNDCLASSEXW wc  = {0};
    wc.cbSize        = sizeof(wc);
    wc.style         = CS_OWNDC;           // persistent HDC
    wc.lpfnWndProc   = gl_wnd_proc;
    wc.hInstance     = GetModuleHandleW(NULL);
    wc.lpszClassName = WND_CLASS_NAME;
    wc.hCursor       = LoadCursorW(NULL, IDC_ARROW);

    wnd_class_atom = RegisterClassExW(&wc);
}

// ---------------------------------------------------------------------------
// Set up WGL pixel format + context on the GL child window
// ---------------------------------------------------------------------------
static int setup_wgl(GlView* view)
{
    view->hdc = GetDC(view->hwnd);  // CS_OWNDC — no ReleaseDC needed
    if (!view->hdc) return -1;

    PIXELFORMATDESCRIPTOR pfd = {0};
    pfd.nSize        = sizeof(pfd);
    pfd.nVersion     = 1;
    pfd.dwFlags      = PFD_DRAW_TO_WINDOW | PFD_SUPPORT_OPENGL | PFD_DOUBLEBUFFER;
    pfd.iPixelType   = PFD_TYPE_RGBA;
    pfd.cColorBits   = 32;
    pfd.cDepthBits   = 0;   // mpv doesn't need depth buffer
    pfd.iLayerType   = PFD_MAIN_PLANE;

    int pf = ChoosePixelFormat(view->hdc, &pfd);
    if (!pf) return -1;
    if (!SetPixelFormat(view->hdc, pf, &pfd)) return -1;

    view->hglrc = wglCreateContext(view->hdc);
    if (!view->hglrc) return -1;

    return 0;
}

// ---------------------------------------------------------------------------
// OpenGL proc-address lookup (wglGetProcAddress + opengl32.dll fallback)
// ---------------------------------------------------------------------------
static void* gl_get_proc_address(void* ctx, const char* name)
{
    (void)ctx;
    // wglGetProcAddress only resolves extension functions.
    // Core GL 1.1 functions need GetProcAddress from opengl32.dll.
    void* addr = (void*)wglGetProcAddress(name);
    if (addr == NULL ||
        addr == (void*)1  || addr == (void*)2 ||
        addr == (void*)3  || addr == (void*)(intptr_t)-1)
    {
        HMODULE gl = GetModuleHandleW(L"opengl32.dll");
        if (gl) {
            addr = (void*)GetProcAddress(gl, name);
        }
    }
    return addr;
}

// ---------------------------------------------------------------------------
// Public C bridge — same signatures as metal_layer.h
// ---------------------------------------------------------------------------

void* blowup_create_gl_view(double width, double height)
{
    ensure_wnd_class();

    GlView* view = &g_static_view;
    memset(view, 0, sizeof(GlView));
    view->init_width  = width;
    view->init_height = height;

    return view;
}

int blowup_attach_to_window(void* parent_hwnd_ptr, void* view_ptr)
{
    if (!parent_hwnd_ptr || !view_ptr) return -1;

    HWND parent = (HWND)parent_hwnd_ptr;
    GlView* view = (GlView*)view_ptr;
    view->parent_hwnd = parent;

    RECT rc;
    GetClientRect(parent, &rc);
    int w = rc.right  > 0 ? rc.right  : (int)view->init_width;
    int h = rc.bottom > 0 ? rc.bottom : (int)view->init_height;

    // Reserve CONTROLS_HEIGHT pixels at the bottom for the WebView2
    // controls bar — the GL view only covers the video area above it.
    int video_h = h - CONTROLS_HEIGHT;
    if (video_h < 0) video_h = 0;

    view->hwnd = CreateWindowExW(
        0,
        WND_CLASS_NAME,
        L"BlowupGL",
        WS_CHILD | WS_VISIBLE,
        0, 0, w, video_h,
        parent,
        NULL,
        GetModuleHandleW(NULL),
        NULL);

    if (!view->hwnd) {
        return -1;
    }

    SetWindowLongPtrW(view->hwnd, GWLP_USERDATA, (LONG_PTR)view);

    if (setup_wgl(view) != 0) {
        DestroyWindow(view->hwnd);
        view->hwnd = NULL;
        return -1;
    }

    // Put the GL view at the top of the z-order so it's visually above
    // the WebView2 render target in the video area. WM_NCHITTEST returns
    // HTTRANSPARENT so mouse events still reach WebView2.
    SetWindowPos(view->hwnd, HWND_TOP, 0, 0, 0, 0,
                 SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE);

    // Subclass parent to auto-resize GL child on WM_SIZE
    SetWindowSubclass(parent, parent_subclass_proc, 1, (DWORD_PTR)view);

    return 0;
}

void* blowup_get_gl_proc_address(void)
{
    return (void*)gl_get_proc_address;
}

void blowup_make_gl_context_current(void* view_ptr)
{
    if (!view_ptr) return;
    GlView* view = (GlView*)view_ptr;
    if (view->hglrc && view->hdc) {
        wglMakeCurrent(view->hdc, view->hglrc);
    }
}

void blowup_request_render(void* view_ptr)
{
    if (!view_ptr) return;
    GlView* view = (GlView*)view_ptr;
    if (view->hwnd) {
        PostMessageW(view->hwnd, WM_BLOWUP_RENDER, 0, 0);
    }
}

void blowup_remove_view(void* view_ptr)
{
    if (!view_ptr) return;
    GlView* view = (GlView*)view_ptr;

    if (view->parent_hwnd) {
        RemoveWindowSubclass(view->parent_hwnd, parent_subclass_proc, 1);
    }

    if (view->hglrc) {
        wglMakeCurrent(NULL, NULL);
        wglDeleteContext(view->hglrc);
        view->hglrc = NULL;
    }

    if (view->hwnd) {
        DestroyWindow(view->hwnd);
        view->hwnd = NULL;
    }

    // Do NOT free(view) — view points at the static g_static_view slot.
    memset(view, 0, sizeof(GlView));
}
