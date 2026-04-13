# Windows Player: Native Video Window + Tauri Controls Overlay — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace the Windows `WebviewWindowBuilder`-based player with a pair of top-level windows — a Rust-created native HWND hosting the mpv GL child, and a separate Tauri WebView window acting as a floating controls overlay — so the platform-level DComp vs GDI compositing conflict goes away and the Windows player reaches visual parity with macOS.

**Architecture:** Two independent top-level windows on Windows: `video_window` is a pure Win32 HWND created via `CreateWindowExW`, containing the existing OpenGL child and mpv render context; `player-controls` is a Tauri WebView window (label `player-controls`) reusing `Player.tsx`. The two windows are composed by the OS (DWM) so they never share a compositor. Communication goes through the existing `cmd_player_*` Tauri commands + new C → Rust event callbacks for window move/size/mouse/key events. macOS code path is untouched — all new logic is gated behind `#[cfg(target_os = "windows")]`.

**Tech Stack:** Rust (Tauri v2), C (Win32 + WGL + OpenGL + DWM), TypeScript (React 19), libmpv render API. New link libs: `dwmapi`, `shcore`. No new Rust/Cargo dependencies.

**Spec reference:** `docs/superpowers/specs/2026-04-12-windows-player-native-window-design.md`

**Branch:** `feat/windows-player-native-window` (already created, spec committed as `da2ab1a`)

**Commit cadence:** one commit per phase (§phase 0 through §phase 12). Final merge to `refactor/workspace-core-server` is a separate step after the manual checklist passes.

---

## File Structure

### New files
- `crates/tauri/src/player/macos.rs` — macOS-only `open_player` / `run_open_player_phases` / `setup_gl_and_mpv_on_main`, moved verbatim from `mod.rs`
- `crates/tauri/src/player/windows/mod.rs` — Windows-only `open_player` / `close_player_windows`, `PLAYER_APP_HANDLE`, dispatch table
- `crates/tauri/src/player/windows/video_window.rs` — video HWND FFI declarations, event dispatch, mouse throttle, nested `keyboard` module for WM_KEYDOWN mapping, unit tests for throttle
- `crates/tauri/src/player/windows/controls.rs` — controls `WebviewWindowBuilder`, reposition, close-requested handling, mouse-move forwarding
- `crates/tauri/src/player/windows/fullscreen.rs` — `VideoWindowState` enum, enter/leave fullscreen state machine, unit tests

### Modified files
- `crates/tauri/src/player/mod.rs` — strip macOS specifics, keep shared `MpvPlayer` / globals / `cleanup_player_resources`, add cfg dispatch
- `crates/tauri/src/player/commands.rs` — new window-management commands, reroute `cmd_player_toggle_fullscreen` for Windows
- `crates/tauri/src/lib.rs` — register new commands, call `close_player_inner` in `RunEvent::Exit`
- `crates/tauri/build.rs` — link `dwmapi` + `shcore` on Windows
- `crates/tauri/capabilities/default.json` — add `"player-controls"` to windows list
- `crates/tauri/native/win_gl_layer.c` — extend with top-level window creation, WndProc, fullscreen, event forwarding (~293 → ~600 lines)
- `crates/tauri/native/win_gl_layer.h` — header declarations matching new C functions
- `src/Player.tsx` — drop `IS_WINDOWS` branches, add window control buttons + drag area, listen for new events

### Unchanged
- `crates/tauri/src/player/ffi.rs`
- `crates/tauri/src/player/native.rs`
- `crates/tauri/native/metal_layer.m` / `.h`
- `src/player-main.tsx`
- `player.html`
- `vite.config.ts`

---

## Phase 0: Preparation

Verify capabilities, extract macOS code to its own file so later Windows changes don't touch it, commit the structural refactor. This phase produces zero behavior change.

### Task 0.1: Add `player-controls` to capabilities

**Files:**
- Modify: `crates/tauri/capabilities/default.json:5`

- [ ] **Step 1: Edit the windows list**

Current line 5:
```json
"windows": ["main", "player", "waveform-*", "subtitle-viewer-*"],
```

Change to:
```json
"windows": ["main", "player", "player-controls", "waveform-*", "subtitle-viewer-*"],
```

- [ ] **Step 2: Verify JSON is valid**

Run: `bunx --bun ajv validate -s crates/tauri/gen/schemas/desktop-schema.json -d crates/tauri/capabilities/default.json 2>/dev/null || python -c "import json; json.load(open('crates/tauri/capabilities/default.json'))"`

Expected: exit code 0.

### Task 0.2: Extract macOS player entrypoint to `macos.rs`

**Files:**
- Create: `crates/tauri/src/player/macos.rs`
- Modify: `crates/tauri/src/player/mod.rs` (remove `run_open_player_phases`, `setup_gl_and_mpv_on_main`, keep shared state and `MpvPlayer`)

This is a **pure refactor — no semantic change**. After this task, macOS behavior is identical and compiles as before.

- [ ] **Step 1: Create `macos.rs` with the moved code**

Create `crates/tauri/src/player/macos.rs`:

```rust
//! macOS-specific player window lifecycle.
//!
//! Wraps the existing `run_open_player_phases` flow that reuses the
//! Tauri `WebviewWindowBuilder` for the player window and attaches a
//! `CAOpenGLLayer` as a subview below the `WKWebView`. See
//! `crates/tauri/native/metal_layer.m` for the ObjC side.
//!
//! Windows uses a different code path entirely — see
//! `crate::player::windows`.

#![cfg(target_os = "macos")]

use super::{
    CURRENT_FILE_PATH, EVENT_LOOP_RUNNING, EVENT_LOOP_SHUTDOWN, MPV_HANDLE, MpvHandlePtr,
    MpvPlayer, PLAYER, RENDER_CTX, RenderCtxPtr, close_player_inner, event_loop, native,
};
use crate::player::ffi::{self, MPV_FORMAT_DOUBLE, Mpv, MpvRenderCtx};
use std::sync::atomic::Ordering;
use tauri::{AppHandle, Manager, WebviewUrl, WebviewWindowBuilder};

pub fn open_player(app: &AppHandle, file_path: &str) -> Result<(), String> {
    close_player_inner(app);

    if let Some(old_window) = app.get_webview_window("player") {
        old_window.close().ok();
    }

    EVENT_LOOP_SHUTDOWN.store(false, Ordering::SeqCst);
    *CURRENT_FILE_PATH.lock().unwrap() = Some(file_path.to_string());

    let app_clone = app.clone();
    let file_path_owned = file_path.to_string();
    tauri::async_runtime::spawn(async move {
        if let Err(e) = run_open_player_phases(app_clone, file_path_owned).await {
            tracing::error!(error = %e, "failed to set up player window");
        }
    });
    Ok(())
}

async fn run_open_player_phases(app: AppHandle, file_path: String) -> Result<(), String> {
    let window = {
        let (tx, rx) = tokio::sync::oneshot::channel();
        let app_for_build = app.clone();
        app.run_on_main_thread(move || {
            let _ = tx.send(
                WebviewWindowBuilder::new(
                    &app_for_build,
                    "player",
                    WebviewUrl::App("player.html".into()),
                )
                .title("blowup player")
                .inner_size(1280.0, 720.0)
                .min_inner_size(640.0, 360.0)
                .transparent(true)
                .build()
                .map_err(|e| format!("failed to create player window: {e}")),
            );
        })
        .map_err(|e| format!("dispatch phase 1: {e}"))?;
        rx.await
            .map_err(|e| format!("phase 1 channel dropped: {e}"))??
    };

    #[cfg(debug_assertions)]
    {
        let _ = window.run_on_main_thread({
            let w = window.clone();
            move || {
                w.open_devtools();
            }
        });
    }

    tokio::time::sleep(std::time::Duration::from_millis(500)).await;

    let (tx, rx) = tokio::sync::oneshot::channel();
    let app_for_setup = app.clone();
    let window_for_setup = window.clone();
    let file_path_for_setup = file_path.clone();
    app.run_on_main_thread(move || {
        let result =
            setup_gl_and_mpv_on_main(&app_for_setup, &window_for_setup, &file_path_for_setup);
        let _ = tx.send(result);
    })
    .map_err(|e| format!("dispatch phase 2: {e}"))?;
    rx.await
        .map_err(|e| format!("phase 2 channel dropped: {e}"))??;

    Ok(())
}

fn setup_gl_and_mpv_on_main(
    app: &AppHandle,
    window: &tauri::WebviewWindow,
    file_path: &str,
) -> Result<(), String> {
    let _gl_view = native::create_and_attach_gl_view(window)?;

    let mpv = Mpv::new()?;
    mpv.set_option("vo", "libmpv")?;
    mpv.set_option("hwdec", "auto")?;
    mpv.set_option("keep-open", "yes")?;
    mpv.initialize()?;

    native::make_gl_context_current();
    let get_proc_addr = native::get_gl_proc_address_fn();
    let render_ctx_raw = mpv.create_render_context(get_proc_addr)?;
    let render_ctx = MpvRenderCtx {
        ctx: render_ctx_raw,
    };

    *RENDER_CTX.lock().unwrap() = Some(RenderCtxPtr(render_ctx_raw));
    *MPV_HANDLE.lock().unwrap() = Some(MpvHandlePtr(mpv.raw_handle()));

    unsafe {
        render_ctx.set_update_callback(Some(super::on_mpv_render_update), std::ptr::null_mut())
    };

    mpv.observe_property("time-pos", MPV_FORMAT_DOUBLE, 1)?;
    mpv.observe_property("duration", MPV_FORMAT_DOUBLE, 2)?;
    mpv.observe_property("pause", ffi::MPV_FORMAT_FLAG, 3)?;
    mpv.observe_property("volume", MPV_FORMAT_DOUBLE, 4)?;

    mpv.command(&["loadfile", file_path])?;
    tracing::info!(file_path, "player opened with render API");

    let player = MpvPlayer {
        _render_ctx: render_ctx,
        mpv,
    };
    {
        let mut guard = PLAYER.lock().unwrap();
        *guard = Some(player);
    }

    window.on_window_event(move |event| {
        if let tauri::WindowEvent::Destroyed = event {
            tracing::info!("player window destroyed, cleaning up");
            super::cleanup_player_resources();
        }
    });

    let app_handle = app.clone();
    std::thread::spawn(move || {
        EVENT_LOOP_RUNNING.store(true, Ordering::SeqCst);
        event_loop(&app_handle);
        EVENT_LOOP_RUNNING.store(false, Ordering::SeqCst);
        tracing::info!("event loop thread exited");
    });

    Ok(())
}
```

- [ ] **Step 2: Strip macOS code from `mod.rs` and add dispatch**

Edit `crates/tauri/src/player/mod.rs`:

1. Add `pub mod macos;` and `pub mod windows;` near the top (after the existing `pub mod commands; pub mod ffi; pub mod native;`)
2. Delete the `open_player`, `run_open_player_phases`, `setup_gl_and_mpv_on_main`, and `close_player_inner` function bodies
3. Replace them with dispatch:

```rust
#[cfg(target_os = "macos")]
pub use macos::open_player;

#[cfg(target_os = "windows")]
pub use windows::open_player;

#[cfg(not(any(target_os = "macos", target_os = "windows")))]
pub fn open_player(_app: &tauri::AppHandle, _file_path: &str) -> Result<(), String> {
    Err("embedded player not supported on this platform yet".into())
}

pub fn close_player(app: &tauri::AppHandle) -> Result<(), String> {
    close_player_inner(app);
    Ok(())
}

pub(crate) fn close_player_inner(app: &tauri::AppHandle) {
    cleanup_player_resources();

    #[cfg(target_os = "macos")]
    if let Some(window) = app.get_webview_window("player") {
        window.close().ok();
        tracing::info!("player window closed");
    }

    #[cfg(target_os = "windows")]
    windows::close_player_windows(app);
}
```

