# Windows 播放器：原生视频窗 + Tauri 浮动控件子窗

**日期**: 2026-04-12
**状态**: 待批准
**范围**: 仅 Windows 平台；macOS 代码一行不改
**分支**: `feat/windows-player-native-window`

---

## 1. 背景

### 1.1 现状

blowup 的嵌入式 mpv 播放器在 macOS 上工作正常：`BlowupGLView`（含 `MpvOpenGLLayer`）作为 `NSView` 子视图插到 Tauri 的 `contentView` 里、位于 `WKWebView` 之下，再把 WKWebView 的 `drawsBackground` 设为 NO，整个合成由 Core Animation 在同一 layer tree 里自然完成。视频在透明 WebView 背后，控件叠在视频之上，auto-hide、液态玻璃、圆角、双击全屏全部能做。

Windows 上这套方案行不通。Tauri v2 使用 WebView2（Chromium），其渲染目标是 **DirectComposition surface**，而我们创建的 GL child 是 `WS_CHILD` 的传统 GDI/OpenGL HWND。DComp 面和 GDI 子窗口分属两个合成管线，z-order 跨体系无效：

- 不透明窗口：WebView2 的 DComp 输出会完全盖住 GL 子窗，视频不可见
- 透明窗口 + 透明 HTML body：GL 子窗能被看到，但只能占客户区的一部分

现有 workaround（`crates/tauri/native/win_gl_layer.c` + `src/Player.tsx`）的做法是：

- 视频 GL child 只占 `client_height - CONTROLS_HEIGHT(100px)` 的上方区域
- 底部 100px 留给 WebView2 渲染控件条，不被 GL 覆盖
- GL child 的 `WM_NCHITTEST` 返回 `HTTRANSPARENT`，鼠标事件穿透到父 HWND 由 WebView2 接收

这套方案的代价很重：

- 控件条**不能 auto-hide**（隐藏后会露出空的 WebView2 底色）
- 控件条**不能用液态玻璃**（被硬改成不透明黑条 `rgba(18,18,20,0.98)`）
- 控件条**没有圆角**（`radius:0`）
- **不能**在视频上方浮动任何 HTML 元素（字幕预览层、hover 提示层等）
- 视觉与 macOS 明显不一致，`Player.tsx` 里有 20+ 处 `IS_WINDOWS` 分支

### 1.2 目标

把 Windows 的视频窗改成**由 Rust 直接创建的原生顶级 HWND**，mpv 嵌入其中、**完全不经过 WebView2**。控件以**独立的 Tauri WebView 顶级窗口**形态浮在视频窗上，两个顶级窗口由 OS 合成叠加。IPC 通过现有 `cmd_player_*` 命令和 `player-state` 事件继续工作。

成功标准：

- Windows 上视觉与 macOS 等齐（液态玻璃、圆角、auto-hide、浮动控件）
- IPC 层零撕裂（14 个现有 `cmd_player_*` 命令签名不变）
- macOS 代码一行不改
- 能删除所有 `Player.tsx` 里的 `IS_WINDOWS` 分支
- 鼠标 / 键盘 / 拖动 / 全屏 / 多显示器 / HiDPI / Windows 11 DWM 圆角全部工作

### 1.3 非目标

- Linux 支持
- 独占全屏（`ChangeDisplaySettings`）
- 从任务管理器强杀的清理兜底
- 不同 WebView2 / libmpv 版本兼容性矩阵
- 自动 GUI 测试

---

## 2. 窗口模型

Windows 上播放器由**两个顶级 HWND**组成，通过 OS 合成叠加：

