#define WIN32_LEAN_AND_MEAN
#include <windows.h>
#include <commctrl.h>
#include <dwmapi.h>
#include <shellscalingapi.h>
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
// Video window state (single-instance, mirrors Rust PLAYER mutex)
// ---------------------------------------------------------------------------
typedef struct {
    HWND    hwnd;
    int     is_fullscreen;
    // Saved state for exiting fullscreen
    WINDOWPLACEMENT saved_placement;
    LONG    saved_style;
    LONG    saved_exstyle;
    int     saved_was_maximized;
    // Mouse move throttling
    DWORD   last_mousemove_tick;
} VideoWindow;

static VideoWindow g_video_window;
static const wchar_t* VIDEO_WND_CLASS_NAME = L"BlowupVideoWindow";
static ATOM video_wnd_class_atom = 0;

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
// Video window proc — top-level HWND hosting the GL child + mpv
// ---------------------------------------------------------------------------
static LRESULT CALLBACK video_wnd_proc(HWND hwnd, UINT msg, WPARAM wp, LPARAM lp)
{
    switch (msg) {
    case WM_ERASEBKGND:
        return 1;  // GL child paints the client area; no flicker

    case WM_MOVE: {
        RECT rc;
        GetWindowRect(hwnd, &rc);
        blowup_on_video_window_event(
            0,
            (int)(short)LOWORD(lp),
            (int)(short)HIWORD(lp),
            rc.right - rc.left,
            rc.bottom - rc.top);
        return 0;
    }

    case WM_SIZE: {
        int w = LOWORD(lp);
        int h = HIWORD(lp);
        // Resize the GL child (if attached) to fill the client area.
        if (g_static_view.hwnd) {
            MoveWindow(g_static_view.hwnd, 0, 0, w, h, TRUE);
        }
        RECT rc;
        GetWindowRect(hwnd, &rc);
        blowup_on_video_window_event(1, rc.left, rc.top, w, h);
        return 0;
    }

    case WM_CLOSE:
        blowup_on_video_window_event(4, 0, 0, 0, 0);
        // Don't call DestroyWindow here — Rust calls blowup_destroy_video_window
        // from cleanup_player_resources, which ensures mpv is torn down first.
        return 0;

    case WM_LBUTTONDBLCLK: {
        blowup_on_video_window_event(3, LOWORD(lp), HIWORD(lp), 0, 0);
        return 0;
    }

    case WM_DESTROY:
        return 0;

    default:
        return DefWindowProcW(hwnd, msg, wp, lp);
    }
}