4. Keep `MpvPlayer`, `TrackInfo`, `PlayerState`, all the global statics (`PLAYER`, `RENDER_CTX`, `MPV_HANDLE`, `CURRENT_FILE_PATH`, `EVENT_LOOP_SHUTDOWN`, `EVENT_LOOP_RUNNING`, `UPDATE_CB_COUNT`, `RENDER_FRAME_COUNT`), `on_mpv_render_update`, `blowup_render_mpv_frame`, `event_loop`, `cleanup_player_resources`, `get_current_file_path`, `with_player` where they are.
5. Change the visibility of the statics that `macos.rs` / `windows/` need to access — make them `pub(super)` (or keep `pub(crate)` if already referenced elsewhere). Specifically: `PLAYER`, `RENDER_CTX`, `MPV_HANDLE`, `CURRENT_FILE_PATH`, `EVENT_LOOP_SHUTDOWN`, `EVENT_LOOP_RUNNING`, `RenderCtxPtr`, `MpvHandlePtr` must all be reachable from `macos.rs`.
6. Add a stub `pub(crate) fn close_player_windows(_app: &tauri::AppHandle) {}` in `windows/mod.rs` for now — it will be filled in later phases.

- [ ] **Step 3: Create empty `windows/mod.rs` so the module resolves**

Create `crates/tauri/src/player/windows/mod.rs`:

```rust
//! Windows-specific player window lifecycle.
//!
//! Windows cannot reuse the Tauri `WebviewWindowBuilder` for the player
//! window because WebView2's DirectComposition surface cannot z-order
//! against a child OpenGL HWND in the same window. Instead we create a
//! native top-level HWND ourselves (`blowup_create_video_window`) and
//! host the controls in a separate `WebviewWindowBuilder` window with
//! label `player-controls`.
//!
//! See `docs/superpowers/specs/2026-04-12-windows-player-native-window-design.md`.

#![cfg(target_os = "windows")]

use tauri::AppHandle;

pub fn open_player(_app: &AppHandle, _file_path: &str) -> Result<(), String> {
    Err("Windows native player not yet implemented (Phase 2)".into())
}

pub(crate) fn close_player_windows(_app: &AppHandle) {
    // Filled in during Phase 3
}
```

- [ ] **Step 4: Build on current platform**

Run: `cargo build --manifest-path crates/tauri/Cargo.toml`

Expected: clean build (no new warnings, no errors).

On Windows this will fail because `windows::open_player` returns an error — that's intentional, the subsequent phases replace the stub. On macOS it should work exactly as before.

- [ ] **Step 5: Run clippy**

Run: `cargo clippy --manifest-path crates/tauri/Cargo.toml -- -D warnings`

Expected: pass.

### Task 0.3: Commit Phase 0

- [ ] **Step 1: Stage and commit**

Run:
```bash
git add crates/tauri/capabilities/default.json \
        crates/tauri/src/player/mod.rs \
        crates/tauri/src/player/macos.rs \
        crates/tauri/src/player/windows/mod.rs
git commit -m "$(cat <<'MSG'
refactor(player): extract macOS code to macos.rs, add windows/ stub

Pure structural refactor — macOS flow is identical, Windows path
returns a "not yet implemented" error. Capabilities file now allows
the forthcoming "player-controls" window label. Subsequent phases
fill in the Windows native video window + controls overlay.

Co-Authored-By: Claude Opus 4.6 (1M context) <noreply@anthropic.com>
MSG
)"
```

Expected: `1 commit` on `feat/windows-player-native-window`.

---

## Phase 1: C-layer video window skeleton

Add the new C functions for creating and destroying a top-level video HWND, plus the FFI-wired event forwarding callback. At the end of this phase the C side can hand out a bare HWND that renders nothing — no GL, no mpv, just a titleless gray rectangle. Pure Win32, compiles into `native_win_gl` via the existing `cc` build step.

### Task 1.1: Add DWM + shcore link libs

**Files:**
- Modify: `crates/tauri/build.rs:23-35`

- [ ] **Step 1: Edit `compile_native` Windows branch**

Find the `#[cfg(target_os = "windows")]` block in `compile_native` (currently lines 23-35). Add two new link-lib declarations:

```rust
    #[cfg(target_os = "windows")]
    {
        println!("cargo:rerun-if-changed=native/win_gl_layer.c");
        println!("cargo:rerun-if-changed=native/win_gl_layer.h");
        cc::Build::new()
            .file("native/win_gl_layer.c")
            .compile("native_win_gl");

        println!("cargo:rustc-link-lib=opengl32");
        println!("cargo:rustc-link-lib=user32");
        println!("cargo:rustc-link-lib=gdi32");
        println!("cargo:rustc-link-lib=comctl32");
        println!("cargo:rustc-link-lib=dwmapi");
        println!("cargo:rustc-link-lib=shcore");
    }
```

### Task 1.2: Extend `win_gl_layer.h` with new declarations

**Files:**
- Modify: `crates/tauri/native/win_gl_layer.h`

- [ ] **Step 1: Append new declarations**

Add the following to `win_gl_layer.h` before the closing `#endif`:

```c
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
```

### Task 1.3: Implement the video window skeleton in C

**Files:**
- Modify: `crates/tauri/native/win_gl_layer.c`

Scope of this task: declare a new window class for the video window, implement `blowup_create_video_window` / `blowup_destroy_video_window` / `blowup_get_video_window_rect` / `blowup_set_video_window_rect`. The WndProc handles only `WM_MOVE`, `WM_SIZE`, `WM_CLOSE`, `WM_DESTROY`, `WM_ERASEBKGND`, and delegates everything else to `DefWindowProcW`. No GL, no fullscreen, no mouse, no keyboard yet.

- [ ] **Step 1: Add new includes + state near the top**

Near the existing `#include <commctrl.h>` add:

```c
#include <dwmapi.h>
#include <shellscalingapi.h>
```

Just after `static GlView g_static_view;` add a new static for the video window state:

```c
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
```

- [ ] **Step 2: Implement the video window WndProc (skeleton only)**

Add this function above `ensure_wnd_class`:

```c
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
    wc.style         = CS_HREDRAW | CS_VREDRAW;
    wc.lpfnWndProc   = video_wnd_proc;
    wc.hInstance     = GetModuleHandleW(NULL);
    wc.lpszClassName = VIDEO_WND_CLASS_NAME;
    wc.hCursor       = LoadCursorW(NULL, IDC_ARROW);
    wc.hbrBackground = (HBRUSH)GetStockObject(BLACK_BRUSH);

    video_wnd_class_atom = RegisterClassExW(&wc);
}
```

- [ ] **Step 3: Implement create/destroy/get/set**

Add below the existing `blowup_remove_view`:

```c
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
```

- [ ] **Step 4: Add stub implementations for the other declared functions**

The header declares a bunch of functions we'll implement in later phases. To keep the linker happy we need stub bodies now. Add at the bottom of `win_gl_layer.c`:

```c
// ---------------------------------------------------------------------------
// Stubs for fullscreen / window control / round corners (Phase 6, 8, 11)
// ---------------------------------------------------------------------------

int  blowup_enter_fullscreen(void* hwnd) { (void)hwnd; return -1; }
int  blowup_leave_fullscreen(void* hwnd) { (void)hwnd; return -1; }
int  blowup_is_fullscreen(void* hwnd)    { (void)hwnd; return 0; }
void blowup_window_minimize(void* hwnd)  { (void)hwnd; }
void blowup_window_toggle_maximize(void* hwnd) { (void)hwnd; }
void blowup_window_start_drag(void* hwnd) { (void)hwnd; }
void blowup_apply_round_corners(void* hwnd) { (void)hwnd; }
```

- [ ] **Step 5: Provide a weak default for `blowup_on_video_window_event`**

Since the Rust implementation is in a later phase and we want the C layer to compile and link standalone against stubs, add a weak default at the bottom of `win_gl_layer.c`:

```c
// Weak default — Rust provides a strong override in
// crate::player::windows::video_window::blowup_on_video_window_event.
// This weak symbol prevents link errors if the Rust side is missing
// during an incremental rebuild.
#if defined(__GNUC__)
__attribute__((weak))
void blowup_on_video_window_event(int event_type, int x, int y, int w, int h) {
    (void)event_type; (void)x; (void)y; (void)w; (void)h;
}
#endif
```

> **Note:** MSVC does not support `__attribute__((weak))`. The `cc` crate defaults to MSVC on Windows. So under MSVC this `#if` block is skipped and we **must** provide a Rust implementation before the crate links. We do exactly that in Phase 2 Task 2.3 before attempting to build.

- [ ] **Step 6: Verify C compiles (on macOS host — skip build)**

On macOS we can't build the Windows C file. Instead, do a syntactic smoke check with `clang`:

Run: `clang -c -target x86_64-pc-windows-msvc -I crates/tauri/native crates/tauri/native/win_gl_layer.c -o /tmp/win_gl_layer.o 2>&1 | head -20 || true`