```
┌─ Video Window (native, Rust-created) ────────────┐
│  HWND: WS_POPUP | WS_THICKFRAME                  │
│  自己 CreateWindowExW,不经过 Tauri                │
│  contentView = 全尺寸 GL child (HWND_TOP)        │
│  mpv 通过 render API 绘制到 GL FBO               │
│  WM_NCHITTEST → 边缘 resize / 客户区 HTCLIENT    │
│  WM_LBUTTONDBLCLK → 直接切全屏                    │
│  WM_MOUSEMOVE → 节流 50ms 后发事件给控件窗        │
│  WM_SIZE / WM_MOVE → 驱动控件窗重新定位           │
│  WM_KEYDOWN → 捕获快捷键直接调播放器命令          │
│  WM_CLOSE → 触发 cleanup_player_resources         │
└──────────────────────────────────────────────────┘
    ↕ Rust 侧全局绑定 (PLAYER_HWND + controls label)
┌─ Controls Window (Tauri WebviewWindow) ──────────┐
│  label: "player-controls"                        │
│  decorations(false), skip_taskbar(true)          │
│  always_on_top(true), transparent(true)          │
│  inner_size: 跟随视频窗宽度 × 100px 固定高度       │
│  position: 视频窗底部                             │
│  加载 player.html → Player.tsx                   │
│  右上角 min/max/close 按钮                        │
│  顶部拖动区通过 invoke 驱动视频窗拖动             │
│  普通的 invoke("cmd_player_*") / listen(...)     │
└──────────────────────────────────────────────────┘
```

**为什么控件是独立 top-level 而不是视频窗的 child HWND：** 独立顶级窗口由 DWM 合成，避免 WebView2 DComp 和 Win32 子 HWND 的同窗合成冲突。代价是自己维护"跟随"逻辑（`WM_MOVE/WM_SIZE → SetWindowPos`），但比合成冲突可控。

**主窗口（main label）不受影响**：继续跑 `App.tsx`，播放器打开时可保持可见 / 最小化 / 被遮挡，由用户自己决定。

---

## 3. 代码结构

### 3.1 Rust 模块

```
crates/tauri/src/player/
├── mod.rs                # 跨平台公共部分：MpvPlayer、get_state、render
│                         # callback、cleanup_player_resources、cfg 分发
├── ffi.rs                # 不变
├── native.rs             # 不变
├── commands.rs           # 扩展：新增窗口管理/鼠标通知命令,改 toggle_fullscreen
├── macos.rs              # 新建：现 mod.rs 里的 macOS 代码搬过来
│                         # (run_open_player_phases / setup_gl_and_mpv_on_main)
└── windows/              # 新建:Windows 专属子模块
    ├── mod.rs            # open_player / close_player / 全局状态
    │                     # (PLAYER_HWND, CONTROLS_WINDOW_READY)
    ├── video_window.rs   # 视频窗创建 / 消息转发 / 快捷键派发
    ├── controls.rs       # 控件子窗创建 / 跟随 / 关闭联动
    └── fullscreen.rs     # 全屏状态机 + WINDOWPLACEMENT 保存/还原
```

`mod.rs` 分发：

```rust
#[cfg(target_os = "windows")]
pub use windows::{open_player, close_player};
#[cfg(target_os = "macos")]
pub use macos::{open_player, close_player};
```

**关键原则**：macOS 代码从 `mod.rs` 原样搬到 `macos.rs`，**不做任何重构**。Windows 走完全独立的新路径。`MpvPlayer` 结构体、`cleanup_player_resources` 中的 mpv 清理部分保持在 `mod.rs`（两平台共享）。

### 3.2 原生 C 层

`crates/tauri/native/win_gl_layer.c` + `win_gl_layer.h` 扩展（293 行 → 约 550 行），**不拆分新文件**。

新增导出函数：

```c
// 顶级视频窗创建与销毁
void* blowup_create_video_window(double width, double height);
void  blowup_destroy_video_window(void* hwnd);
void  blowup_get_video_window_rect(void* hwnd, int* x, int* y, int* w, int* h);
void  blowup_set_video_window_rect(void* hwnd, int x, int y, int w, int h);

// 全屏切换（保存/还原 placement）
int   blowup_enter_fullscreen(void* hwnd);
int   blowup_leave_fullscreen(void* hwnd);
int   blowup_is_fullscreen(void* hwnd);

// 窗口控制
void  blowup_window_minimize(void* hwnd);
void  blowup_window_toggle_maximize(void* hwnd);
void  blowup_window_start_drag(void* hwnd);

// Windows 11 DWM 圆角
void  blowup_apply_round_corners(void* hwnd);
```

`blowup_attach_to_window(parent_hwnd, view_ptr)` **签名不变**，调用方从 Tauri HWND 换成 `blowup_create_video_window` 返回的 HWND。

