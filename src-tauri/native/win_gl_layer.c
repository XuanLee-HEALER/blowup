#define WIN32_LEAN_AND_MEAN
#include <windows.h>
#include <commctrl.h>
#include <gl/gl.h>
#include <stdio.h>
#include "win_gl_layer.h"

// ---------------------------------------------------------------------------
// Custom message for render requests (thread-safe via PostMessage)
// ---------------------------------------------------------------------------
#define WM_BLOWUP_RENDER (WM_USER + 1)

// ---------------------------------------------------------------------------
// GlView — holds the child HWND + WGL context
// ---------------------------------------------------------------------------
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

// ---------------------------------------------------------------------------
// Parent-window subclass — resizes GL child to match parent client area
// ---------------------------------------------------------------------------
static LRESULT CALLBACK parent_subclass_proc(
    HWND hwnd, UINT msg, WPARAM wp, LPARAM lp,
    UINT_PTR subclass_id, DWORD_PTR ref_data)
{
    GlView* view = (GlView*)ref_data;

    if (msg == WM_SIZE && view && view->hwnd) {
        RECT rc;
        GetClientRect(hwnd, &rc);
        MoveWindow(view->hwnd, 0, 0, rc.right, rc.bottom, TRUE);
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
            int w = rc.right;
            int h = rc.bottom;

            blowup_render_mpv_frame(0, w, h);
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
            int w = rc.right;
            int h = rc.bottom;

            blowup_render_mpv_frame(0, w, h);
            SwapBuffers(view->hdc);
        }
        EndPaint(hwnd, &ps);
        return 0;
    }

    case WM_ERASEBKGND:
        return 1;   // prevent flicker

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
    wc.hInstance      = GetModuleHandleW(NULL);
    wc.lpszClassName  = WND_CLASS_NAME;
    wc.hCursor        = LoadCursorW(NULL, IDC_ARROW);

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

    GlView* view = (GlView*)calloc(1, sizeof(GlView));
    if (!view) return NULL;

    view->init_width  = width;
    view->init_height = height;

    OutputDebugStringA("blowup: GL view struct created\n");
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

    // Create the GL child window inside the Tauri window
    view->hwnd = CreateWindowExW(
        0,
        WND_CLASS_NAME,
        L"BlowupGL",
        WS_CHILD | WS_VISIBLE,
        0, 0, w, h,
        parent,
        NULL,
        GetModuleHandleW(NULL),
        NULL);

    if (!view->hwnd) {
        OutputDebugStringA("blowup: CreateWindowExW failed\n");
        return -1;
    }

    // Store the view pointer on the HWND for the window proc
    SetWindowLongPtrW(view->hwnd, GWLP_USERDATA, (LONG_PTR)view);

    // Set up WGL OpenGL context
    if (setup_wgl(view) != 0) {
        OutputDebugStringA("blowup: WGL setup failed\n");
        DestroyWindow(view->hwnd);
        view->hwnd = NULL;
        return -1;
    }

    // Position GL window behind all siblings (WebView2 is in front)
    SetWindowPos(view->hwnd, HWND_BOTTOM, 0, 0, 0, 0,
                 SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE);

    // Subclass parent to auto-resize GL child on WM_SIZE
    SetWindowSubclass(parent, parent_subclass_proc, 1, (DWORD_PTR)view);

    {
        char buf[256];
        snprintf(buf, sizeof(buf),
                 "blowup: GL view attached %dx%d, hwnd=%p\n", w, h, (void*)view->hwnd);
        OutputDebugStringA(buf);
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
        OutputDebugStringA("blowup: WGL context made current\n");
    } else {
        OutputDebugStringA("blowup: WARNING — no WGL context\n");
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

    // Remove parent subclass first
    if (view->parent_hwnd) {
        RemoveWindowSubclass(view->parent_hwnd, parent_subclass_proc, 1);
    }

    // Tear down WGL
    if (view->hglrc) {
        wglMakeCurrent(NULL, NULL);
        wglDeleteContext(view->hglrc);
        view->hglrc = NULL;
    }

    // Destroy the child window (HDC released automatically with CS_OWNDC)
    if (view->hwnd) {
        DestroyWindow(view->hwnd);
        view->hwnd = NULL;
    }

    free(view);
    OutputDebugStringA("blowup: GL view removed\n");
}