Expected: no errors (warnings about `LoadCursorW` or DWM header paths are OK if clang doesn't have the Windows SDK). If you're on a Windows host, run `cargo build --manifest-path crates/tauri/Cargo.toml` and expect a successful compile of `native_win_gl` (Rust side will still fail because Phase 2 isn't done yet — that's OK, we only care about the C compile step here).

### Task 1.4: Commit Phase 1

- [ ] **Step 1: Stage and commit**

Run:
```bash
git add crates/tauri/build.rs \
        crates/tauri/native/win_gl_layer.c \
        crates/tauri/native/win_gl_layer.h
git commit -m "$(cat <<'MSG'
feat(player/win): C skeleton for native video window

Introduces blowup_create_video_window and a dedicated WndProc class
(BlowupVideoWindow) that will host the GL child + mpv render context.
The window is borderless-with-resize-frame (WS_POPUP | WS_THICKFRAME)
and posts move/size/close/mousemove/dblclick/keydown events back to
Rust via blowup_on_video_window_event (stub for now). Fullscreen,
round corners, window control, and GL/mpv integration are stubbed —
filled in by later phases.

Co-Authored-By: Claude Opus 4.6 (1M context) <noreply@anthropic.com>
MSG
)"
```

---

## Phase 2: Rust Windows module — empty window smoke test

Implement the Rust-side `extern "C" blowup_on_video_window_event`, create the bare `windows::open_player` that only calls `blowup_create_video_window`, wire dispatch in `mod.rs`, register the function so MSVC doesn't choke on the missing weak symbol, and verify an empty native window appears when "play" is clicked.

### Task 2.1: Windows module globals and FFI declarations

**Files:**
- Modify: `crates/tauri/src/player/windows/mod.rs`
- Create: `crates/tauri/src/player/windows/video_window.rs`
- Create: `crates/tauri/src/player/windows/controls.rs` (empty stub for cfg dispatch later)
- Create: `crates/tauri/src/player/windows/fullscreen.rs` (empty stub)

- [ ] **Step 1: Write `windows/video_window.rs`**

Create `crates/tauri/src/player/windows/video_window.rs`:

```rust
//! Video window lifecycle + event dispatch (Win32 side).
//!
//! The C layer (`native/win_gl_layer.c`) creates the HWND, runs its
//! WndProc, and calls `blowup_on_video_window_event` when something
//! happens. Rust decides what to do with each event.

#![cfg(target_os = "windows")]

use std::ffi::c_void;
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};

// ---------------------------------------------------------------------------
// FFI declarations — implemented in native/win_gl_layer.c
// ---------------------------------------------------------------------------

unsafe extern "C" {
    pub fn blowup_create_video_window(width: f64, height: f64) -> *mut c_void;
    pub fn blowup_destroy_video_window(hwnd: *mut c_void);
    pub fn blowup_get_video_window_rect(
        hwnd: *mut c_void,
        x: *mut i32,
        y: *mut i32,
        w: *mut i32,
        h: *mut i32,
    );
    pub fn blowup_set_video_window_rect(hwnd: *mut c_void, x: i32, y: i32, w: i32, h: i32);

    pub fn blowup_enter_fullscreen(hwnd: *mut c_void) -> i32;
    pub fn blowup_leave_fullscreen(hwnd: *mut c_void) -> i32;
    pub fn blowup_is_fullscreen(hwnd: *mut c_void) -> i32;

    pub fn blowup_window_minimize(hwnd: *mut c_void);
    pub fn blowup_window_toggle_maximize(hwnd: *mut c_void);
    pub fn blowup_window_start_drag(hwnd: *mut c_void);
    pub fn blowup_apply_round_corners(hwnd: *mut c_void);
}

// ---------------------------------------------------------------------------
// HWND wrapper — `*mut c_void` isn't Send/Sync, so we wrap it.
// ---------------------------------------------------------------------------

#[derive(Copy, Clone)]
pub struct HwndPtr(pub *mut c_void);
unsafe impl Send for HwndPtr {}
unsafe impl Sync for HwndPtr {}

pub static PLAYER_HWND: Mutex<Option<HwndPtr>> = Mutex::new(None);

/// Set to `true` the first time the controls window finishes building
/// so event handlers can safely call `app.get_webview_window(...)`.
pub static CONTROLS_WINDOW_READY: AtomicBool = AtomicBool::new(false);

// ---------------------------------------------------------------------------
// C → Rust event callback — strong symbol overriding the weak C stub.
//
// event_type:
//   0 = move (x,y = new screen position, w,h = current size)
//   1 = size (x,y = screen position, w,h = new size)
//   2 = mousemove (x,y = client-space cursor)
//   3 = dblclick (x,y = client-space cursor)
//   4 = close
//   5 = keydown (x = Win32 virtual-key code)
//   6 = window-state-changed (x = 0 normal, 1 max, 2 fullscreen)
// ---------------------------------------------------------------------------

#[unsafe(no_mangle)]
pub extern "C" fn blowup_on_video_window_event(
    event_type: i32,
    x: i32,
    y: i32,
    w: i32,
    h: i32,
) {
    match event_type {
        0 | 1 => {
            // move / size — Phase 5 fills in the controls reposition
            tracing::trace!(event_type, x, y, w, h, "video window move/size");
            super::controls::reposition(x, y, w, h);
        }
        2 => {
            // mousemove — Phase 10 forwards to controls window
            tracing::trace!(x, y, "video mousemove");
            super::controls::forward_mouse_move();
        }
        3 => {
            // dblclick — Phase 8 toggles fullscreen
            tracing::info!("video dblclick → toggle fullscreen");
            super::fullscreen::toggle();
        }
        4 => {
            // close — Phase 3 triggers cleanup
            tracing::info!("video window WM_CLOSE");
            super::on_video_close();
        }
        5 => {
            // keydown — Phase 9 dispatches to player commands
            tracing::trace!(vk = x, "video keydown");
            keyboard::dispatch(x);
        }
        6 => {
            // window-state-changed — Phase 8 broadcasts to frontend
            tracing::info!(state = x, "video window state changed");
            super::fullscreen::on_state_changed(x);
        }
        other => {
            tracing::warn!(event_type = other, "unknown video window event");
        }
    }
    let _ = (y, w, h); // silence unused in some branches
}

// ---------------------------------------------------------------------------
// Mouse move throttling (Phase 10)
// ---------------------------------------------------------------------------

pub static LAST_MOUSEMOVE_MS: std::sync::atomic::AtomicU64 =
    std::sync::atomic::AtomicU64::new(0);
const MOUSEMOVE_THROTTLE_MS: u64 = 50;

pub fn should_forward_mouse_move(now_ms: u64) -> bool {
    let last = LAST_MOUSEMOVE_MS.load(Ordering::Relaxed);
    if now_ms.saturating_sub(last) >= MOUSEMOVE_THROTTLE_MS {
        LAST_MOUSEMOVE_MS.store(now_ms, Ordering::Relaxed);
        true
    } else {
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::Ordering;

    #[test]
    fn throttle_allows_first_call() {
        LAST_MOUSEMOVE_MS.store(0, Ordering::Relaxed);
        assert!(should_forward_mouse_move(100));
    }

    #[test]
    fn throttle_blocks_under_50ms() {
        LAST_MOUSEMOVE_MS.store(1000, Ordering::Relaxed);
        assert!(!should_forward_mouse_move(1025));
        assert!(!should_forward_mouse_move(1049));
    }

    #[test]
    fn throttle_allows_at_50ms_boundary() {
        LAST_MOUSEMOVE_MS.store(1000, Ordering::Relaxed);
        assert!(should_forward_mouse_move(1050));
    }
}

// ---------------------------------------------------------------------------
// Keyboard dispatch (spec §3.1 keeps this inside video_window.rs). Phase 9
// fills in the real mapping; this is the stub form.
// ---------------------------------------------------------------------------

pub mod keyboard {
    pub fn dispatch(_vk: i32) {
        // Phase 9
    }
}
```

- [ ] **Step 2: Write stub `windows/controls.rs`**

Create `crates/tauri/src/player/windows/controls.rs`:

```rust
//! Controls overlay window — a Tauri WebviewWindow (label "player-controls")
//! that hosts Player.tsx, floats above the video window, and follows it on
//! WM_MOVE/WM_SIZE.

#![cfg(target_os = "windows")]

// Phase 4 fills in create() / close().
pub fn create(_app: &tauri::AppHandle, _video_rect: (i32, i32, i32, i32)) -> Result<(), String> {
    Ok(())
}

pub fn close(_app: &tauri::AppHandle) {
    // no-op until Phase 4
}

// Phase 5 fills in reposition().
pub fn reposition(_x: i32, _y: i32, _w: i32, _h: i32) {
    // no-op until Phase 5
}

// Phase 10 fills in forward_mouse_move().
pub fn forward_mouse_move() {
    // no-op until Phase 10
}
```

- [ ] **Step 3: Write stub `windows/fullscreen.rs`**

Create `crates/tauri/src/player/windows/fullscreen.rs`:

```rust
//! Fullscreen state machine (Phase 8).

#![cfg(target_os = "windows")]

pub fn toggle() {
    // Phase 8
}

pub fn on_state_changed(_state: i32) {
    // Phase 8
}
```

- [ ] **Step 4: Replace `windows/mod.rs` with the real skeleton**

Edit `crates/tauri/src/player/windows/mod.rs`:

```rust
//! Windows-specific player window lifecycle.

#![cfg(target_os = "windows")]

pub mod controls;
pub mod fullscreen;
pub mod video_window;

use std::sync::OnceLock;
use tauri::{AppHandle, Manager};
use video_window::{HwndPtr, PLAYER_HWND};

/// Cached AppHandle for C → Rust callbacks that need to reach Tauri
/// (reposition controls window, emit events). Set during the first
/// successful `open_player` call.
pub static PLAYER_APP_HANDLE: OnceLock<AppHandle> = OnceLock::new();

pub fn open_player(app: &AppHandle, file_path: &str) -> Result<(), String> {
    super::close_player_inner(app);

    super::EVENT_LOOP_SHUTDOWN.store(false, std::sync::atomic::Ordering::SeqCst);
    *super::CURRENT_FILE_PATH.lock().unwrap() = Some(file_path.to_string());

    let _ = PLAYER_APP_HANDLE.set(app.clone());

    // Phase 2: only create the bare video window. No GL, no mpv, no
    // controls. Subsequent phases add the rest.
    let (tx, rx) = std::sync::mpsc::channel();
    let _ = app.run_on_main_thread(move || {
        let hwnd = unsafe { video_window::blowup_create_video_window(1280.0, 720.0) };
        if hwnd.is_null() {
            let _ = tx.send(Err("blowup_create_video_window returned NULL".to_string()));
            return;
        }
        unsafe { video_window::blowup_apply_round_corners(hwnd) };
        *PLAYER_HWND.lock().unwrap() = Some(HwndPtr(hwnd));
        let _ = tx.send(Ok(()));
    });
    rx.recv().map_err(|e| format!("phase 1 channel: {e}"))??;

    tracing::info!(file_path, "[phase 2] video window created (mpv not yet wired)");
    Ok(())
}

pub(crate) fn close_player_windows(app: &AppHandle) {
    controls::close(app);

    if let Some(HwndPtr(hwnd)) = PLAYER_HWND.lock().unwrap().take() {
        unsafe { video_window::blowup_destroy_video_window(hwnd) };
        tracing::info!("video window destroyed");
    }
}

/// Called by the C → Rust callback on WM_CLOSE. Triggers the shared
/// cleanup path so mpv is torn down first, then the window is destroyed.
pub fn on_video_close() {
    if let Some(app) = PLAYER_APP_HANDLE.get() {
        super::close_player_inner(app);
    }
}
```

- [ ] **Step 5: Verify keyboard stub is reachable**

The stub `keyboard::dispatch` was defined as a nested module inside `video_window.rs` in step 1. Confirm the `match` arm for event type 5 calls `keyboard::dispatch(x)` (unqualified, same module scope). Phase 9 fills in the real body.

### Task 2.2: Run the Rust unit tests for the throttle helper

- [ ] **Step 1: Run**

Run: `cargo test --manifest-path crates/tauri/Cargo.toml player::windows::video_window::tests --target x86_64-pc-windows-msvc 2>&1 || cargo test --manifest-path crates/tauri/Cargo.toml player::windows::video_window::tests`

Expected: 3 passing tests on Windows. On macOS the `#[cfg(target_os = "windows")]` gate means they don't compile in — that's fine.

- [ ] **Step 2: If you're on macOS and can't run Windows tests, skip this step**

The tests will be exercised when someone builds on Windows in Phase 2 Task 2.3.

### Task 2.3: Build and smoke-test on Windows

- [ ] **Step 1: Run `just dev` on Windows**

Run: `just dev`

Expected: the app builds, main window opens. No player window yet (Phase 2 only creates the HWND when `open_player` is called).

- [ ] **Step 2: Click play on a library item**

Navigate to Library, double-click a video. Watch for a new **borderless black rectangle** to appear near the center of the screen, about 1280×720. It should have a resize cursor on edges. There's no video (mpv isn't attached yet), no title bar, no controls. Closing it via Alt+F4 should log `[video window WM_CLOSE]` and disappear.

- [ ] **Step 3: If the window doesn't appear**

Open the dev console of the main window and check for:
- `failed to set up player window` tracing errors → phase 1 channel mismatch, check your `run_on_main_thread` closure capture
- `blowup_create_video_window returned NULL` → `CreateWindowExW` failed; check `GetLastError` in the C layer (add a `tracing::error!` on NULL in `open_player`)

### Task 2.4: Commit Phase 2

- [ ] **Step 1: Stage and commit**

Run:
```bash
git add crates/tauri/src/player/windows/mod.rs \
        crates/tauri/src/player/windows/video_window.rs \
        crates/tauri/src/player/windows/controls.rs \
        crates/tauri/src/player/windows/fullscreen.rs
git commit -m "$(cat <<'MSG'
feat(player/win): bare video window smoke test

Rust side of the Windows player entry path: open_player creates a
native HWND via blowup_create_video_window, close_player tears it
down. Controls, mpv, fullscreen, keyboard are all stubs. Throttle
helper for Phase 10 mouse forwarding has unit tests. Keyboard
dispatch lives as a nested module inside video_window.rs per spec
§3.1.

Co-Authored-By: Claude Opus 4.6 (1M context) <noreply@anthropic.com>
MSG
)"
```

---

## Phase 3: GL + mpv integration