**新增的 C → Rust 回调**，用来转发窗口事件：

```c
// Rust 侧导出,C 侧声明为 extern:
//   event_type:
//     0 = move    (x, y 是新位置,w/h 是尺寸)
//     1 = size    (w, h 是新尺寸,x/y 是位置)
//     2 = mousemove (x, y 是鼠标坐标,节流后才上报)
//     3 = dblclick  (x, y 是鼠标坐标)
//     4 = close
//     5 = keydown (x = virtual key code)
//     6 = window-state-changed (x = 0 normal / 1 max / 2 fullscreen)
extern void blowup_on_video_window_event(int event_type, int x, int y, int w, int h);
```

Rust 侧在 `player::windows::video_window` 里实现这个 extern。

### 3.3 前端

`src/Player.tsx` 和 `src/player-main.tsx` 复用现有实现。`player.html` 复用现有入口（已在 `vite.config.ts` 第 24 行的 rollup inputs 里）。

改动：

- **删除**：所有 `IS_WINDOWS` 分支（~50 行），`CONTROLS_HEIGHT` 常量，视频 click 区 div
- **保留**：`glass` 设计 tokens 全部、auto-hide timer、进度条、音量、音轨字幕面板、keyboard shortcuts（但 keyboard 只在控件窗有焦点时生效，见 §4.5）
- **新增**：右上角三个窗口控制按钮（min / max / close）、顶部 8px 拖动触发区、`player:video-mouse-move` 监听、`player:window-state` 监听更新按钮图标

---

## 4. 运行时行为

### 4.1 Open 流程

```
open_player(app, file_path)
  1. close_player_inner(app)            // 幂等清理旧实例
  2. EVENT_LOOP_SHUTDOWN = false
  3. CURRENT_FILE_PATH = Some(file_path)
  4. async phase 1 [main thread]:
       hwnd = blowup_create_video_window(1280, 720)
       blowup_apply_round_corners(hwnd)  // Win11 DWM 圆角
       PLAYER_HWND = Some(hwnd)
  5. phase 2 [main thread]:
       view = blowup_create_gl_view(w, h)
       blowup_attach_to_window(hwnd, view)
       mpv init + render context
       observe properties
       loadfile
       spawn event loop thread (push-model property events)
  6. phase 3 [main thread]:
       let controls = WebviewWindowBuilder::new("player-controls",
           WebviewUrl::App("player.html".into()))
           .decorations(false).skip_taskbar(true)
           .always_on_top(true).transparent(true)
           .resizable(false).focused(false)
           .inner_size(w, 100.0)
           .position(x, y + h - 100)
           .build()?;
       设置 WS_EX_NOACTIVATE (通过 SetWindowLongPtrW)
       监听 CloseRequested → prevent_close → 发给视频窗 WM_CLOSE
       CONTROLS_WINDOW_READY = true
```

**与现有 macOS phase 结构的差异**：不再需要 500ms sleep 等 WebView2，因为 phase 1 创建纯 Win32 窗口，无 WebView2 参与。控件窗（WebView2）在 phase 3 才创建，即使初始化慢也不阻塞视频显示。

### 4.2 Close 流程

```
cleanup_player_resources()
  1. EVENT_LOOP_SHUTDOWN = true
  2. mpv_wakeup(raw_handle)
  3. 等 event loop 退出 (max 500ms)
  4. RENDER_CTX = None
  5. native::remove_gl_view()            // 现有
  6. MPV_HANDLE = None
  7. drop(MpvPlayer)                     // _render_ctx 先释放
  8. ✨ [Windows] 关闭控件子窗:
       if let Some(w) = app.get_webview_window("player-controls") {
           w.close().ok();
       }
       CONTROLS_WINDOW_READY = false
  9. ✨ [Windows] 销毁视频窗:
       blowup_destroy_video_window(PLAYER_HWND.take())
  10. CURRENT_FILE_PATH = None
```

**三种关闭触发路径，都走 cleanup：**