static void ensure_video_wnd_class(void)
{
    if (video_wnd_class_atom) return;

    WNDCLASSEXW wc = {0};
    wc.cbSize        = sizeof(wc);
    wc.style         = CS_HREDRAW | CS_VREDRAW | CS_DBLCLKS;
    wc.lpfnWndProc   = video_wnd_proc;
    wc.hInstance     = GetModuleHandleW(NULL);
    wc.lpszClassName = VIDEO_WND_CLASS_NAME;
    wc.hCursor       = LoadCursorW(NULL, IDC_ARROW);
    wc.hbrBackground = (HBRUSH)GetStockObject(BLACK_BRUSH);

    video_wnd_class_atom = RegisterClassExW(&wc);
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

    int video_h;
    if (parent == g_video_window.hwnd) {
        // New architecture: GL child fills the entire client area of
        // the native video window. The controls live in a separate
        // top-level Tauri window (label "player-controls").
        video_h = h;
    } else {
        // Legacy path (not reachable on Windows anymore, but kept for
        // robustness during transition).
        video_h = h - CONTROLS_HEIGHT;
        if (video_h < 0) video_h = 0;
    }

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

    // If the parent is our own video window, its WndProc already handles
    // WM_SIZE by resizing the GL child; no subclass needed. Only subclass
    // when the parent is a foreign window (legacy Tauri path).
    if (parent != g_video_window.hwnd) {
        SetWindowSubclass(parent, parent_subclass_proc, 1, (DWORD_PTR)view);
    }

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

    if (view->parent_hwnd && view->parent_hwnd != g_video_window.hwnd) {
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

// ---------------------------------------------------------------------------
// Top-level video window (used on Windows instead of a Tauri WebviewWindow)
// ---------------------------------------------------------------------------

void* blowup_create_video_window(double width, double height)
{
    // Enable per-monitor DPI so future SetWindowPos calls use physical
    // pixels. Ignore failures (older Windows).
    typedef BOOL (WINAPI *SetProcessDpiAwarenessContext_t)(DPI_AWARENESS_CONTEXT);
    HMODULE user32 = GetModuleHandleW(L"user32.dll");
    if (user32) {
        SetProcessDpiAwarenessContext_t fn =
            (SetProcessDpiAwarenessContext_t)GetProcAddress(
                user32, "SetProcessDpiAwarenessContext");
        if (fn) {
            fn(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2);
        }
    }

    ensure_video_wnd_class();

    memset(&g_video_window, 0, sizeof(VideoWindow));

    // Centered on primary monitor
    int sx = GetSystemMetrics(SM_CXSCREEN);
    int sy = GetSystemMetrics(SM_CYSCREEN);
    int w  = (int)width;
    int h  = (int)height;
    int x  = (sx - w) / 2;
    int y  = (sy - h) / 2;

    HWND hwnd = CreateWindowExW(
        0,
        VIDEO_WND_CLASS_NAME,
        L"blowup player",
        WS_POPUP | WS_THICKFRAME | WS_VISIBLE | WS_CLIPCHILDREN,
        x, y, w, h,
        NULL, NULL,
        GetModuleHandleW(NULL),
        NULL);

    if (!hwnd) return NULL;

    g_video_window.hwnd = hwnd;
    ShowWindow(hwnd, SW_SHOW);
    UpdateWindow(hwnd);
    return (void*)hwnd;
}

void blowup_destroy_video_window(void* hwnd_ptr)
{
    if (!hwnd_ptr) return;
    HWND hwnd = (HWND)hwnd_ptr;
    if (g_video_window.hwnd == hwnd) {
        memset(&g_video_window, 0, sizeof(VideoWindow));
    }
    DestroyWindow(hwnd);
}

void blowup_get_video_window_rect(void* hwnd_ptr, int* x, int* y, int* w, int* h)
{
    if (!hwnd_ptr) return;
    HWND hwnd = (HWND)hwnd_ptr;
    RECT rc;
    GetWindowRect(hwnd, &rc);
    if (x) *x = rc.left;
    if (y) *y = rc.top;
    if (w) *w = rc.right - rc.left;
    if (h) *h = rc.bottom - rc.top;
}

void blowup_set_video_window_rect(void* hwnd_ptr, int x, int y, int w, int h)
{
    if (!hwnd_ptr) return;
    SetWindowPos((HWND)hwnd_ptr, NULL, x, y, w, h,
                 SWP_NOZORDER | SWP_NOACTIVATE | SWP_FRAMECHANGED);
}

// ---------------------------------------------------------------------------
// Stubs for fullscreen / window control / round corners (Phase 6, 8, 11)
// ---------------------------------------------------------------------------

int blowup_enter_fullscreen(void* hwnd_ptr)
{
    if (!hwnd_ptr) return -1;
    HWND hwnd = (HWND)hwnd_ptr;

    if (g_video_window.is_fullscreen) return 0;

    // Save current placement + style
    g_video_window.saved_placement.length = sizeof(WINDOWPLACEMENT);
    if (!GetWindowPlacement(hwnd, &g_video_window.saved_placement)) return -1;
    g_video_window.saved_style   = GetWindowLongW(hwnd, GWL_STYLE);
    g_video_window.saved_exstyle = GetWindowLongW(hwnd, GWL_EXSTYLE);
    g_video_window.saved_was_maximized = IsZoomed(hwnd) ? 1 : 0;

    // If currently maximized, restore first so the saved_placement we
    // already captured reflects the non-maximized rect (otherwise
    // exiting fullscreen would leap to pre-maximize bounds).
    if (g_video_window.saved_was_maximized) {
        ShowWindow(hwnd, SW_RESTORE);
    }

    // Pick the monitor the window is currently on
    HMONITOR mon = MonitorFromWindow(hwnd, MONITOR_DEFAULTTONEAREST);
    MONITORINFO mi = { sizeof(MONITORINFO) };
    if (!GetMonitorInfoW(mon, &mi)) return -1;

    // Strip caption + thick frame, keep WS_POPUP + WS_VISIBLE
    LONG new_style = (g_video_window.saved_style
        & ~(WS_CAPTION | WS_THICKFRAME | WS_MINIMIZEBOX | WS_MAXIMIZEBOX | WS_SYSMENU))
        | WS_POPUP | WS_VISIBLE;
    SetWindowLongW(hwnd, GWL_STYLE, new_style);

    SetWindowPos(hwnd, HWND_TOP,
                 mi.rcMonitor.left,
                 mi.rcMonitor.top,
                 mi.rcMonitor.right  - mi.rcMonitor.left,
                 mi.rcMonitor.bottom - mi.rcMonitor.top,
                 SWP_FRAMECHANGED | SWP_NOOWNERZORDER);

    g_video_window.is_fullscreen = 1;
    blowup_on_video_window_event(6, 2, 0, 0, 0);  // state = fullscreen
    return 0;
}

int blowup_leave_fullscreen(void* hwnd_ptr)
{
    if (!hwnd_ptr) return -1;
    HWND hwnd = (HWND)hwnd_ptr;
    if (!g_video_window.is_fullscreen) return 0;

    SetWindowLongW(hwnd, GWL_STYLE,   g_video_window.saved_style);
    SetWindowLongW(hwnd, GWL_EXSTYLE, g_video_window.saved_exstyle);
    SetWindowPlacement(hwnd, &g_video_window.saved_placement);
    SetWindowPos(hwnd, NULL, 0, 0, 0, 0,
                 SWP_NOMOVE | SWP_NOSIZE | SWP_NOZORDER | SWP_FRAMECHANGED);

    g_video_window.is_fullscreen = 0;

    // If we entered fullscreen from Maximized, re-maximize
    if (g_video_window.saved_was_maximized) {
        ShowWindow(hwnd, SW_MAXIMIZE);
        blowup_on_video_window_event(6, 1, 0, 0, 0);  // state = maximized
    } else {
        blowup_on_video_window_event(6, 0, 0, 0, 0);  // state = normal
    }
    return 0;
}

int blowup_is_fullscreen(void* hwnd_ptr)
{
    (void)hwnd_ptr;
    return g_video_window.is_fullscreen ? 1 : 0;
}

void blowup_window_minimize(void* hwnd_ptr)
{
    if (!hwnd_ptr) return;
    HWND hwnd = (HWND)hwnd_ptr;
    // Edge case (spec §9 #4): minimizing from fullscreen would leave
    // stale WS_POPUP style on restore. Leave fullscreen first.
    if (g_video_window.is_fullscreen) {
        blowup_leave_fullscreen(hwnd);
    }
    ShowWindow(hwnd, SW_MINIMIZE);
}

void blowup_window_toggle_maximize(void* hwnd_ptr)
{
    if (!hwnd_ptr) return;
    HWND hwnd = (HWND)hwnd_ptr;
    // If we're fullscreen, max button should just return to Normal.
    if (g_video_window.is_fullscreen) {
        blowup_leave_fullscreen(hwnd);
        return;
    }
    if (IsZoomed(hwnd)) {
        ShowWindow(hwnd, SW_RESTORE);
    } else {
        ShowWindow(hwnd, SW_MAXIMIZE);
    }
    // Broadcast state change back to Rust
    int state = IsZoomed(hwnd) ? 1 : 0;
    blowup_on_video_window_event(6, state, 0, 0, 0);
}

void blowup_window_start_drag(void* hwnd_ptr)
{
    if (!hwnd_ptr) return;
    HWND hwnd = (HWND)hwnd_ptr;
    // Release any captured mouse, then tell Windows to start its modal
    // move loop as if the user had grabbed a non-client caption area.
    ReleaseCapture();
    SendMessageW(hwnd, WM_NCLBUTTONDOWN, HTCAPTION, 0);
}

void blowup_apply_round_corners(void* hwnd) { (void)hwnd; }

// Weak default — Rust provides a strong override in
// crate::player::windows (currently windows::mod.rs, Phase 2 moves
// it to crate::player::windows::video_window). This weak symbol
// prevents link errors under GCC/Clang. Under MSVC this is a no-op
// because `__attribute__((weak))` isn't supported; Rust's strong
// definition (added as a temporary stub in Phase 1, real in Phase 2)
// provides the symbol instead.
#if defined(__GNUC__)
__attribute__((weak))
void blowup_on_video_window_event(int event_type, int x, int y, int w, int h) {
    (void)event_type; (void)x; (void)y; (void)w; (void)h;
}
#endif