Attach the existing GL child window (`win_gl_layer.c`'s `blowup_create_gl_view` + `blowup_attach_to_window`) to our new video HWND as its child, wire mpv render context, and verify video plays. This is the biggest single behavior jump — at the end of this phase the Windows player plays video with no controls.

### Task 3.1: Adjust C parent-subclass to target the video window

**Files:**
- Modify: `crates/tauri/native/win_gl_layer.c`

The existing code subclasses the parent HWND (previously Tauri's window) to resize the GL child on `WM_SIZE`. In the new flow, the parent is our video window whose WndProc already handles `WM_SIZE` directly. We should drop the subclass install in `blowup_attach_to_window` when the parent is the video window.

- [ ] **Step 1: Skip the subclass when parent is our video window**

In `blowup_attach_to_window` (around line 242), replace:

```c
    // Subclass parent to auto-resize GL child on WM_SIZE
    SetWindowSubclass(parent, parent_subclass_proc, 1, (DWORD_PTR)view);
```

with:

```c
    // If the parent is our own video window, its WndProc already handles
    // WM_SIZE by resizing the GL child; no subclass needed. Only subclass
    // when the parent is a foreign window (legacy Tauri path).
    if (parent != g_video_window.hwnd) {
        SetWindowSubclass(parent, parent_subclass_proc, 1, (DWORD_PTR)view);
    }
```

Also in `blowup_remove_view`, guard the unsubclass:

```c
    if (view->parent_hwnd && view->parent_hwnd != g_video_window.hwnd) {
        RemoveWindowSubclass(view->parent_hwnd, parent_subclass_proc, 1);
    }
```

- [ ] **Step 2: Stop reserving CONTROLS_HEIGHT on video window parents**

Still in `blowup_attach_to_window`, find the block that computes `video_h = h - CONTROLS_HEIGHT` and replace it:

```c
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
```

### Task 3.2: Windows open_player phase 2 — attach GL + mpv

**Files:**
- Modify: `crates/tauri/src/player/windows/mod.rs`

- [ ] **Step 1: Extend `open_player` with mpv setup**

Rewrite the `open_player` function in `crates/tauri/src/player/windows/mod.rs`:

```rust
pub fn open_player(app: &AppHandle, file_path: &str) -> Result<(), String> {
    super::close_player_inner(app);

    super::EVENT_LOOP_SHUTDOWN.store(false, std::sync::atomic::Ordering::SeqCst);
    *super::CURRENT_FILE_PATH.lock().unwrap() = Some(file_path.to_string());
    let _ = PLAYER_APP_HANDLE.set(app.clone());

    let file_path_owned = file_path.to_string();
    let app_clone = app.clone();
    tauri::async_runtime::spawn(async move {
        if let Err(e) = run_open_phases(app_clone, file_path_owned).await {
            tracing::error!(error = %e, "windows open_player failed");
        }
    });
    Ok(())
}

async fn run_open_phases(app: AppHandle, file_path: String) -> Result<(), String> {
    // Phase 1: create HWND (main thread)
    let (tx, rx) = tokio::sync::oneshot::channel();
    app.run_on_main_thread(move || {
        let hwnd = unsafe { video_window::blowup_create_video_window(1280.0, 720.0) };
        if hwnd.is_null() {
            let _ = tx.send(Err("blowup_create_video_window returned NULL".to_string()));
            return;
        }
        unsafe { video_window::blowup_apply_round_corners(hwnd) };
        *PLAYER_HWND.lock().unwrap() = Some(HwndPtr(hwnd));
        let _ = tx.send(Ok(()));
    })
    .map_err(|e| format!("dispatch phase 1: {e}"))?;
    rx.await.map_err(|e| format!("phase 1 channel: {e}"))??;

    // Phase 2: attach GL + mpv (main thread)
    let (tx2, rx2) = tokio::sync::oneshot::channel();
    let file_path_for_setup = file_path.clone();
    let app_for_setup = app.clone();
    app.run_on_main_thread(move || {
        let result = setup_gl_and_mpv_on_video_window(&app_for_setup, &file_path_for_setup);
        let _ = tx2.send(result);
    })
    .map_err(|e| format!("dispatch phase 2: {e}"))?;
    rx2.await.map_err(|e| format!("phase 2 channel: {e}"))??;

    // Phase 3 (controls window) is added in Phase 4 of the plan.

    Ok(())
}

fn setup_gl_and_mpv_on_video_window(app: &AppHandle, file_path: &str) -> Result<(), String> {
    use crate::player::ffi::{self, MPV_FORMAT_DOUBLE, Mpv, MpvRenderCtx};
    use crate::player::{EVENT_LOOP_RUNNING, MPV_HANDLE, MpvHandlePtr, MpvPlayer, PLAYER, RENDER_CTX, RenderCtxPtr};
    use std::sync::atomic::Ordering;

    let hwnd = PLAYER_HWND
        .lock()
        .unwrap()
        .map(|HwndPtr(p)| p)
        .ok_or_else(|| "video window missing".to_string())?;

    // Grab the actual client size from the HWND (in case DPI stretched it)
    let (mut w, mut h) = (1280i32, 720i32);
    unsafe {
        video_window::blowup_get_video_window_rect(
            hwnd,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            &mut w,
            &mut h,
        );
    }

    let view = unsafe {
        super::super::native::create_and_attach_gl_view_win_hwnd(hwnd, w as f64, h as f64)?
    };
    let _ = view;

    let mpv = Mpv::new()?;
    mpv.set_option("vo", "libmpv")?;
    mpv.set_option("hwdec", "auto")?;
    mpv.set_option("keep-open", "yes")?;
    mpv.initialize()?;

    super::super::native::make_gl_context_current();
    let get_proc_addr = super::super::native::get_gl_proc_address_fn();
    let render_ctx_raw = mpv.create_render_context(get_proc_addr)?;
    let render_ctx = MpvRenderCtx {
        ctx: render_ctx_raw,
    };

    *RENDER_CTX.lock().unwrap() = Some(RenderCtxPtr(render_ctx_raw));
    *MPV_HANDLE.lock().unwrap() = Some(MpvHandlePtr(mpv.raw_handle()));

    unsafe {
        render_ctx.set_update_callback(Some(super::super::on_mpv_render_update), std::ptr::null_mut())
    };

    mpv.observe_property("time-pos", MPV_FORMAT_DOUBLE, 1)?;
    mpv.observe_property("duration", MPV_FORMAT_DOUBLE, 2)?;
    mpv.observe_property("pause", ffi::MPV_FORMAT_FLAG, 3)?;
    mpv.observe_property("volume", MPV_FORMAT_DOUBLE, 4)?;

    mpv.command(&["loadfile", file_path])?;
    tracing::info!(file_path, "windows player: video + mpv ready");

    let player = MpvPlayer {
        _render_ctx: render_ctx,
        mpv,
    };
    *PLAYER.lock().unwrap() = Some(player);

    let app_handle = app.clone();
    std::thread::spawn(move || {
        EVENT_LOOP_RUNNING.store(true, Ordering::SeqCst);
        super::super::event_loop(&app_handle);
        EVENT_LOOP_RUNNING.store(false, Ordering::SeqCst);
        tracing::info!("event loop thread exited");
    });

    Ok(())
}
```

### Task 3.3: Add an HWND variant of `create_and_attach_gl_view` in `native.rs`

**Files:**
- Modify: `crates/tauri/src/player/native.rs`

The existing `create_and_attach_gl_view(window: &WebviewWindow)` requires a Tauri window so it can call `.hwnd()`. We need a parallel function that takes a raw HWND and size directly.

- [ ] **Step 1: Add `create_and_attach_gl_view_win_hwnd`**

Inside the `#[cfg(target_os = "windows")]` section of `native.rs`, below the existing `create_and_attach_gl_view`, add:

```rust
#[cfg(target_os = "windows")]
pub fn create_and_attach_gl_view_win_hwnd(
    parent_hwnd: *mut c_void,
    width: f64,
    height: f64,
) -> Result<*mut c_void, String> {
    remove_gl_view();

    unsafe {
        let view = blowup_create_gl_view(width, height);
        if view.is_null() {
            return Err("failed to create GL view".into());
        }

        let ret = blowup_attach_to_window(parent_hwnd, view);
        if ret != 0 {
            blowup_remove_view(view);
            return Err("failed to attach GL view".into());
        }

        *GL_VIEW_PTR.lock().unwrap() = Some(ViewPtr(view));
        tracing::info!("GL view attached to video HWND");
        Ok(view)
    }
}
```

### Task 3.4: Build and run

- [ ] **Step 1: `just dev`**

Run: `just dev`

Expected: clean build. Main window opens.

- [ ] **Step 2: Play a video**

Double-click a library item. Expected:
- Native video window appears
- Video plays (audio + picture)
- No controls bar (Phase 4 adds it)
- Alt+F4 closes the window; mpv is torn down (check tracing for `player resources cleaned up`)

- [ ] **Step 3: If video is black but audio plays**

The GL child size probably didn't reach mpv. In `setup_gl_and_mpv_on_video_window` verify `blowup_get_video_window_rect` returned non-zero width/height, and check the C side's `blowup_attach_to_window` path takes the "video_h = h" branch (Task 3.1 step 2).

- [ ] **Step 4: If the window closes but the process hangs**

`cleanup_player_resources` may be waiting on the event loop. Check `EVENT_LOOP_RUNNING` state; the `mpv_wakeup_raw` call should interrupt `wait_event`.

### Task 3.5: Commit Phase 3

- [ ] **Step 1: Stage and commit**

Run:
```bash
git add crates/tauri/native/win_gl_layer.c \
        crates/tauri/src/player/mod.rs \
        crates/tauri/src/player/native.rs \
        crates/tauri/src/player/windows/mod.rs
git commit -m "$(cat <<'MSG'
feat(player/win): play video in the native window

Phase 2 attaches the existing GL child + mpv render context to the
Rust-created video HWND. The C-layer parent-subclass is bypassed for
our own video window since its WndProc already handles WM_SIZE. On
WM_CLOSE, cleanup_player_resources tears down mpv before the HWND is
destroyed. No controls yet (Phase 4).

Co-Authored-By: Claude Opus 4.6 (1M context) <noreply@anthropic.com>
MSG
)"
```

---

## Phase 4: Controls overlay window

Create the Tauri `player-controls` WebView window sized to the video window's width × 100px, positioned at the bottom of the video window. Reuse `player.html` as the entry. No reposition follow yet (Phase 5), no window-control buttons yet (Phase 6), no Player.tsx changes yet (Phase 7). The controls window shows the existing controls bar (with its `IS_WINDOWS` branches still present) floating as a separate OS window, proving the overlay architecture works.

### Task 4.1: Implement `controls::create` and `controls::close`

**Files:**
- Modify: `crates/tauri/src/player/windows/controls.rs`

- [ ] **Step 1: Rewrite `controls.rs`**

```rust
//! Controls overlay window — a Tauri WebviewWindow (label "player-controls")
//! that hosts Player.tsx, floats above the video window, and follows it on
//! WM_MOVE/WM_SIZE.

#![cfg(target_os = "windows")]

use std::sync::atomic::Ordering;
use tauri::{
    AppHandle, Manager, PhysicalPosition, PhysicalSize, WebviewUrl, WebviewWindowBuilder,
};

use super::video_window::CONTROLS_WINDOW_READY;

const CONTROLS_HEIGHT: u32 = 100;

pub fn create(app: &AppHandle, video_rect: (i32, i32, i32, i32)) -> Result<(), String> {
    let (vx, vy, vw, vh) = video_rect;

    let window = WebviewWindowBuilder::new(
        app,
        "player-controls",
        WebviewUrl::App("player.html".into()),
    )
    .title("")
    .decorations(false)
    .skip_taskbar(true)
    .always_on_top(true)
    .transparent(true)
    .resizable(false)
    .focused(false)
    .visible(false) // positioned below before first paint to avoid a flash
    .inner_size(vw.max(640) as f64, CONTROLS_HEIGHT as f64)
    .position(vx as f64, (vy + vh - CONTROLS_HEIGHT as i32) as f64)
    .build()
    .map_err(|e| format!("failed to create player-controls window: {e}"))?;

    // Ensure the window never steals activation (WS_EX_NOACTIVATE).
    #[cfg(target_os = "windows")]
    if let Ok(hwnd) = window.hwnd() {
        use std::ffi::c_void;
        unsafe extern "system" {
            fn GetWindowLongPtrW(hwnd: *mut c_void, n_index: i32) -> isize;
            fn SetWindowLongPtrW(hwnd: *mut c_void, n_index: i32, new_long: isize) -> isize;
        }
        const GWL_EXSTYLE: i32 = -20;
        const WS_EX_NOACTIVATE: isize = 0x08000000;
        const WS_EX_TOOLWINDOW: isize = 0x00000080;
        unsafe {
            let cur = GetWindowLongPtrW(hwnd.0 as *mut c_void, GWL_EXSTYLE);
            SetWindowLongPtrW(
                hwnd.0 as *mut c_void,
                GWL_EXSTYLE,
                cur | WS_EX_NOACTIVATE | WS_EX_TOOLWINDOW,
            );
        }
    }

    // Intercept user-initiated close requests → route them to the video
    // window so the cleanup path is always consistent.
    let app_for_close = app.clone();
    window.on_window_event(move |event| {
        if let tauri::WindowEvent::CloseRequested { api, .. } = event {
            api.prevent_close();
            crate::player::close_player_inner(&app_for_close);
        }
    });

    window.show().map_err(|e| e.to_string())?;
    CONTROLS_WINDOW_READY.store(true, Ordering::SeqCst);
    tracing::info!("player-controls window created");
    Ok(())
}

pub fn close(app: &AppHandle) {
    CONTROLS_WINDOW_READY.store(false, Ordering::SeqCst);
    if let Some(w) = app.get_webview_window("player-controls") {
        w.close().ok();
        tracing::info!("player-controls window closed");
    }
}

pub fn reposition(_x: i32, _y: i32, _w: i32, _h: i32) {
    // Phase 5
}

pub fn forward_mouse_move() {
    // Phase 10
}
```

### Task 4.2: Add a phase 3 step in `windows::run_open_phases`

**Files:**
- Modify: `crates/tauri/src/player/windows/mod.rs`

- [ ] **Step 1: Append phase 3 to `run_open_phases`**

At the end of `run_open_phases` (just before `Ok(())`), add:

```rust
    // Phase 3: create the controls overlay window
    let (tx3, rx3) = tokio::sync::oneshot::channel();
    let app_for_ctrl = app.clone();
    app.run_on_main_thread(move || {
        let hwnd_opt = PLAYER_HWND.lock().unwrap().map(|HwndPtr(p)| p);
        let Some(hwnd) = hwnd_opt else {
            let _ = tx3.send(Err("video HWND missing at phase 3".to_string()));
            return;
        };
        let mut vx = 0i32;
        let mut vy = 0i32;
        let mut vw = 0i32;
        let mut vh = 0i32;
        unsafe {
            video_window::blowup_get_video_window_rect(hwnd, &mut vx, &mut vy, &mut vw, &mut vh);
        }
        let result = controls::create(&app_for_ctrl, (vx, vy, vw, vh));
        let _ = tx3.send(result);
    })
    .map_err(|e| format!("dispatch phase 3: {e}"))?;
    // Non-fatal: if controls creation fails we still let the video play.
    match rx3.await.map_err(|e| format!("phase 3 channel: {e}"))? {
        Ok(()) => {}
        Err(e) => tracing::warn!(error = %e, "controls window failed; video runs headless"),
    }
```

### Task 4.3: Build and run

- [ ] **Step 1: `just dev`**

Run: `just dev`

Expected: clean build.

- [ ] **Step 2: Play a video**

Double-click a library item. Expected:
- Video window appears with video playing
- A 100px-tall **transparent Tauri controls window** appears at the bottom of the video window
- The controls still have the legacy `IS_WINDOWS` bottom-bar look (removed in Phase 7)
- Clicking play/pause in the controls works (existing IPC commands)
- Closing the video window with Alt+F4 also closes the controls window

- [ ] **Step 3: If controls window doesn't appear**

Check tracing for `failed to create player-controls window`. Most likely:
- Capabilities still missing `player-controls` — revisit Task 0.1
- WebView2 runtime issue — unrelated to this plan

- [ ] **Step 4: If controls appear in wrong position**

Phase 5 fixes follow; for now just verify it appears *somewhere* and is not frozen offscreen.

### Task 4.4: Commit Phase 4

- [ ] **Step 1: Stage and commit**

Run:
```bash
git add crates/tauri/src/player/windows/controls.rs \
        crates/tauri/src/player/windows/mod.rs
git commit -m "$(cat <<'MSG'
feat(player/win): create player-controls overlay window

Phase 3 of windows::open_player now builds a Tauri WebView window
(label "player-controls") sized to the video window's footer and
positioned below it. WS_EX_NOACTIVATE ensures the overlay never
steals keyboard focus from the video window. CloseRequested is
intercepted and routed to close_player_inner for a consistent
cleanup path.

Co-Authored-By: Claude Opus 4.6 (1M context) <noreply@anthropic.com>
MSG
)"
```

---

## Phase 5: Follow logic — controls window tracks video window

On every `WM_MOVE` / `WM_SIZE` event, `blowup_on_video_window_event` now calls `controls::reposition` which moves the Tauri controls window to match. This makes dragging or resizing the video window feel like a single unit.

### Task 5.1: Implement `controls::reposition`

**Files:**
- Modify: `crates/tauri/src/player/windows/controls.rs`

- [ ] **Step 1: Replace the reposition stub**

Replace the `pub fn reposition` stub body:

```rust
pub fn reposition(x: i32, y: i32, w: i32, h: i32) {
    use std::sync::atomic::Ordering;
    if !CONTROLS_WINDOW_READY.load(Ordering::Acquire) {
        return;
    }
    let Some(app) = super::PLAYER_APP_HANDLE.get() else { return };
    let Some(window) = app.get_webview_window("player-controls") else { return };
    let _ = window.set_size(PhysicalSize {
        width: w.max(1) as u32,
        height: CONTROLS_HEIGHT,
    });
    let _ = window.set_position(PhysicalPosition {
        x,
        y: y + h - CONTROLS_HEIGHT as i32,
    });
}
```

### Task 5.2: Build and run

- [ ] **Step 1: `just dev`**

Run: `just dev`

- [ ] **Step 2: Drag the video window**

Grab the resize frame (bottom-right corner) and resize — controls window should track the bottom edge and match the width. Try dragging the video window around the screen (for now via F10 system menu or keyboard shortcut since there's no title bar drag yet) — controls should follow.

*Note:* user-initiated drag via the controls window drag strip is Phase 7. For now, the only way to move the video window is programmatic or via system menu.

- [ ] **Step 3: If controls lag badly or flicker**

`WM_MOVE` events during drag are on the main thread and synchronous — there shouldn't be lag. If you see flicker, check that `set_position` and `set_size` are not both triggering redraws — consolidate into a single `set_position`+`set_size` call sequence (already done).

### Task 5.3: Commit Phase 5

- [ ] **Step 1: Stage and commit**

Run:
```bash
git add crates/tauri/src/player/windows/controls.rs
git commit -m "$(cat <<'MSG'
feat(player/win): controls window follows video window on move/resize

reposition() is invoked from blowup_on_video_window_event (types 0/1)
and set_position/set_size the Tauri controls window to the new
bottom-edge strip of the video window. The two windows now feel like
a single player unit during resize.

Co-Authored-By: Claude Opus 4.6 (1M context) <noreply@anthropic.com>
MSG
)"
```

---

## Phase 6: Window management commands

Add `cmd_player_window_minimize`, `cmd_player_window_toggle_maximize`, `cmd_player_window_start_drag`. Implement the corresponding C FFI entry points. Register the new commands in `lib.rs`. The `cmd_player_toggle_fullscreen` command reroutes on Windows to a (still stubbed) `windows::fullscreen::toggle`.

### Task 6.1: Fill in the C-side window-control functions

**Files:**
- Modify: `crates/tauri/native/win_gl_layer.c`

- [ ] **Step 1: Replace the stubs at the bottom of the file**

Find the stub block added in Phase 1 Task 1.3 step 4 and replace with real implementations:

```c
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
```

Leave `blowup_apply_round_corners` and the fullscreen stubs intact — they are filled in by Phases 8 and 11.

### Task 6.2: Add the Tauri commands

**Files:**
- Modify: `crates/tauri/src/player/commands.rs`

- [ ] **Step 1: Append new commands to `commands.rs`**

Add at the bottom of `crates/tauri/src/player/commands.rs`:

```rust
// ---------------------------------------------------------------------------
// Window management (Windows-only bodies; macOS falls back to Tauri window API)
// ---------------------------------------------------------------------------

#[tauri::command]
pub fn cmd_player_window_minimize(app: tauri::AppHandle) -> Result<(), String> {
    tracing::info!("cmd_player_window_minimize");
    #[cfg(target_os = "windows")]
    {
        let _ = &app;
        if let Some(super::windows::video_window::HwndPtr(hwnd)) =
            *super::windows::video_window::PLAYER_HWND.lock().unwrap()
        {
            unsafe { super::windows::video_window::blowup_window_minimize(hwnd) };
        }
        return Ok(());
    }
    #[cfg(target_os = "macos")]
    {
        if let Some(w) = app.get_webview_window("player") {
            w.minimize().map_err(|e| e.to_string())?;
        }
        Ok(())
    }
}

#[tauri::command]
pub fn cmd_player_window_toggle_maximize(app: tauri::AppHandle) -> Result<(), String> {
    tracing::info!("cmd_player_window_toggle_maximize");
    #[cfg(target_os = "windows")]
    {
        let _ = &app;
        if let Some(super::windows::video_window::HwndPtr(hwnd)) =
            *super::windows::video_window::PLAYER_HWND.lock().unwrap()
        {
            unsafe { super::windows::video_window::blowup_window_toggle_maximize(hwnd) };
        }
        return Ok(());
    }
    #[cfg(target_os = "macos")]
    {
        if let Some(w) = app.get_webview_window("player") {
            let is_max = w.is_maximized().map_err(|e| e.to_string())?;
            if is_max {
                w.unmaximize().map_err(|e| e.to_string())?;
            } else {
                w.maximize().map_err(|e| e.to_string())?;
            }
        }
        Ok(())
    }
}

#[tauri::command]
pub fn cmd_player_window_start_drag(app: tauri::AppHandle) -> Result<(), String> {
    tracing::info!("cmd_player_window_start_drag");
    #[cfg(target_os = "windows")]
    {
        let _ = &app;
        if let Some(super::windows::video_window::HwndPtr(hwnd)) =
            *super::windows::video_window::PLAYER_HWND.lock().unwrap()
        {
            unsafe { super::windows::video_window::blowup_window_start_drag(hwnd) };
        }
        Ok(())
    }
    #[cfg(target_os = "macos")]
    {
        // macOS has native title bar / drag region; no-op.
        let _ = app;
        Ok(())
    }
}
```

- [ ] **Step 2: Reroute `cmd_player_toggle_fullscreen`**

Replace the existing `cmd_player_toggle_fullscreen` implementation:

```rust
#[tauri::command]
pub fn cmd_player_toggle_fullscreen(app: tauri::AppHandle) -> Result<(), String> {
    tracing::info!("cmd_player_toggle_fullscreen");
    #[cfg(target_os = "windows")]
    {
        let _ = &app;
        super::windows::fullscreen::toggle();
        return Ok(());
    }
    #[cfg(target_os = "macos")]
    {
        if let Some(window) = app.get_webview_window("player") {
            let is_fullscreen = window.is_fullscreen().map_err(|e| e.to_string())?;
            window
                .set_fullscreen(!is_fullscreen)
                .map_err(|e| e.to_string())?;
        }
        Ok(())
    }
}
```

### Task 6.3: Register new commands in `lib.rs`

**Files:**
- Modify: `crates/tauri/src/lib.rs`

- [ ] **Step 1: Add three entries to `invoke_handler`**

In `crates/tauri/src/lib.rs` find the `invoke_handler![...]` block (line 259). After `player::commands::cmd_player_load_overlay_subs`, add:

```rust
            player::commands::cmd_player_window_minimize,
            player::commands::cmd_player_window_toggle_maximize,
            player::commands::cmd_player_window_start_drag,
```

### Task 6.4: Build

- [ ] **Step 1: `cargo clippy`**

Run: `cargo clippy --manifest-path crates/tauri/Cargo.toml -- -D warnings`

Expected: pass. If you see `unused` warnings for `app` in the Windows branches, leave the `let _ = &app;` lines in place — they silence those without breaking macOS.

- [ ] **Step 2: Smoke test on Windows**

Run: `just dev`

Play a video. The new commands aren't hooked to UI yet (Phase 7), but the app should still build and run as after Phase 5.

### Task 6.5: Commit Phase 6

- [ ] **Step 1: Stage and commit**

Run:
```bash
git add crates/tauri/native/win_gl_layer.c \
        crates/tauri/src/player/commands.rs \
        crates/tauri/src/lib.rs
git commit -m "$(cat <<'MSG'
feat(player/win): window management commands (min/max/drag/fullscreen)

cmd_player_window_{minimize,toggle_maximize,start_drag} dispatch to
Win32 APIs via FFI on Windows, fall back to Tauri window API on
macOS. cmd_player_toggle_fullscreen reroutes to the Windows state
machine stub. Commands registered in lib.rs invoke_handler.

Co-Authored-By: Claude Opus 4.6 (1M context) <noreply@anthropic.com>
MSG
)"
```

---

## Phase 7: Frontend — drop IS_WINDOWS, add window control buttons

Remove all `IS_WINDOWS` branches from `Player.tsx`. Add three window-control buttons (min/max/close) in the top-right corner. Add an 8px drag strip at the top of the controls bar that calls `cmd_player_window_start_drag` on `onMouseDown`. Listen for `player:video-mouse-move` and `player:window-state` events.

### Task 7.1: Remove IS_WINDOWS branches

**Files:**
- Modify: `src/Player.tsx`

- [ ] **Step 1: Delete the `IS_WINDOWS` constant and `CONTROLS_HEIGHT`**

At `src/Player.tsx:39-52`, delete the entire "Platform detection" block (comment + `IS_WINDOWS` + `CONTROLS_HEIGHT`).

- [ ] **Step 2: Simplify the `glass` tokens**

Replace the `glass` object (currently lines 56-75) with the macOS-only values:

```ts
const glass = {
  bg: "rgba(255, 255, 255, 0.06)",
  bgHover: "rgba(255, 255, 255, 0.12)",
  bgActive: "rgba(255, 255, 255, 0.18)",
  border: "1px solid rgba(255, 255, 255, 0.12)",
  borderLight: "1px solid rgba(255, 255, 255, 0.08)",
  backdrop: "blur(40px) saturate(180%)",
  shadow: "0 8px 32px rgba(0, 0, 0, 0.35), inset 0 1px 0 rgba(255, 255, 255, 0.08)",
  shadowSmall: "0 2px 8px rgba(0, 0, 0, 0.3)",
  radius: 14,
  radiusSmall: 8,
  text: "rgba(255, 255, 255, 0.9)",
  textDim: "rgba(255, 255, 255, 0.5)",
  trackBg: "rgba(255, 255, 255, 0.15)",
  trackFill: "rgba(255, 255, 255, 0.85)",
};
```

- [ ] **Step 3: Simplify `resetHideTimer`**

Replace the current `resetHideTimer` (lines 212-219) with:

```ts
const resetHideTimer = useCallback(() => {
  setShowControls(true);
  if (hideTimer.current) clearTimeout(hideTimer.current);
  hideTimer.current = window.setTimeout(() => {
    if (!seeking && !showTracks && isFullscreen) setShowControls(false);
  }, 3000);
}, [seeking, showTracks, isFullscreen]);
```

And add state near the top of the component:

```ts
const [isFullscreen, setIsFullscreen] = useState(false);
```

Rationale: per spec §4.7, auto-hide only activates in fullscreen. Non-fullscreen keeps `showControls` true.

- [ ] **Step 4: Delete the click-to-play-pause video area div**

Delete the entire `<div data-tauri-drag-region... onClick={() => playPause()} onDoubleClick={...}/>` block at lines 298-303 (the "Click area" comment). The video window handles clicks in its WndProc now.

- [ ] **Step 5: Simplify the main container style**

Replace the outer `<div style={{...fixed, inset:0...}}>` block (around line 273) with:

```tsx
<div
  style={{
    position: "fixed", inset: 0,
    background: "transparent",
    display: "flex", flexDirection: "column",
    cursor: "default",
    fontFamily: "-apple-system, BlinkMacSystemFont, 'SF Pro Text', 'Helvetica Neue', sans-serif",
    userSelect: "none",
  }}
  onMouseMove={resetHideTimer}
>
```

- [ ] **Step 6: Simplify the controls bar style**

Replace the controls bar div's style (around line 354) with the macOS branch unconditionally:

```tsx
<div style={{
  margin: "0 16px 16px",
  padding: "12px 16px 14px",
  background: glass.bg,
  backdropFilter: glass.backdrop,
  WebkitBackdropFilter: glass.backdrop,
  border: glass.border,
  borderRadius: glass.radius,
  boxShadow: glass.shadow,
  opacity: showControls ? 1 : 0,
  transition: "opacity 0.3s ease",
  pointerEvents: showControls ? "auto" : "none",
}}>
```

### Task 7.2: Add window-control buttons

**Files:**
- Modify: `src/Player.tsx`

- [ ] **Step 1: Add three button components above the controls bar**

Inside the main JSX, above the `{/* Controls bar */}` block, add:

```tsx
{/* Top strip: drag region + window control buttons (Windows only has
    effect; macOS has its own native title bar on the player window). */}
<div
  onMouseDown={(e) => {
    if (e.button !== 0) return;
    e.preventDefault();
    invoke("cmd_player_window_start_drag");
  }}
  style={{
    position: "absolute", top: 0, left: 0, right: 0,
    height: 28,
    display: "flex", justifyContent: "flex-end", alignItems: "center",
    paddingRight: 8,
    gap: 4,
    pointerEvents: showControls ? "auto" : "none",
    opacity: showControls ? 1 : 0,
    transition: "opacity 0.3s ease",
  }}
>
  <WindowButton onClick={() => invoke("cmd_player_window_minimize")}>
    <svg width="12" height="12" viewBox="0 0 12 12" fill="none" stroke="currentColor" strokeWidth="1.5">
      <line x1="2" y1="6" x2="10" y2="6" strokeLinecap="round" />
    </svg>
  </WindowButton>
  <WindowButton onClick={() => invoke("cmd_player_window_toggle_maximize")}>
    <svg width="12" height="12" viewBox="0 0 12 12" fill="none" stroke="currentColor" strokeWidth="1.5">
      <rect x="2.5" y="2.5" width="7" height="7" rx="1" />
    </svg>
  </WindowButton>
  <WindowButton onClick={() => invoke("cmd_close_player")}>
    <svg width="12" height="12" viewBox="0 0 12 12" fill="none" stroke="currentColor" strokeWidth="1.5">
      <line x1="3" y1="3" x2="9" y2="9" strokeLinecap="round" />
      <line x1="9" y1="3" x2="3" y2="9" strokeLinecap="round" />
    </svg>
  </WindowButton>
</div>
```

And at the bottom of the file (next to `GlassButton` and `TrackItem`), add:

```tsx
function WindowButton({ onClick, children }: {
  onClick: () => void;
  children: React.ReactNode;
}) {
  const [hover, setHover] = useState(false);
  return (
    <button
      onClick={onClick}
      onMouseEnter={() => setHover(true)}
      onMouseLeave={() => setHover(false)}
      style={{
        background: hover ? glass.bgHover : "transparent",
        border: "none",
        color: glass.text,
        cursor: "pointer",
        width: 22, height: 22,
        borderRadius: 4,
        display: "flex", alignItems: "center", justifyContent: "center",
        lineHeight: 0,
        transition: "background 0.15s ease",
      }}
    >
      {children}
    </button>
  );
}
```

### Task 7.3: Listen for video-window events

**Files:**
- Modify: `src/Player.tsx`

- [ ] **Step 1: Add event listeners in an effect**

Near the existing `useEffect` that subscribes to `player-state`, add a new effect:

```ts
// Video window → controls overlay events (Windows). macOS never emits
// these because the video and controls share one native window.
useEffect(() => {
  const unlistenMouse = listen("player:video-mouse-move", () => {
    resetHideTimer();
  });
  const unlistenState = listen<{ state: number }>("player:window-state", (event) => {
    // state: 0 = normal, 1 = maximized, 2 = fullscreen
    setIsFullscreen(event.payload.state === 2);
  });
  return () => {
    unlistenMouse.then((f) => f());
    unlistenState.then((f) => f());
  };
}, [resetHideTimer]);
```

### Task 7.4: Build and run

- [ ] **Step 1: `just typecheck` + `just lint`**

Run: `just typecheck && just lint`

Expected: pass.

- [ ] **Step 2: `just dev`**

Play a video. Expected:
- Controls bar now has liquid glass background with `blur(40px) saturate(180%)` — not the opaque black strip
- Controls bar has rounded corners (14px)
- Top-right corner has three window buttons: min / max (square) / close (×)
- Click min: video window minimizes (controls go with it because they're a child of the same OS composition group via `always_on_top` + position follow)
- Click max: video window maximizes to work area, controls follow
- Click ×: both windows close + mpv cleanup
- Drag the top 28px strip: video window moves, controls follow

- [ ] **Step 3: If drag doesn't work**

Check the console for `cmd_player_window_start_drag` invocation errors. The most common cause is the mouse being captured by a React synthetic event handler above ours — make sure `e.preventDefault()` is called.

### Task 7.5: Commit Phase 7

- [ ] **Step 1: Stage and commit**

Run:
```bash
git add src/Player.tsx
git commit -m "$(cat <<'MSG'
feat(player/ui): unify Player.tsx across platforms, add window controls

Drops all IS_WINDOWS branches now that Windows has a separate native
video window. Liquid glass background, rounded corners, and auto-hide
come back on Windows. Adds a 28px top drag strip that triggers the
video window drag via cmd_player_window_start_drag, plus three
window-control buttons (minimize / maximize / close) in the strip's
right-hand side.

Co-Authored-By: Claude Opus 4.6 (1M context) <noreply@anthropic.com>
MSG
)"
```

---

## Phase 8: Fullscreen state machine

Implement `blowup_enter_fullscreen` / `blowup_leave_fullscreen` in C, `VideoWindowState` enum and `toggle()` / `on_state_changed()` in Rust, wire `WM_LBUTTONDBLCLK` to trigger toggle. Emit `player:window-state` events. Add Rust unit tests for state transitions.

### Task 8.1: Rust state machine

**Files:**
- Modify: `crates/tauri/src/player/windows/fullscreen.rs`

- [ ] **Step 1: Replace stub with real state machine**

```rust
//! Fullscreen state machine for the Windows native video window.
//!
//! The actual style/placement manipulation lives in C (see
//! blowup_enter_fullscreen / blowup_leave_fullscreen). Rust owns the
//! logical state enum and the toggle decision, plus the broadcast to
//! the frontend.

#![cfg(target_os = "windows")]

use serde::Serialize;
use std::sync::Mutex;
use tauri::{Emitter, Manager};

use super::PLAYER_APP_HANDLE;
use super::video_window::{HwndPtr, PLAYER_HWND};

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum VideoWindowState {
    Normal,
    Maximized,
    Fullscreen,
}

impl VideoWindowState {
    pub fn from_code(code: i32) -> Self {
        match code {
            1 => Self::Maximized,
            2 => Self::Fullscreen,
            _ => Self::Normal,
        }
    }
    pub fn to_code(self) -> i32 {
        match self {
            Self::Normal => 0,
            Self::Maximized => 1,
            Self::Fullscreen => 2,
        }
    }
}

static LOGICAL_STATE: Mutex<VideoWindowState> = Mutex::new(VideoWindowState::Normal);

#[derive(Serialize, Clone)]
struct WindowStatePayload {
    state: i32,
}

pub fn toggle() {
    let hwnd_opt = PLAYER_HWND.lock().unwrap().map(|HwndPtr(p)| p);
    let Some(hwnd) = hwnd_opt else { return };

    let is_fs = unsafe { super::video_window::blowup_is_fullscreen(hwnd) } != 0;
    let ret = unsafe {
        if is_fs {
            super::video_window::blowup_leave_fullscreen(hwnd)
        } else {
            super::video_window::blowup_enter_fullscreen(hwnd)
        }
    };
    if ret != 0 {
        tracing::warn!(is_fs, "fullscreen toggle failed");
    }
    // The C layer broadcasts WM_SIZE-driven window-state-changed (event 6)
    // afterwards; we don't need to emit here directly.
}

/// Called from the C → Rust event callback with the new state code.
pub fn on_state_changed(code: i32) {
    let state = VideoWindowState::from_code(code);
    *LOGICAL_STATE.lock().unwrap() = state;

    if let Some(app) = PLAYER_APP_HANDLE.get() {
        let _ = app.emit_to(
            "player-controls",
            "player:window-state",
            WindowStatePayload {
                state: state.to_code(),
            },
        );
    }
    tracing::info!(?state, "broadcast window-state");
}

pub fn current_state() -> VideoWindowState {
    *LOGICAL_STATE.lock().unwrap()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn state_code_roundtrip() {
        for s in [
            VideoWindowState::Normal,
            VideoWindowState::Maximized,
            VideoWindowState::Fullscreen,
        ] {
            assert_eq!(VideoWindowState::from_code(s.to_code()), s);
        }
    }

    #[test]
    fn from_code_defaults_to_normal() {
        assert_eq!(VideoWindowState::from_code(99), VideoWindowState::Normal);
        assert_eq!(VideoWindowState::from_code(-1), VideoWindowState::Normal);
    }

    #[test]
    fn from_code_known_values() {
        assert_eq!(VideoWindowState::from_code(0), VideoWindowState::Normal);
        assert_eq!(VideoWindowState::from_code(1), VideoWindowState::Maximized);
        assert_eq!(VideoWindowState::from_code(2), VideoWindowState::Fullscreen);
    }
}
```

### Task 8.2: C-side enter/leave fullscreen

**Files:**
- Modify: `crates/tauri/native/win_gl_layer.c`

- [ ] **Step 1: Replace fullscreen stubs with real implementations**

Find the stub `blowup_enter_fullscreen` / `blowup_leave_fullscreen` / `blowup_is_fullscreen` at the bottom of the file and replace with:

```c
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
```

### Task 8.3: Wire WM_LBUTTONDBLCLK in the video WndProc

**Files:**
- Modify: `crates/tauri/native/win_gl_layer.c`

- [ ] **Step 1: Add a `WM_LBUTTONDBLCLK` case in `video_wnd_proc`**

Right after the `WM_CLOSE` case, add:

```c
    case WM_LBUTTONDBLCLK: {
        blowup_on_video_window_event(3, LOWORD(lp), HIWORD(lp), 0, 0);
        return 0;
    }
```

Also, **enable double-click detection on the window class** — `WM_LBUTTONDBLCLK` only fires if the class style includes `CS_DBLCLKS`. In `ensure_video_wnd_class`, change:

```c
    wc.style = CS_HREDRAW | CS_VREDRAW;
```

to:

```c
    wc.style = CS_HREDRAW | CS_VREDRAW | CS_DBLCLKS;
```

### Task 8.4: Add `cmd_player_close_player` dispatch for Windows

**Files:**
- Modify: `crates/tauri/src/player/commands.rs`

`close_player_inner` is already platform-dispatched via `mod.rs`, so `cmd_player_close_player` already works. Verify by reading the current body — if it just calls `close_player(&app)`, no change needed. Confirm and move on.

- [ ] **Step 1: Verify**

Read `crates/tauri/src/player/commands.rs` `cmd_player_close_player`. Expected: it calls `close_player(&app)`. No edit needed.

### Task 8.5: Run unit tests

- [ ] **Step 1: `cargo test`**

Run: `cargo test --manifest-path crates/tauri/Cargo.toml player::windows`

Expected: all throttle tests from Phase 2 + the three fullscreen state tests pass (6 total).

### Task 8.6: Build and smoke-test fullscreen

- [ ] **Step 1: `just dev`**

- [ ] **Step 2: Open a video**

- [ ] **Step 3: Press F**

Expected: video window enters borderless fullscreen on the current monitor; controls window repositions to the bottom of the new fullscreen rect; `player:window-state { state: 2 }` event logged in devtools of controls window.

- [ ] **Step 4: Press Esc**

Expected: Esc does NOT work yet (Phase 9 wires keyboard dispatch). Instead, test with the frontend F button in controls bar, or double-click. At this point Esc just takes no action — Phase 9 fixes it.

**Workaround for Phase 8 testing**: click the frontend fullscreen button in the controls bar.

- [ ] **Step 5: Double-click the video area**

Expected: double-click toggles fullscreen (via WM_LBUTTONDBLCLK → event type 3 → `fullscreen::toggle`).

### Task 8.7: Commit Phase 8

- [ ] **Step 1: Stage and commit**

Run:
```bash
git add crates/tauri/native/win_gl_layer.c \
        crates/tauri/src/player/windows/fullscreen.rs
git commit -m "$(cat <<'MSG'
feat(player/win): fullscreen state machine + double-click toggle

blowup_enter_fullscreen saves WINDOWPLACEMENT + style, strips
WS_CAPTION/WS_THICKFRAME, and stretches to the current monitor's
rcMonitor. blowup_leave_fullscreen restores. Rust's fullscreen::toggle
picks enter vs leave based on blowup_is_fullscreen, and
on_state_changed broadcasts player:window-state to the controls
window. Double-click (WM_LBUTTONDBLCLK, CS_DBLCLKS) drives toggle.

Co-Authored-By: Claude Opus 4.6 (1M context) <noreply@anthropic.com>
MSG
)"
```

---

## Phase 9: Keyboard dispatch

Add `WM_KEYDOWN` handling in the C WndProc so keys are forwarded to Rust. Implement the Rust mapping table that invokes player commands.

### Task 9.1: C-side WM_KEYDOWN

**Files:**
- Modify: `crates/tauri/native/win_gl_layer.c`

- [ ] **Step 1: Add `WM_KEYDOWN` case**

In `video_wnd_proc`, after the `WM_LBUTTONDBLCLK` case, add:

```c
    case WM_KEYDOWN: {
        int vk = (int)wp;
        blowup_on_video_window_event(5, vk, 0, 0, 0);
        return 0;
    }

    case WM_MOUSEMOVE: {
        DWORD now = GetTickCount();
        if (now - g_video_window.last_mousemove_tick >= 50) {
            g_video_window.last_mousemove_tick = now;
            blowup_on_video_window_event(2, LOWORD(lp), HIWORD(lp), 0, 0);
        }
        return 0;
    }
```

(We fold in `WM_MOUSEMOVE` here because Phase 10 needs it and it's trivial to add in the same patch — we commit this C change as part of Phase 9's commit but the Rust consumer for mousemove comes in Phase 10.)

### Task 9.2: Rust keyboard dispatch

**Files:**
- Modify: `crates/tauri/src/player/windows/video_window.rs` (replace the nested `keyboard` stub module from Phase 2)

- [ ] **Step 1: Replace the nested `keyboard` module**

Find the `pub mod keyboard { pub fn dispatch(_vk: i32) { /* Phase 9 */ } }` block at the bottom of `video_window.rs` and replace with:

```rust
// ---------------------------------------------------------------------------
// Keyboard dispatch — the native video window captures WM_KEYDOWN and
// forwards the VK code here via blowup_on_video_window_event(5, vk, ...).
// We map VK codes to existing player commands.
// ---------------------------------------------------------------------------

pub mod keyboard {
    // Win32 virtual-key constants. Letter keys map to ASCII uppercase values;
    // there is no VK_F or VK_M macro in the Windows SDK.
    const VK_SPACE: i32 = 0x20;
    const VK_LEFT: i32 = 0x25;
    const VK_UP: i32 = 0x26;
    const VK_RIGHT: i32 = 0x27;
    const VK_DOWN: i32 = 0x28;
    const VK_ESCAPE: i32 = 0x1B;
    const VK_F: i32 = 0x46;
    const VK_M: i32 = 0x4D;

    pub fn dispatch(vk: i32) {
        match vk {
            VK_SPACE => {
                let _ = crate::player::commands::cmd_player_play_pause();
            }
            VK_LEFT => {
                let _ = crate::player::commands::cmd_player_seek_relative(-5.0);
            }
            VK_RIGHT => {
                let _ = crate::player::commands::cmd_player_seek_relative(5.0);
            }
            VK_UP => {
                let cur = crate::player::with_player(|p| {
                    Ok(p.mpv.get_property_double("volume").unwrap_or(100.0))
                })
                .unwrap_or(100.0);
                let _ = crate::player::commands::cmd_player_set_volume(
                    (cur + 5.0).min(100.0),
                );
            }
            VK_DOWN => {
                let cur = crate::player::with_player(|p| {
                    Ok(p.mpv.get_property_double("volume").unwrap_or(0.0))
                })
                .unwrap_or(0.0);
                let _ = crate::player::commands::cmd_player_set_volume(
                    (cur - 5.0).max(0.0),
                );
            }
            VK_F => {
                crate::player::windows::fullscreen::toggle();
            }
            VK_ESCAPE => {
                let hwnd_opt = super::PLAYER_HWND.lock().unwrap().map(|super::HwndPtr(p)| p);
                if let Some(hwnd) = hwnd_opt
                    && unsafe { super::blowup_is_fullscreen(hwnd) } != 0
                {
                    unsafe { super::blowup_leave_fullscreen(hwnd) };
                }
            }
            VK_M => {
                let cur = crate::player::with_player(|p| {
                    Ok(p.mpv.get_property_double("volume").unwrap_or(100.0))
                })
                .unwrap_or(100.0);
                let new = if cur > 0.0 { 0.0 } else { 100.0 };
                let _ = crate::player::commands::cmd_player_set_volume(new);
            }
            _ => {}
        }
    }
}
```

### Task 9.3: Build and test

- [ ] **Step 1: `just dev`**

- [ ] **Step 2: Play, then press keys**

Expected:
- Space: play/pause toggles
- Left/Right: seek ±5s
- Up/Down: volume ±5
- F: toggle fullscreen
- Esc: exit fullscreen (no-op if not in fullscreen)
- M: mute/unmute toggle

- [ ] **Step 3: If keys don't register**

The video window must have keyboard focus. Click the video area first. If it still doesn't work, check that `WM_KEYDOWN` is firing (add a temporary `tracing::info!(vk, "video keydown")` in `blowup_on_video_window_event`).

### Task 9.4: Commit Phase 9

- [ ] **Step 1: Stage and commit**

Run:
```bash
git add crates/tauri/native/win_gl_layer.c \
        crates/tauri/src/player/windows/video_window.rs
git commit -m "$(cat <<'MSG'
feat(player/win): keyboard shortcuts via video window WM_KEYDOWN

Video WndProc now forwards WM_KEYDOWN and WM_MOUSEMOVE to Rust
(mouse throttled in C at 50ms). The nested keyboard::dispatch
module inside video_window.rs maps VK codes to existing player
commands (space/seek/volume/fullscreen/mute/escape). Uses hardcoded
VK constants for letter keys (no VK_F macro exists in Windows SDK).

Co-Authored-By: Claude Opus 4.6 (1M context) <noreply@anthropic.com>
MSG
)"
```

---

## Phase 10: Mouse auto-hide wiring

The C side already forwards throttled `WM_MOUSEMOVE` events from Phase 9. This phase wires the Rust `controls::forward_mouse_move` to emit a Tauri event, and the frontend already listens for it (Phase 7 Task 7.3). So this phase is mostly plumbing — just fill in the Rust forward.

### Task 10.1: Implement `controls::forward_mouse_move`

**Files:**
- Modify: `crates/tauri/src/player/windows/controls.rs`

- [ ] **Step 1: Replace the stub**

```rust
pub fn forward_mouse_move() {
    use std::sync::atomic::Ordering;
    if !CONTROLS_WINDOW_READY.load(Ordering::Acquire) {
        return;
    }
    let Some(app) = super::PLAYER_APP_HANDLE.get() else { return };
    let _ = app.emit_to("player-controls", "player:video-mouse-move", ());
}
```

(Add `use tauri::Emitter;` at the top of the file if not already there.)

### Task 10.2: Run and verify

- [ ] **Step 1: `just dev`**

- [ ] **Step 2: Play a video, press F to enter fullscreen**

Expected:
- Rest for 3 seconds — controls bar fades out
- Move mouse — controls bar fades back in instantly
- Rest again — fades out again

- [ ] **Step 3: Press Esc to exit fullscreen**

Expected: controls bar stays visible permanently (non-fullscreen → `isFullscreen=false` → `resetHideTimer` no longer schedules a hide).

### Task 10.3: Commit Phase 10

- [ ] **Step 1: Stage and commit**

Run:
```bash
git add crates/tauri/src/player/windows/controls.rs
git commit -m "$(cat <<'MSG'
feat(player/win): wire mouse auto-hide in fullscreen

controls::forward_mouse_move emits player:video-mouse-move to the
controls window, which resets the existing 3s auto-hide timer.
Combined with the isFullscreen guard added in Phase 7, this gives
Windows the same fade-out behavior as macOS in fullscreen while
keeping controls always visible in Normal/Maximized.

Co-Authored-By: Claude Opus 4.6 (1M context) <noreply@anthropic.com>
MSG
)"
```

---

## Phase 11: HiDPI, multi-monitor, DWM rounded corners

Per-monitor DPI was set up in Phase 1 Task 1.3 step 3 via `SetProcessDpiAwarenessContext`. This phase fills in `blowup_apply_round_corners` (DWM) and verifies multi-monitor fullscreen works.

### Task 11.1: DWM rounded corners

**Files:**
- Modify: `crates/tauri/native/win_gl_layer.c`

- [ ] **Step 1: Replace the stub**

Replace:

```c
void blowup_apply_round_corners(void* hwnd) { (void)hwnd; }
```

with:

```c
void blowup_apply_round_corners(void* hwnd_ptr)
{
    if (!hwnd_ptr) return;
    HWND hwnd = (HWND)hwnd_ptr;

    // DWMWA_WINDOW_CORNER_PREFERENCE = 33 (Windows 11+)
    // DWMWCP_ROUND = 2
    enum { DWMWA_WINDOW_CORNER_PREFERENCE_LOCAL = 33 };
    enum { DWMWCP_ROUND_LOCAL = 2 };

    int pref = DWMWCP_ROUND_LOCAL;
    // Silently ignore HRESULT on older Windows where the attribute is
    // unsupported — DwmSetWindowAttribute just returns an error code,
    // the window stays square-cornered.
    DwmSetWindowAttribute(hwnd, DWMWA_WINDOW_CORNER_PREFERENCE_LOCAL,
                          &pref, sizeof(pref));
}
```

### Task 11.2: Multi-monitor fullscreen verification

This is already working from Phase 8 via `MonitorFromWindow(hwnd, MONITOR_DEFAULTTONEAREST)` in `blowup_enter_fullscreen`. No code changes needed.

- [ ] **Step 1: Manual test**

If you have a second monitor:
1. Drag the video window onto monitor 2 (use the top drag strip)
2. Press F
3. Expected: video goes fullscreen on monitor 2
4. Press Esc
5. Drag to monitor 1, press F
6. Expected: fullscreens on monitor 1

If you only have one monitor, skip this step — code is correct by inspection.

### Task 11.3: Commit Phase 11

- [ ] **Step 1: Stage and commit**

Run:
```bash
git add crates/tauri/native/win_gl_layer.c
git commit -m "$(cat <<'MSG'
feat(player/win): Windows 11 DWM rounded corners on video window

blowup_apply_round_corners sets DWMWA_WINDOW_CORNER_PREFERENCE to
DWMWCP_ROUND via DwmSetWindowAttribute. Silent no-op on Windows 10.
Per-monitor DPI awareness (Phase 1) + MonitorFromWindow fullscreen
(Phase 8) mean multi-monitor and HiDPI already work without further
code changes.

Co-Authored-By: Claude Opus 4.6 (1M context) <noreply@anthropic.com>
MSG
)"
```

---

## Phase 12: App exit cleanup

`lib.rs` `RunEvent::Exit` currently doesn't tear down the player. On Windows, this means if you close the main window while the player is open, the video HWND leaks (mpv thread still running, GL context held, etc). Add a `close_player_inner` call at the top of the Exit handler.

### Task 12.1: Call `close_player_inner` on Exit

**Files:**
- Modify: `crates/tauri/src/lib.rs:366-387`

- [ ] **Step 1: Insert player cleanup at the top of the Exit handler**

Replace the `.run(|handle, event| { ... })` block's `if let tauri::RunEvent::Exit = event { ... }` body with:

```rust
        .run(|handle, event| {
            if let tauri::RunEvent::Exit = event {
                crate::player::close_player_inner(handle);
                cache::flush_cache();
                if let Some(idx) = handle.try_state::<Arc<LibraryIndex>>() {
                    idx.flush();
                }
                if let Some(pool) = handle.try_state::<sqlx::SqlitePool>() {
                    tauri::async_runtime::block_on(async {
                        sqlx::query(
                            "UPDATE downloads SET status='paused' WHERE status='downloading'",
                        )
                        .execute(pool.inner())
                        .await
                        .ok();
                    });
                }
                if let Some(tm) = handle.try_state::<TorrentManager>() {
                    tm.shutdown();
                }
            }
        });
```

### Task 12.2: Verify

- [ ] **Step 1: Make `close_player_inner` visible to lib.rs**

`close_player_inner` is declared `pub(crate)` in `mod.rs` from Phase 0 Task 0.2. Verify it's reachable from `lib.rs` (same crate). If it was accidentally declared `pub(super)`, bump it to `pub(crate)`.

- [ ] **Step 2: Smoke test**

Run: `just dev`

1. Open a video
2. Close the main window (×)
3. Expected: video window and controls window both close, mpv cleanup logs appear, app exits cleanly with no hung threads

### Task 12.3: Commit Phase 12

- [ ] **Step 1: Stage and commit**

Run:
```bash
git add crates/tauri/src/lib.rs crates/tauri/src/player/mod.rs
git commit -m "$(cat <<'MSG'
fix(player): tear down player on app Exit

RunEvent::Exit now calls close_player_inner before cache flush so
mpv, GL context, video HWND, and controls window are all cleaned
up when the user closes the main window with the player open.

Co-Authored-By: Claude Opus 4.6 (1M context) <noreply@anthropic.com>
MSG
)"
```

---

## Phase 13: Final verification

Run the full manual checklist from spec §8.2 L0 + L1 + L2. Fix any regressions. Tag the branch as ready for merge.

### Task 13.1: Run automated checks

- [ ] **Step 1: `just check`**

Run: `just check`

Expected: lint + typecheck + all Rust tests pass. This invokes clippy, ESLint, `bunx tsc --noEmit`, and `cargo test`.

- [ ] **Step 2: Fix any warnings**

Treat new clippy warnings as errors. Typical suspects: unused variables in Windows cfg branches (fix with `let _ = ...;`), dead code in macOS cfg branches (fix with `#[allow(dead_code)]` or deletion).

### Task 13.2: Manual L0 checklist

- [ ] Open a video from Library → video window + controls window both appear
- [ ] Video plays (picture + audio)
- [ ] Play/pause button toggles
- [ ] Progress bar drag seeks
- [ ] Volume slider adjusts
- [ ] Tracks panel opens, switching audio/subtitle tracks takes effect

### Task 13.3: Manual L1 checklist

- [ ] Controls background is liquid glass (`blur(40px) saturate(180%)`, not opaque)
- [ ] Controls have 14px rounded corners
- [ ] Dragging the top strip moves both windows
- [ ] Resizing the video window (edges) resizes both
- [ ] Minimize button minimizes both windows
- [ ] Maximize toggles between Maximized and Normal
- [ ] Close button closes both windows and tears down mpv
- [ ] F key enters fullscreen, F again or Esc exits
- [ ] Double-click video area toggles fullscreen
- [ ] Frontend fullscreen button (right side of controls) also toggles
- [ ] In fullscreen, mouse rest 3s → controls fade; move → reappear
- [ ] In Normal/Maximized, controls stay visible permanently
- [ ] Space / arrows / M / Esc keys work as documented
- [ ] Subtitle overlay (ASS merge) renders on top of video

### Task 13.4: Manual L2 checklist

- [ ] Open → close → open same video (no leak; tracing shows clean teardown)
- [ ] Open → close → open different video (correct state carry-over)
- [ ] Close main window while playing → all clean
- [ ] Do other work in main window (download, TMDB) while video plays → video doesn't stutter
- [ ] (Multi-monitor) Drag to 2nd monitor → F → fullscreens on 2nd monitor
- [ ] (HiDPI) On 150% / 200% monitor: window dimensions correct, text crisp
- [ ] (Windows 11) DWM rounded corners visible on video window

### Task 13.5: Final commit (if fixes needed)

- [ ] **Step 1: If you had to fix any regressions during the checklist**

Commit them as `fix(player/win): <what-you-fixed>`.

### Task 13.6: Push the branch

- [ ] **Step 1: Push to remote**

Run: `git push -u origin feat/windows-player-native-window`

- [ ] **Step 2: Stop before merging**

Do **not** automatically merge or open a PR. Report completion back to the user and let them decide when to merge back into `refactor/workspace-core-server`.

---

## Appendix A: Tracing / debug tips

- `EnvFilter` default from `lib.rs:24-25` is `blowup_lib=debug`. To watch the new Windows player code specifically, set `RUST_LOG=blowup_lib::player::windows=trace`.
- In devtools of the `player-controls` window, add `localStorage.setItem("PLAYER_DEBUG", "1")` and it'll log every invoke / event (if you add the hook — optional, not in the plan).
- Windows-only: to see `WM_*` messages in Spy++, attach to `BlowupVideoWindow` class (the class name registered in `ensure_video_wnd_class`).

## Appendix B: Known gotchas

- **`WS_EX_NOACTIVATE` + `show()`**: if you `build()` the controls window with `visible(false)` and then call `show()`, the ex-style set via `SetWindowLongPtrW` must be applied **before** `show()`, otherwise Windows sometimes treats the first `show()` as an activation. Already handled in `controls::create`.
- **Race between `WM_CLOSE` and `cleanup_player_resources`**: the C WndProc returns early on WM_CLOSE without calling `DestroyWindow` — Rust calls `blowup_destroy_video_window` from the cleanup path, which calls `DestroyWindow`. If you reverse this order, mpv will be torn down against a freed GL context.
- **Double-enter fullscreen**: `blowup_enter_fullscreen` early-returns if `is_fullscreen == 1`, but `Rust toggle()` still reads state from C via `blowup_is_fullscreen` before deciding. If C state is wrong (because a crash or a missed `SetWindowPlacement`), toggle will be confused. Always prefer C as source of truth; Rust's `LOGICAL_STATE` is only for frontend broadcasts.
- **mpv render on a detached GL child**: if `blowup_attach_to_window` is called before `blowup_create_gl_view`, a NULL view will be returned. The order is enforced in `setup_gl_and_mpv_on_video_window` — GL view first, then attach, then mpv.