1. **控件窗右上角 × 按钮**：前端 `onClick → invoke("cmd_player_close_player")` → `close_player_inner` → cleanup。按钮走 Tauri 命令,**不**走 Tauri window close,因此控件窗的 `CloseRequested` 不会触发。
2. **视频窗 `WM_CLOSE`**（Alt+F4 / 系统菜单）→ C 层调 `blowup_on_video_window_event(event_type=4)` → Rust 侧触发 cleanup。
3. **主窗口关闭 / App 退出** → `RunEvent::Exit` → `lib.rs` 清理钩子里调 `close_player_inner`（**新增**,当前 `lib.rs:366-387` 里没有 player 清理）。

此外,如果控件窗因为异常原因收到 `CloseRequested`（例如用户通过 `Ctrl+Shift+W` 快捷键或其他 Tauri 内部机制）,监听 `WindowEvent::CloseRequested` → `api.prevent_close()` → 发送 `WM_CLOSE` 给视频窗走路径 2。确保控件窗不会先于视频窗消失。

**幂等性**：所有清理步骤对"已清理"状态 safe no-op。`PLAYER_HWND.take()` 保证 HWND 只销毁一次。

### 4.3 窗口跟随

C 层 WndProc 收到视频窗 `WM_MOVE` 或 `WM_SIZE` 后，调 `blowup_on_video_window_event(0 or 1, x, y, w, h)`。Rust 侧导出的这个函数里：

```rust
fn reposition_controls_window(x: i32, y: i32, w: i32, h: i32) {
    if let Some(app) = PLAYER_APP_HANDLE.get() {
        if let Some(controls) = app.get_webview_window("player-controls") {
            let _ = controls.set_position(PhysicalPosition { x, y: y + h - 100 });
            let _ = controls.set_size(PhysicalSize { width: w as u32, height: 100 });
        }
    }
}
```

`PLAYER_APP_HANDLE: OnceLock<AppHandle>` 在 `open_player` 第一次调用时初始化。

### 4.4 鼠标拖动

- 控件窗顶部 8px 触发区 `onMouseDown` → `invoke("cmd_player_window_start_drag")`
- Rust 侧对**视频窗** `SendMessage(WM_NCLBUTTONDOWN, HTCAPTION, lParam)`，Windows 进入拖动模态
- 视频窗的 `WM_MOVE` 事件在拖动过程中持续触发 → 控件窗自动跟随
- 拖动结束后两个窗口处于同步位置

**不使用** `data-tauri-drag-region`，因为它只能拖控件窗自己。

### 4.5 键盘事件

**坑**：控件窗是 `always_on_top + skip_taskbar + WS_EX_NOACTIVATE`，**不会被激活**、不接收键盘焦点。键盘焦点始终在视频窗上（原生 HWND），所以控件窗内的 React `keydown` listener 在 Windows 上**永远收不到事件**。

**解法**：快捷键只由视频窗的 WndProc 捕获 `WM_KEYDOWN` 处理，控件窗的前端 keydown listener 对 Windows 是死代码。

Windows VK 常量里 `VK_SPACE`/`VK_LEFT`/`VK_RIGHT`/`VK_UP`/`VK_DOWN`/`VK_ESCAPE` 存在；字母键没有 `VK_F`/`VK_M` 常量，直接用 `'F' as i32 = 0x46`、`'M' as i32 = 0x4D`。

Rust 侧映射表（`video_window.rs`）：

```rust
const VK_F: i32 = 0x46;
const VK_M: i32 = 0x4D;

match vk {
    VK_SPACE  => cmd_player_play_pause(),
    VK_LEFT   => cmd_player_seek_relative(-5.0),
    VK_RIGHT  => cmd_player_seek_relative(5.0),
    VK_UP     => cmd_player_set_volume((current + 5.0).min(100.0)),
    VK_DOWN   => cmd_player_set_volume((current - 5.0).max(0.0)),
    VK_F      => toggle_fullscreen(),
    VK_ESCAPE => leave_fullscreen_if_active(),
    VK_M      => toggle_mute(),
    _         => (),
}
```

前端 `Player.tsx:246-261` 的 keydown listener 在 Windows 上保留但不会触发（`WS_EX_NOACTIVATE` 保证）；macOS 继续用前端 listener。不需要 `document.hasFocus()` 或其他 guard。

### 4.6 全屏状态机

三个状态：Normal、Maximized、Fullscreen。

```rust
enum VideoWindowState {
    Normal,
    Maximized,
    Fullscreen {
        prev_was_maximized: bool,
        prev_placement: WINDOWPLACEMENT,
        prev_style: LONG,
        prev_exstyle: LONG,
    },
}
```

**Normal ↔ Maximized**：标准 `ShowWindow(SW_MAXIMIZE / SW_RESTORE)`。

**Normal → Fullscreen**：

1. 保存 `GetWindowPlacement` + `GetWindowLongW(GWL_STYLE/GWL_EXSTYLE)`
2. `SetWindowLongW(GWL_STYLE, WS_POPUP | WS_VISIBLE)`（去掉 `WS_THICKFRAME`，保留 `WS_VISIBLE`）
3. `MonitorFromWindow(hwnd, MONITOR_DEFAULTTONEAREST)` → `GetMonitorInfo` → 取 `rcMonitor`（整个屏幕，不是 `rcWork`，覆盖任务栏）
4. `SetWindowPos(hwnd, HWND_TOP, mi.rcMonitor.*, SWP_FRAMECHANGED | SWP_NOOWNERZORDER)`
5. 控件窗被 `WM_MOVE`/`WM_SIZE` 事件驱动自动跟随到新位置（屏幕底部）

**Maximized → Fullscreen**：先 `SW_RESTORE` 到 Normal（触发 `WM_MOVE`/`WM_SIZE`），再走 Normal → Fullscreen，但 `prev_was_maximized = true`。

**Fullscreen → 退出**：

1. 恢复 style/exstyle
2. `SetWindowPlacement` 回保存的值
3. 如果 `prev_was_maximized`，再 `ShowWindow(SW_MAXIMIZE)`

**状态广播**：每次状态变化调 `blowup_on_video_window_event(event_type=6, state_code, 0, 0, 0)`，Rust `app.emit_to("player-controls", "player:window-state", {state})`，前端更新 max 按钮图标 □↔❐、更新 auto-hide 策略。

### 4.7 Auto-hide

**规则**：

- **Normal / Maximized 状态**：控件栏常驻，不启用 auto-hide（避免"视频还在但控件栏消失导致露出桌面或任务栏"的视觉断层）
- **Fullscreen 状态**：启用 auto-hide
  - 收到 `player:video-mouse-move` → `setShowControls(true)` + reset timer
  - 3 秒后 timer 触发 → `setShowControls(false)`
  - 例外：`seeking === true` / `showTracks === true` / 控件栏自身 hover 时不隐藏
- **退出全屏时**：强制 `setShowControls(true)`

**Rust 节流**：`WM_MOUSEMOVE` 会每帧触发，`AtomicU64` 存 `last_mousemove_ms`，距离上次 >=50ms 才 `blowup_on_video_window_event(event_type=2, x, y, 0, 0)`。

---

## 5. IPC 契约

### 5.1 新增 Tauri 命令

| 命令 | 参数 | 行为 |
|---|---|---|
| `cmd_player_window_minimize` | — | Windows: `blowup_window_minimize(PLAYER_HWND)`；macOS: `window.minimize()` |
| `cmd_player_window_toggle_maximize` | — | Windows: `blowup_window_toggle_maximize`；macOS: `set_resizable` + `set_maximized` |
| `cmd_player_window_start_drag` | — | Windows: `blowup_window_start_drag`；macOS: no-op（native title bar 已处理） |

Auto-hide 无需 `cmd_player_notify_mouse_active` —— 控件窗内部鼠标移动由 `Player.tsx` 的 `onMouseMove` 直接 reset timer,视频窗鼠标移动由 `player:video-mouse-move` 事件推送,两者都不需要额外往返。

### 5.2 改动的命令

| 命令 | 改动 |
|---|---|
| `cmd_player_toggle_fullscreen` | Windows 走 `windows::fullscreen::toggle`;macOS 保持现有 `WebviewWindow::set_fullscreen` |

### 5.3 新增事件（后端 → 前端）

| 事件 | 目标窗口 | 触发时机 |
|---|---|---|
| `player:video-mouse-move` | `player-controls` | 视频窗 `WM_MOUSEMOVE` 节流后 |
| `player:window-state` | `player-controls` | 视频窗进入/退出 Normal/Max/Fullscreen |

### 5.4 不变

- 14 个现有 `cmd_player_*` 命令（签名和行为）
- `player-state` 事件（mpv 属性变化推送）
- `SubtitleOverlayConfig`、`TrackInfo`、`PlayerState` 类型

---

## 6. 依赖与构建

### 6.1 build.rs

`crates/tauri/build.rs` Windows 分支新增链接：

```rust
println!("cargo:rustc-link-lib=dwmapi");   // DWM 扩展,Win11 圆角
println!("cargo:rustc-link-lib=shcore");   // DPI awareness
```

已有：`opengl32`、`user32`、`gdi32`、`comctl32`。

### 6.2 Cargo 依赖

**不引入新 crate**。Windows 代码全部在 C 侧，Rust 只通过 `extern "C"` 调用，不直接用 `windows-sys` / `winapi`。

### 6.3 Tauri capabilities

需要检查 `crates/tauri/capabilities/*.json`，如果 ACL 按 window label 限制，需为 `player-controls` 添加与 `player`（macOS 用）同等的权限。实施第一步确认。

### 6.4 前端资源

`vite.config.ts` 不动（`player.html` 已在 rollup inputs）。
`player.html` 不动。
`src/player-main.tsx` 不动。

---

## 7. 错误处理

| 失败点 | 原因 | 处理 |
|---|---|---|
| `blowup_create_video_window` → NULL | `RegisterClassExW` / `CreateWindowExW` | `open_player` 返 `Err`,前端 toast |
| `blowup_attach_to_window` → -1 | GL child 创建或 WGL 失败 | 清理已创建的视频窗,返 `Err` |
| `mpv_create` / `mpv_initialize` 失败 | libmpv 缺失或版本不兼容 | 错误串含 mpv 返回码 + log |
| 控件窗 `WebviewWindowBuilder::build` 失败 | WebView2 未安装 / 资源冲突 | 视频窗保留可用,无控件栏(降级而非崩溃) |
| `reposition_controls_window` 找不到窗口 | 控件窗已关闭但事件在队列 | 静默 no-op |
| 视频窗 `WM_CLOSE` 时 mpv 正在加载 | — | 现有 `cleanup_player_resources` wait 500ms 保留 |

**原则**：

- 不 `unwrap()` FFI 调用
- 所有 Err 返回前清理已分配资源（HWND、GL view、mpv ctx）
- 所有失败 `tracing::error!` 带上下文

---

## 8. 测试

### 8.1 Rust 单测

- `player::windows::fullscreen`：状态机的纯函数部分（`PrevState` 保存/恢复逻辑）
- `player::windows::video_window`：`WM_MOUSEMOVE` 节流（`AtomicU64` + `now_ms - last > 50`）

### 8.2 手动 checklist

**L0 基础功能**

- [ ] 从 Library 页面双击视频 → 视频窗 + 控件窗同时出现
- [ ] 视频正常播放（画面、音频、字幕）
- [ ] 控件窗播放/暂停按钮切换状态
- [ ] 进度条拖动 seek
- [ ] 音量条调节
- [ ] 音轨/字幕面板打开并切换生效

**L1 视觉与交互**

- [ ] 控件窗背景是液态玻璃（半透明 + blur）
- [ ] 控件窗四角圆角
- [ ] 拖动控件窗顶部 → 两个窗口一起移动
- [ ] 视频窗边缘 resize,控件窗宽度自动跟随
- [ ] 右上角 min / max / close 按钮分别生效
- [ ] F 键 / 视频区双击 / 全屏按钮 三种方式都能进全屏
- [ ] 全屏下鼠标静止 3s → 控件栏淡出；移动 → 立即显示
- [ ] Esc 退出全屏,回到进入前的状态（Normal 或 Maximized）
- [ ] 空格键（无论哪个窗口活跃）触发播放/暂停
- [ ] 左右箭头 seek ±5s
- [ ] 字幕叠加层正常

**L2 稳定性**

- [ ] 开视频 → 关闭 → 重开同一视频（无泄漏）
- [ ] 开视频 → 关闭 → 开另一视频（状态正确）
- [ ] 播放中点主窗口 × → 全部正确清理
- [ ] 播放中在主窗口做其他操作 → 视频不卡
- [ ] 多显示器：拖到第二屏全屏进入第二屏
- [ ] HiDPI（150% / 200%）尺寸与字体清晰
- [ ] Windows 11 DWM 圆角生效

### 8.3 不做的测试

- 任务管理器强杀清理
- 自动 GUI 测试（Tauri e2e）
- 不同 WebView2 / libmpv 版本兼容性矩阵

---

## 9. Edge cases

1. **重复 `open_player`**：先 `close_player_inner` 完整清理（现有逻辑）
2. **控件窗 build 失败**：视频窗降级运行,log 警告,前端无 UI 但快捷键可用
3. **视频窗 WM_CLOSE 与控件窗 close 同时发生**：`cleanup_player_resources` 幂等
4. **全屏下点最小化按钮**：先 `leave_fullscreen` 再 `ShowWindow(SW_MINIMIZE)`
5. **拖动视频窗时按 F**：`WM_NCLBUTTONDOWN` 的模态循环期间按键不响应（OS 行为,不处理）
6. **视频窗被拖出屏幕边缘**：控件窗跟随,OS 允许（不处理）
7. **rapid open/close**：`PLAYER` mutex 串行化,event loop shutdown 有 500ms 超时
8. **loadfile 后解码失败**：mpv `MPV_EVENT_END_FILE`,event loop break,窗口保留打开（与 macOS 一致）
9. **控件窗被意外 CloseRequested**：监听 `WindowEvent::CloseRequested` + `api.prevent_close()` + 转发 `WM_CLOSE` 给视频窗统一关闭

---

## 10. 回滚计划

如果 L0 阶段发现架构根本问题：

1. `git checkout refactor/workspace-core-server` 切回原分支
2. `feat/windows-player-native-window` 分支保留但不合并
3. macOS 代码因为一行没改,不受影响

---

## 11. 实施顺序（草拟,详细步骤在 writing-plans 阶段展开）

1. **准备**：读 capabilities 配置,确认 ACL；将 `mod.rs` 的 macOS 部分抽到 `macos.rs`（纯移动,不改逻辑）
2. **C 层骨架**：`blowup_create_video_window` + 空的 WndProc + `blowup_destroy_video_window` + `blowup_on_video_window_event` extern 声明
3. **Rust Windows 模块骨架**：`windows/mod.rs` 里的 `open_player` 走 phase 1,先只创建视频窗,不建控件窗,验证空窗口能出现
4. **GL + mpv 接入**：phase 2 把 GL view 和 mpv 接入新视频窗,验证能播
5. **控件子窗**：phase 3 的 `WebviewWindowBuilder`,验证控件出现在正确位置
6. **跟随逻辑**：`WM_MOVE / WM_SIZE → reposition_controls_window`
7. **窗口管理命令**：min / max / close / start_drag 四套
8. **前端改动**：删 `IS_WINDOWS` 分支,加右上角按钮和拖动区
9. **全屏状态机**：`fullscreen.rs` + `WM_LBUTTONDBLCLK`
10. **键盘派发**：`WM_KEYDOWN` 捕获 + Rust 映射表
11. **鼠标 auto-hide**：`WM_MOUSEMOVE` 节流 + 前端事件监听
12. **HiDPI / 多显示器 / DWM 圆角**：`SetThreadDpiAwarenessContext` + `MonitorFromWindow` + `DwmSetWindowAttribute(DWMWCP_ROUND)`
13. **主窗口关闭清理**：`lib.rs` 的 `RunEvent::Exit` 里加 `close_player_inner`
14. **Rust 单测**：状态机 + 节流
15. **手动 checklist**：按 §8.2 顺序跑

---

## 12. 未决 / 需实施时确认

- Tauri capabilities 具体格式和是否需要按 label 授权 —— 第 1 步确认
- Tauri v2 的 `WebviewWindowBuilder` 是否支持 `focused(false)` —— 如不支持,用 `SetWindowLongPtrW(GWL_EXSTYLE, ... | WS_EX_NOACTIVATE)` 手动设置
- Windows 11 圆角的 DWM 属性值（`DWMWA_WINDOW_CORNER_PREFERENCE = 33`,`DWMWCP_ROUND = 2`）—— 实施时对照 MS 文档确认
