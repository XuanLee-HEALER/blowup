# Skill Bridge Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build a stdio MCP bridge (`blowup-mcp`) that proxies any MCP client (Claude Code, Cursor, Cline, Zed, Claude Desktop) to the desktop app over a Unix domain socket. The socket listener is gated by a session-only switch in Settings — by default the desktop exposes nothing extra.

**Architecture:** New workspace crate `crates/mcp/` — a stateless rmcp-based stdio process. Each tool call builds a hyper HTTP request and sends it through hyperlocal (Unix-socket HTTP transport) to the desktop app's axum router (reused 1:1 from `blowup-server`, no new routes for MVP). The desktop app extends AppContext-adjacent state with a `SkillBridgeState` struct holding a `JoinHandle` + `oneshot::Sender` + `socket_path`; opening the Settings switch binds a `UnixListener` and spawns `serve_unix(listener, ctx)`; closing or app exit shuts it down + unlinks.

**Tech Stack:** Rust, rmcp 1.x (`#[tool]` / `#[tool_router]` macros + stdio transport), hyper 1.x + hyperlocal 0.9 (Unix-socket HTTP client), tokio, axum (existing), tauri 2 (existing), React (existing).

**Spec:** `docs/superpowers/specs/2026-04-14-skill-bridge-design.md`

**Out of scope for this plan (deferred):**

- **Windows named pipe support** — The bridge and desktop side will both `cfg(unix)` for MVP. Windows clients will see "skill bridge not yet supported on Windows" in Settings. Adding named pipe support is a follow-up plan because hyperlocal doesn't cover Windows and we'd need a parallel `tokio::net::windows::named_pipe`-based code path.
- Auto-update for the bridge binary (re-install via Settings is enough)
- TLS, token, encryption (file permission `0600` on socket is the security boundary)

**Deviations from spec:**

- **Skill bridge state lives in a Tauri-side `SkillBridgeState` struct, not in `AppContext`.** Reason: `JoinHandle<()>` and `oneshot::Sender` are tokio-runtime types that don't belong in `blowup-core`. The spec said "extend AppContext", but crossing that crate boundary would force `blowup-core` to depend on tokio runtime types just for one Tauri-only field. Cleaner to keep this in `crates/tauri/src/skill_bridge/state.rs` and Tauri-`manage` it like other state.

---

## Phase 1 — Foundation: blowup-server Unix socket support

### Task 1: Add `crates/mcp` to workspace as empty placeholder

This task just makes the workspace aware of the new crate so subsequent `cargo build` doesn't fail. The crate will be filled in later phases.

**Files:**

- Create: `crates/mcp/Cargo.toml`
- Create: `crates/mcp/src/main.rs`
- Modify: `Cargo.toml` (root)

- [ ] **Step 1: Create empty `crates/mcp/src/main.rs`**

```rust
fn main() {
    eprintln!("blowup-mcp: not yet implemented");
}
```

- [ ] **Step 2: Create `crates/mcp/Cargo.toml`**

```toml
[package]
name = "blowup-mcp"
version = "0.1.0"
edition = "2024"

[[bin]]
name = "blowup-mcp"
path = "src/main.rs"

[dependencies]
```

- [ ] **Step 3: Add to workspace members**

Modify `Cargo.toml` (root):

```toml
[workspace]
members = [
    "crates/core",
    "crates/server",
    "crates/tauri",
    "crates/mcp",
]
resolver = "2"
```

- [ ] **Step 4: Verify it builds**

Run: `cargo build -p blowup-mcp`
Expected: `Finished` with no errors. Binary at `target/debug/blowup-mcp`.

- [ ] **Step 5: Run it once**

Run: `./target/debug/blowup-mcp`
Expected: `blowup-mcp: not yet implemented` on stderr.

- [ ] **Step 6: Commit**

```bash
git add Cargo.toml crates/mcp/
git commit -m "feat(mcp): scaffold blowup-mcp crate placeholder

First step of the skill bridge — adds an empty crates/mcp/ to the
workspace so subsequent commits can fill it in without broken
intermediate states.

Co-Authored-By: Claude Opus 4.6 (1M context) <noreply@anthropic.com>"
```

---

### Task 2: Add `serve_unix` function to blowup-server

This is the desktop-side socket listener. It reuses the existing axum router (`build_router`) verbatim — the only difference is the listener type.

**Files:**

- Modify: `crates/server/src/lib.rs`
- Modify: `crates/server/Cargo.toml` (no new deps; tokio already has the `net` feature)
- Test: `crates/server/tests/serve_unix.rs`

- [ ] **Step 1: Write the failing test**

Create `crates/server/tests/serve_unix.rs`:

```rust
//! Tests for blowup_server::serve_unix — verify that:
//! 1. We can bind a Unix socket and the file appears with 0600 perms
//! 2. A real HTTP request over hyperlocal hits the same router as TCP
//! 3. Sending the shutdown signal stops the task and leaves the
//!    socket file in place (cleanup is the caller's job)

#![cfg(unix)]

use blowup_core::AppContext;
use blowup_core::infra::events::EventBus;
use blowup_core::library::index::LibraryIndex;
use blowup_core::tasks::TaskRegistry;
use blowup_core::torrent::tracker::TrackerManager;
use blowup_server::AppState;
use http_body_util::BodyExt;
use hyper::Request;
use hyper::body::Bytes;
use hyper_util::client::legacy::Client;
use hyper_util::rt::TokioExecutor;
use hyperlocal::{UnixClientExt, UnixConnector, Uri};
use serial_test::serial;
use std::os::unix::fs::PermissionsExt;
use std::sync::Arc;
use tokio::sync::OnceCell;
use tokio::sync::oneshot;

const TEST_TOKEN: &str = "test-token";

async fn make_state() -> (AppState, tempfile::TempDir) {
    let tmp = tempfile::tempdir().unwrap();
    blowup_core::config::init_app_data_dir(tmp.path().to_path_buf());
    blowup_core::infra::cache::init_cache();

    let pool = sqlx::SqlitePool::connect(":memory:").await.unwrap();
    sqlx::migrate!("../core/migrations").run(&pool).await.unwrap();

    let library_root = tmp.path().join("library");
    std::fs::create_dir_all(&library_root).unwrap();
    let library_index = Arc::new(LibraryIndex::load(&library_root));

    let (tracker, _) = TrackerManager::load();
    let torrent = Arc::new(OnceCell::new());

    let state: AppState = AppContext::new(
        pool,
        library_index,
        Arc::new(tracker),
        torrent,
        EventBus::new(),
        TaskRegistry::new(),
        Arc::new(TEST_TOKEN.to_string()),
    );
    (state, tmp)
}

#[tokio::test]
#[serial]
async fn serve_unix_binds_and_routes() {
    let (state, tmp) = make_state().await;
    let socket_path = tmp.path().join("test.sock");

    let (shutdown_tx, shutdown_rx) = oneshot::channel();
    let path_clone = socket_path.clone();
    let task = tokio::spawn(async move {
        blowup_server::serve_unix(&path_clone, state, shutdown_rx)
            .await
            .unwrap();
    });

    // Wait for the socket file to appear (bind happens async)
    for _ in 0..50 {
        if socket_path.exists() {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(20)).await;
    }
    assert!(socket_path.exists(), "socket file not created");

    // Verify 0600 perms
    let meta = std::fs::metadata(&socket_path).unwrap();
    let mode = meta.permissions().mode() & 0o777;
    assert_eq!(mode, 0o600, "socket perms should be 0600, got {:o}", mode);

    // Make a real HTTP request over the socket
    let client: Client<UnixConnector, http_body_util::Full<Bytes>> =
        Client::builder(TokioExecutor::new()).build(UnixConnector);
    let url: hyper::Uri = Uri::new(&socket_path, "/api/v1/health").into();
    let req = Request::builder()
        .uri(url)
        .header("authorization", format!("Bearer {TEST_TOKEN}"))
        .body(http_body_util::Full::new(Bytes::new()))
        .unwrap();

    let resp = client.request(req).await.unwrap();
    assert_eq!(resp.status(), 200);

    // Drain body to make sure router actually responded
    let _body = resp.into_body().collect().await.unwrap().to_bytes();

    // Send shutdown
    shutdown_tx.send(()).unwrap();
    tokio::time::timeout(std::time::Duration::from_secs(2), task)
        .await
        .expect("task did not exit within 2s")
        .unwrap();
}
```

- [ ] **Step 2: Add hyperlocal + hyper-util dev-deps to server**

Modify `crates/server/Cargo.toml` `[dev-dependencies]` section:

```toml
hyperlocal = "0.9"
hyper = { version = "1", features = ["client"] }
hyper-util = { version = "0.1", features = ["client", "client-legacy", "http1", "tokio"] }
http-body-util = "0.1"
```

- [ ] **Step 3: Run the test to verify it fails**

Run: `cargo test -p blowup-server --test serve_unix -- --nocapture`
Expected: FAIL with `serve_unix` not found in `blowup_server`.

- [ ] **Step 4: Implement `serve_unix`**

Modify `crates/server/src/lib.rs` — add at the bottom:

```rust
/// Bind + serve the axum router on a Unix domain socket. Used by the
/// desktop app's "Skill bridge" feature: the same router as TCP, but
/// reachable only by processes that can open the socket file (which
/// is `chmod 0600` by the caller, gated by file system permissions
/// instead of a bearer token).
///
/// The caller is responsible for:
/// - creating the parent directory
/// - chmod 0600 on the socket file after bind
/// - removing any stale socket file before calling this
/// - removing the socket file after shutdown
///
/// The function exits cleanly when `shutdown` resolves (the caller
/// drops the sender or sends `()`).
#[cfg(unix)]
pub async fn serve_unix(
    socket_path: &std::path::Path,
    state: AppState,
    shutdown: tokio::sync::oneshot::Receiver<()>,
) -> std::io::Result<()> {
    use tokio::net::UnixListener;
    let listener = UnixListener::bind(socket_path)?;
    let router = build_router(state);
    axum::serve(listener, router)
        .with_graceful_shutdown(async move {
            let _ = shutdown.await;
        })
        .await
        .map_err(std::io::Error::other)
}
```

- [ ] **Step 5: Run the test to verify it passes**

Run: `cargo test -p blowup-server --test serve_unix -- --nocapture`
Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add crates/server/
git commit -m "feat(server): add serve_unix for skill-bridge socket transport

Reuses build_router 1:1 — Unix socket gets the same routes, auth,
and error handling as TCP. The caller owns the socket file lifecycle
(perms, cleanup) so the function stays focused on bind+serve.

Co-Authored-By: Claude Opus 4.6 (1M context) <noreply@anthropic.com>"
```

---

### Task 3: Add empty `routes/skill.rs` placeholder

This is a no-op now but reserves the module path for future skill-only endpoints (mentioned in spec under "Interface extension rules").

**Files:**

- Create: `crates/server/src/routes/skill.rs`
- Modify: `crates/server/src/routes/mod.rs`
- Modify: `crates/server/src/lib.rs`

- [ ] **Step 1: Create the placeholder router**

Create `crates/server/src/routes/skill.rs`:

```rust
//! Skill-workflow-only routes.
//!
//! Currently empty. As the MCP skill bridge evolves, this is where
//! we add endpoints that don't fit the general entries CRUD shape —
//! for example, full-text wiki search, batch tag operations, or
//! "find related entries by name fragment". These routes are mounted
//! under `/api/v1/skill/*` and are reachable from both TCP (with
//! bearer token) and the Unix socket (without).

use axum::Router;

use crate::state::AppState;

pub fn router() -> Router<AppState> {
    Router::new()
}
```

- [ ] **Step 2: Declare the module**

Modify `crates/server/src/routes/mod.rs` — add `pub mod skill;` alphabetically.

- [ ] **Step 3: Mount in `build_router`**

Modify `crates/server/src/lib.rs` — in `build_router`, add `.merge(routes::skill::router())` to the api Router chain (anywhere in the merge list).

- [ ] **Step 4: Verify it builds**

Run: `cargo build -p blowup-server`
Expected: `Finished` with no warnings.

- [ ] **Step 5: Run existing smoke tests to verify nothing broke**

Run: `cargo test -p blowup-server --test smoke`
Expected: All existing tests pass.

- [ ] **Step 6: Commit**

```bash
git add crates/server/src/routes/skill.rs crates/server/src/routes/mod.rs crates/server/src/lib.rs
git commit -m "feat(server): reserve routes/skill.rs for skill-only endpoints

Empty placeholder mounted at /api/v1/skill/*. Future PRs can fill
this in if MCP tools find that the entries CRUD doesn't cover all
their needs (full-text search, batch ops, etc).

Co-Authored-By: Claude Opus 4.6 (1M context) <noreply@anthropic.com>"
```

---

## Phase 2 — `blowup-mcp` core building blocks

### Task 4: Implement `socket.rs` — path resolution with env override

This is the function both the bridge and the desktop app must use to agree on where the socket lives.

**Files:**

- Create: `crates/mcp/src/lib.rs`
- Create: `crates/mcp/src/socket.rs`
- Modify: `crates/mcp/Cargo.toml`

- [ ] **Step 1: Add deps to `crates/mcp/Cargo.toml`**

```toml
[package]
name = "blowup-mcp"
version = "0.1.0"
edition = "2024"

[[bin]]
name = "blowup-mcp"
path = "src/main.rs"

[lib]
path = "src/lib.rs"

[dependencies]
anyhow = "1"
dirs = "5"

[dev-dependencies]
tempfile = "3"
serial_test = "3"
```

- [ ] **Step 2: Create `crates/mcp/src/lib.rs` re-exporting modules for tests**

```rust
//! blowup-mcp library exports — only used by tests. The binary
//! entry is in main.rs.

pub mod socket;
```

- [ ] **Step 3: Write the failing tests**

Create `crates/mcp/src/socket.rs`:

```rust
//! Socket path resolution and connection helper.
//!
//! Production code uses `default_socket_path()`; tests and debugging
//! can override the path via the `BLOWUP_MCP_SOCKET_OVERRIDE`
//! environment variable. The bridge binary and the desktop app's
//! Tauri command MUST both use `resolve_socket_path()` so they agree
//! on the same location (with or without the override).

use std::path::PathBuf;

const ENV_OVERRIDE: &str = "BLOWUP_MCP_SOCKET_OVERRIDE";

/// Default socket path on this OS. macOS: app-data dir; Linux: runtime
/// dir (auto-cleaned by systemd-tmpfiles), with HOME/.local/share
/// fallback.
#[cfg(target_os = "macos")]
pub fn default_socket_path() -> PathBuf {
    dirs::data_dir()
        .unwrap_or_else(|| PathBuf::from("/tmp"))
        .join("blowup")
        .join("skill.sock")
}

#[cfg(all(unix, not(target_os = "macos")))]
pub fn default_socket_path() -> PathBuf {
    if let Some(rt) = std::env::var_os("XDG_RUNTIME_DIR") {
        return PathBuf::from(rt).join("blowup").join("skill.sock");
    }
    dirs::data_local_dir()
        .unwrap_or_else(|| PathBuf::from("/tmp"))
        .join("blowup")
        .join("skill.sock")
}

#[cfg(not(unix))]
pub fn default_socket_path() -> PathBuf {
    // Windows / other targets are not yet supported — see the plan
    // header. We still return *something* so callers compile, but
    // the bridge and the desktop will both refuse to use it.
    PathBuf::from("blowup-skill-unsupported")
}

/// Returns `BLOWUP_MCP_SOCKET_OVERRIDE` if set and non-empty,
/// otherwise `default_socket_path()`. **Always** call this — never
/// `default_socket_path()` directly — so tests can inject a tempdir.
pub fn resolve_socket_path() -> PathBuf {
    if let Ok(s) = std::env::var(ENV_OVERRIDE) {
        if !s.is_empty() {
            return PathBuf::from(s);
        }
    }
    default_socket_path()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serial_test::serial;

    #[test]
    #[serial]
    fn override_takes_precedence() {
        // SAFETY: the serial_test attribute makes this section single-
        // threaded so set_var/remove_var don't race with other tests.
        unsafe {
            std::env::set_var(ENV_OVERRIDE, "/tmp/custom.sock");
        }
        assert_eq!(resolve_socket_path(), PathBuf::from("/tmp/custom.sock"));
        unsafe {
            std::env::remove_var(ENV_OVERRIDE);
        }
    }

    #[test]
    #[serial]
    fn empty_override_falls_back_to_default() {
        unsafe {
            std::env::set_var(ENV_OVERRIDE, "");
        }
        let p = resolve_socket_path();
        assert_ne!(p, PathBuf::from(""));
        assert!(p.to_string_lossy().contains("skill.sock"));
        unsafe {
            std::env::remove_var(ENV_OVERRIDE);
        }
    }

    #[test]
    #[serial]
    fn unset_override_uses_default() {
        unsafe {
            std::env::remove_var(ENV_OVERRIDE);
        }
        let p = resolve_socket_path();
        assert!(p.to_string_lossy().contains("skill.sock"));
    }
}
```

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test -p blowup-mcp socket::tests`
Expected: 3 tests pass.

- [ ] **Step 5: Commit**

```bash
git add crates/mcp/
git commit -m "feat(mcp): add cross-platform socket path resolution

resolve_socket_path() always reads BLOWUP_MCP_SOCKET_OVERRIDE first
so tests and debugging can inject a tempdir without rebuilding.
Both the bridge and the desktop tauri command call this function
to guarantee they agree on the path.

Co-Authored-By: Claude Opus 4.6 (1M context) <noreply@anthropic.com>"
```

---

### Task 5: Implement `error.rs` — `McpError` type and conversions

Defines the 4-layer error model from the spec. The bridge maps every kind of failure into one of `BridgeOffline / Internal / BadRequest / NotFound`, sets the FATAL prefix correctly, and only marks `BadRequest`/`NotFound` as retryable.

**Files:**

- Create: `crates/mcp/src/error.rs`
- Modify: `crates/mcp/src/lib.rs`

- [ ] **Step 1: Add the module declaration**

Modify `crates/mcp/src/lib.rs`:

```rust
//! blowup-mcp library exports — only used by tests. The binary
//! entry is in main.rs.

pub mod error;
pub mod socket;
```

- [ ] **Step 2: Write the failing tests**

Create `crates/mcp/src/error.rs`:

```rust
//! Bridge error model. See `docs/superpowers/specs/2026-04-14-skill-bridge-design.md`
//! section "Error handling" for the rationale.
//!
//! Every failure path the bridge can encounter maps into one of four
//! `ErrorCode` variants. The two non-retryable variants (BridgeOffline,
//! Internal) get a `[FATAL] ` prefix on their message so Claude's
//! skill instructions can pattern-match and stop instead of looping.
//!
//! L3 errors (BadRequest, NotFound) carry an optional `hint` string
//! that the bridge hard-codes per tool — Claude reads it and adjusts
//! parameters before its single retry.

use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErrorCode {
    /// L1 — couldn't reach the desktop app at all (socket missing,
    /// permission denied, connection refused).
    BridgeOffline,
    /// L2 — connection succeeded but the response was unparseable
    /// or the server returned 5xx.
    Internal,
    /// L3 — server returned 4xx that's not 404. Includes validation
    /// errors and stale-state conflicts.
    BadRequest,
    /// L3 — server returned 404. Caller usually fixes by querying
    /// list_* first.
    NotFound,
}

impl ErrorCode {
    pub fn retryable(self) -> bool {
        matches!(self, ErrorCode::BadRequest | ErrorCode::NotFound)
    }

    pub fn is_fatal(self) -> bool {
        !self.retryable()
    }
}

#[derive(Debug, Clone)]
pub struct McpError {
    pub code: ErrorCode,
    pub message: String,
    pub hint: Option<String>,
}

impl McpError {
    pub fn bridge_offline() -> Self {
        Self {
            code: ErrorCode::BridgeOffline,
            message: "blowup app 未启用 skill bridge,请在 desktop 设置中打开 'Skill Bridge' 开关后重试".to_string(),
            hint: None,
        }
    }

    pub fn internal(detail: impl Into<String>) -> Self {
        Self {
            code: ErrorCode::Internal,
            message: format!("blowup app 内部错误: {}", detail.into()),
            hint: None,
        }
    }

    pub fn bad_request(message: impl Into<String>, hint: Option<String>) -> Self {
        Self {
            code: ErrorCode::BadRequest,
            message: message.into(),
            hint,
        }
    }

    pub fn not_found(message: impl Into<String>, hint: Option<String>) -> Self {
        Self {
            code: ErrorCode::NotFound,
            message: message.into(),
            hint,
        }
    }

    /// The exact string Claude sees as the MCP tool error. Fatal
    /// errors get a `[FATAL] ` prefix; retryable errors include the
    /// hint (if any) inline so Claude doesn't have to query a
    /// separate field.
    pub fn user_message(&self) -> String {
        let prefix = if self.code.is_fatal() { "[FATAL] " } else { "" };
        match &self.hint {
            Some(h) => format!("{prefix}{}\n提示: {}", self.message, h),
            None => format!("{prefix}{}", self.message),
        }
    }
}

impl fmt::Display for McpError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.user_message())
    }
}

impl std::error::Error for McpError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fatal_errors_get_fatal_prefix() {
        let e = McpError::bridge_offline();
        assert!(e.user_message().starts_with("[FATAL] "));

        let e = McpError::internal("db locked");
        assert!(e.user_message().starts_with("[FATAL] "));
    }

    #[test]
    fn retryable_errors_have_no_fatal_prefix() {
        let e = McpError::bad_request("条目名称为空", None);
        assert!(!e.user_message().starts_with("[FATAL]"));
        assert_eq!(e.user_message(), "条目名称为空");
    }

    #[test]
    fn hint_is_appended_inline() {
        let e = McpError::not_found(
            "条目 #999 不存在",
            Some("请先用 list_entries 查询".to_string()),
        );
        let msg = e.user_message();
        assert!(msg.contains("条目 #999 不存在"));
        assert!(msg.contains("请先用 list_entries 查询"));
    }

    #[test]
    fn retryable_classification() {
        assert!(ErrorCode::BadRequest.retryable());
        assert!(ErrorCode::NotFound.retryable());
        assert!(!ErrorCode::BridgeOffline.retryable());
        assert!(!ErrorCode::Internal.retryable());
    }
}
```

- [ ] **Step 3: Run the tests**

Run: `cargo test -p blowup-mcp error::tests`
Expected: 4 tests pass.

- [ ] **Step 4: Commit**

```bash
git add crates/mcp/src/error.rs crates/mcp/src/lib.rs
git commit -m "feat(mcp): add 4-layer McpError model with FATAL prefix

L1/L2 errors are non-retryable and get [FATAL] prefix so the skill
file can pattern-match and stop instead of looping. L3 errors
(4xx) carry an optional hint that's appended inline to the user
message.

Co-Authored-By: Claude Opus 4.6 (1M context) <noreply@anthropic.com>"
```

---

### Task 6: Implement `client.rs` — hyperlocal HTTP client wrapper

A small wrapper that takes (method, path, optional body) and returns deserialized JSON or an `McpError`. This is what every tool will call.

**Files:**

- Create: `crates/mcp/src/client.rs`
- Modify: `crates/mcp/src/lib.rs`
- Modify: `crates/mcp/Cargo.toml`

- [ ] **Step 1: Add deps**

Modify `crates/mcp/Cargo.toml` `[dependencies]` section:

```toml
[dependencies]
anyhow = "1"
dirs = "5"
serde = { version = "1", features = ["derive"] }
serde_json = "1"
tokio = { version = "1", features = ["macros", "rt-multi-thread", "io-util", "io-std", "net", "sync"] }
hyper = { version = "1", features = ["client", "http1"] }
hyper-util = { version = "0.1", features = ["client", "client-legacy", "http1", "tokio"] }
hyperlocal = "0.9"
http-body-util = "0.1"
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["env-filter"] }
```

- [ ] **Step 2: Add module declaration**

Modify `crates/mcp/src/lib.rs`:

```rust
//! blowup-mcp library exports — only used by tests. The binary
//! entry is in main.rs.

pub mod client;
pub mod error;
pub mod socket;
```

- [ ] **Step 3: Create `client.rs` (with embedded U1 unit test)**

Create `crates/mcp/src/client.rs`:

```rust
//! Stateless HTTP client over a Unix domain socket.
//!
//! Every tool call instantiates one of these on demand — Unix socket
//! connect cost is ~zero so there's no point keeping a pool, and a
//! fresh connection means "open the switch" / "close the switch"
//! takes effect immediately.
//!
//! The client returns deserialized JSON or an `McpError` mapped from
//! the exact failure: connect refused → BridgeOffline; 4xx → BadRequest
//! / NotFound (with a hint provided by the caller); 5xx or body parse
//! → Internal.

use crate::error::McpError;
use crate::socket::resolve_socket_path;
use http_body_util::{BodyExt, Full};
use hyper::body::Bytes;
use hyper::{Method, Request, StatusCode};
use hyper_util::client::legacy::Client;
use hyper_util::rt::TokioExecutor;
use hyperlocal::{UnixClientExt, UnixConnector, Uri as UnixUri};
use serde::de::DeserializeOwned;
use serde::Serialize;
use std::path::PathBuf;

pub struct BlowupClient {
    socket_path: PathBuf,
    inner: Client<UnixConnector, Full<Bytes>>,
}

impl BlowupClient {
    pub fn new() -> Self {
        Self {
            socket_path: resolve_socket_path(),
            inner: Client::builder(TokioExecutor::new()).build(UnixConnector),
        }
    }

    /// Optional hint passed by callers — appended to L3 error messages.
    pub async fn get<T: DeserializeOwned>(
        &self,
        path: &str,
        hint: Option<&str>,
    ) -> Result<T, McpError> {
        self.send::<(), T>(Method::GET, path, None, hint).await
    }

    pub async fn post<B: Serialize, T: DeserializeOwned>(
        &self,
        path: &str,
        body: &B,
        hint: Option<&str>,
    ) -> Result<T, McpError> {
        self.send(Method::POST, path, Some(body), hint).await
    }

    pub async fn put<B: Serialize, T: DeserializeOwned>(
        &self,
        path: &str,
        body: &B,
        hint: Option<&str>,
    ) -> Result<T, McpError> {
        self.send(Method::PUT, path, Some(body), hint).await
    }

    async fn send<B: Serialize, T: DeserializeOwned>(
        &self,
        method: Method,
        path: &str,
        body: Option<&B>,
        hint: Option<&str>,
    ) -> Result<T, McpError> {
        let uri: hyper::Uri = UnixUri::new(&self.socket_path, path).into();
        let body_bytes = match body {
            Some(b) => serde_json::to_vec(b)
                .map_err(|e| McpError::internal(format!("serialize body: {e}")))?,
            None => Vec::new(),
        };

        let mut req = Request::builder().method(method).uri(uri);
        if !body_bytes.is_empty() {
            req = req.header("content-type", "application/json");
        }
        let req = req
            .body(Full::new(Bytes::from(body_bytes)))
            .map_err(|e| McpError::internal(format!("build request: {e}")))?;

        let resp = self
            .inner
            .request(req)
            .await
            .map_err(|_| McpError::bridge_offline())?;

        let status = resp.status();
        let body = resp
            .into_body()
            .collect()
            .await
            .map_err(|e| McpError::internal(format!("read body: {e}")))?
            .to_bytes();

        if status.is_success() {
            // Some endpoints return empty bodies — handle "()" by
            // accepting empty and synthesizing `null` for serde.
            if body.is_empty() {
                return serde_json::from_slice(b"null")
                    .map_err(|e| McpError::internal(format!("deserialize empty: {e}")));
            }
            return serde_json::from_slice(&body)
                .map_err(|e| McpError::internal(format!("deserialize: {e}")));
        }

        let text = String::from_utf8_lossy(&body).to_string();
        let hint_owned = hint.map(|h| h.to_string());
        Err(match status {
            StatusCode::NOT_FOUND => McpError::not_found(text, hint_owned),
            s if s.is_client_error() => McpError::bad_request(text, hint_owned),
            _ => McpError::internal(format!("HTTP {status}: {text}")),
        })
    }
}

impl Default for BlowupClient {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serial_test::serial;

    /// U1: spin up an in-process axum server bound to a tempdir
    /// Unix socket and verify the client roundtrips JSON correctly.
    #[tokio::test]
    #[serial]
    async fn client_roundtrips_get_json() {
        use axum::routing::get;
        use axum::Router;
        use serde::Deserialize;
        use tokio::sync::oneshot;

        #[derive(Serialize, Deserialize, PartialEq, Debug)]
        struct Echo {
            value: i32,
        }

        let tmp = tempfile::tempdir().unwrap();
        let socket_path = tmp.path().join("test.sock");
        unsafe {
            std::env::set_var("BLOWUP_MCP_SOCKET_OVERRIDE", &socket_path);
        }

        let router: Router = Router::new()
            .route("/echo", get(|| async { axum::Json(Echo { value: 42 }) }));

        let listener = tokio::net::UnixListener::bind(&socket_path).unwrap();
        let (shutdown_tx, shutdown_rx) = oneshot::channel::<()>();
        let task = tokio::spawn(async move {
            axum::serve(listener, router)
                .with_graceful_shutdown(async move {
                    let _ = shutdown_rx.await;
                })
                .await
                .unwrap();
        });

        // Give the listener a beat to be ready
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;

        let client = BlowupClient::new();
        let echo: Echo = client.get("/echo", None).await.unwrap();
        assert_eq!(echo, Echo { value: 42 });

        let _ = shutdown_tx.send(());
        let _ = tokio::time::timeout(std::time::Duration::from_secs(2), task).await;

        unsafe {
            std::env::remove_var("BLOWUP_MCP_SOCKET_OVERRIDE");
        }
    }

    #[tokio::test]
    #[serial]
    async fn client_returns_bridge_offline_when_socket_missing() {
        let tmp = tempfile::tempdir().unwrap();
        let socket_path = tmp.path().join("nope.sock");
        unsafe {
            std::env::set_var("BLOWUP_MCP_SOCKET_OVERRIDE", &socket_path);
        }

        let client = BlowupClient::new();
        let result: Result<serde_json::Value, _> = client.get("/anything", None).await;
        let err = result.unwrap_err();
        assert_eq!(err.code, crate::error::ErrorCode::BridgeOffline);
        assert!(err.user_message().starts_with("[FATAL]"));

        unsafe {
            std::env::remove_var("BLOWUP_MCP_SOCKET_OVERRIDE");
        }
    }
}
```

- [ ] **Step 4: Add `axum` and `tempfile` to dev-deps for the inline test**

Modify `crates/mcp/Cargo.toml` `[dev-dependencies]`:

```toml
[dev-dependencies]
tempfile = "3"
serial_test = "3"
axum = "0.8"
```

(Match the existing axum version used by `blowup-server` — check `crates/server/Cargo.toml` and use the same major.)

- [ ] **Step 5: Run the tests**

Run: `cargo test -p blowup-mcp client::tests`
Expected: 2 tests pass.

- [ ] **Step 6: Commit**

```bash
git add crates/mcp/
git commit -m "feat(mcp): add stateless hyperlocal client with error mapping

BlowupClient is the only thing tools touch — it gets a serializable
body and a typed response, mapping HTTP status codes to the correct
McpError variant. Empty success bodies are handled (PUT/DELETE with
no return value).

Co-Authored-By: Claude Opus 4.6 (1M context) <noreply@anthropic.com>"
```

---

## Phase 3 — rmcp service and tools

### Task 7: rmcp scaffold — minimal stdio server with one ping tool

This task **first verifies the actual rmcp 1.x API shape** (the macros below are based on the documented patterns but the executor MUST run a build to confirm), then wires up a minimal `BlowupService` with a single `ping` tool and a `main.rs` that runs it over stdio.

**Verification step**: rmcp's macros may use slightly different attribute names between minor versions. If the build in Step 4 fails with a macro-related error, look at:

- https://docs.rs/rmcp/latest/rmcp/
- https://github.com/modelcontextprotocol/rust-sdk/tree/main/examples/servers/src
  for the current `#[tool]` / `#[tool_router]` syntax. The structural shape (a struct with methods, each method = one tool) is stable; only the macro names/positions might shift.

**Files:**

- Create: `crates/mcp/src/service.rs`
- Modify: `crates/mcp/src/main.rs`
- Modify: `crates/mcp/src/lib.rs`
- Modify: `crates/mcp/Cargo.toml`

- [ ] **Step 1: Add rmcp + schemars to deps**

Modify `crates/mcp/Cargo.toml` `[dependencies]`:

```toml
rmcp = { version = "0.8", features = ["server", "transport-io", "macros", "schemars"] }
schemars = "1"
```

(Use whatever the current latest is — check `cargo search rmcp` first. As of writing the latest documented is 1.4.0, but version may have moved. The spec assumed `rmcp 0.x` so we cap loosely.)

- [ ] **Step 2: Verify the version actually exists**

Run: `cargo search rmcp --limit 5`
Expected: List shows `rmcp = "X.Y.Z"`. Update the `Cargo.toml` line to match the latest stable.

- [ ] **Step 3: Add module declaration**

Modify `crates/mcp/src/lib.rs`:

```rust
//! blowup-mcp library exports — only used by tests. The binary
//! entry is in main.rs.

pub mod client;
pub mod error;
pub mod service;
pub mod socket;
```

- [ ] **Step 4: Create the minimal service**

Create `crates/mcp/src/service.rs`:

```rust
//! Bridge MCP service — one struct, one `#[tool_router]` impl block,
//! one method per exposed tool. The struct carries a `BlowupClient`
//! that points at the desktop app's Unix socket; methods are
//! `async fn` and return `Result<T, McpError>`.

use crate::client::BlowupClient;
use crate::error::McpError;
use rmcp::{tool, tool_router, ServerHandler};

/// Minimal service used to verify the rmcp wiring before adding the
/// 9 real tools. After Tasks 8/9 the `ping` method goes away.
#[derive(Clone)]
pub struct BlowupService {
    client: BlowupClient,
}

impl BlowupService {
    pub fn new() -> Self {
        Self { client: BlowupClient::new() }
    }
}

#[tool_router]
impl BlowupService {
    /// 探测 desktop app 是否在线。无副作用,返回 desktop 的版本字符串。
    /// 调试 skill bridge 时用。
    #[tool]
    pub async fn ping(&self) -> Result<String, McpError> {
        // Hits GET /api/v1/health which the existing server already exposes
        let _: serde_json::Value = self.client.get("/api/v1/health", None).await?;
        Ok("ok".to_string())
    }
}

impl ServerHandler for BlowupService {}

impl Default for BlowupService {
    fn default() -> Self {
        Self::new()
    }
}
```

> **Note:** If `#[tool_router]` is not the exact macro name in the current rmcp version, search the rmcp docs for "tool router" / "ServerHandler" to find the equivalent. The pattern is: a struct, an impl block annotated to expose its async methods as tools, and a `ServerHandler` implementation. Some versions also require a `#[tool_handler]` attribute on the impl block instead of (or in addition to) `#[tool_router]`.

- [ ] **Step 5: Wire main.rs to run the service over stdio**

Modify `crates/mcp/src/main.rs`:

```rust
//! blowup-mcp — stdio MCP bridge to the blowup desktop app.
//!
//! IMPORTANT: All logging goes to stderr because stdout is the MCP
//! JSON-RPC channel. Mixing them corrupts every tool call.

use blowup_mcp::service::BlowupService;
use rmcp::transport::stdio;
use rmcp::ServiceExt;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Tracing → stderr (NEVER stdout)
    tracing_subscriber::fmt()
        .with_writer(std::io::stderr)
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("blowup_mcp=info")),
        )
        .init();

    tracing::info!("blowup-mcp starting");
    let service = BlowupService::new();
    let server = service.serve(stdio()).await?;
    server.waiting().await?;
    Ok(())
}
```

> **Note:** The exact `serve` and `waiting` method names depend on rmcp's `ServiceExt` trait. If the build fails, look at the rmcp examples directory for the canonical stdio server entry point.

- [ ] **Step 6: Build and verify**

Run: `cargo build -p blowup-mcp`
Expected: `Finished` with no errors. If errors mention rmcp macros, fix per the rmcp 1.x docs and update this plan inline before continuing.

- [ ] **Step 7: Smoke-run the binary**

Run: `echo '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2024-11-05","capabilities":{},"clientInfo":{"name":"manual","version":"0"}}}' | ./target/debug/blowup-mcp`
Expected: A JSON-RPC response on stdout. If the bridge hangs waiting for more input, that's also OK — Ctrl-C and confirm the JSON-RPC response appeared first.

- [ ] **Step 8: Commit**

```bash
git add crates/mcp/
git commit -m "feat(mcp): add rmcp service scaffold with ping tool

Verifies the stdio transport + tool router wiring works end-to-end
before adding the 9 real tools. The ping tool calls
GET /api/v1/health on the bridged desktop, which doubles as a
liveness check.

Co-Authored-By: Claude Opus 4.6 (1M context) <noreply@anthropic.com>"
```

---

### Task 8: Read tools — `list_entries`, `get_entry`, `list_all_tags`, `list_relation_types`

All 4 are GET requests. We define them together because they share the same shape (no body, query params or path params).

**Files:**

- Modify: `crates/mcp/src/service.rs`
- Modify: `crates/mcp/Cargo.toml`

- [ ] **Step 1: Add JSON schema deps if not already**

Verify `crates/mcp/Cargo.toml` already has `schemars = "1"` in `[dependencies]`. If not, add it.

- [ ] **Step 2: Add the 4 read tools**

Modify `crates/mcp/src/service.rs` — replace the `#[tool_router]` impl block with this expanded version. Keep `ping` for now; it'll be removed in Task 10.

```rust
use rmcp::schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct EntrySummary {
    pub id: i64,
    pub name: String,
    pub tags: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct EntryDetail {
    pub id: i64,
    pub name: String,
    pub wiki: String,
    pub tags: Vec<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct ListEntriesArgs {
    /// 模糊匹配条目名称的子串(可选)。例:"情书" 会匹配 "情书"、"情书 1995 重映版" 等。
    /// 不传则返回全部条目(可能很多,慎用)。
    pub query: Option<String>,
    /// 按标签过滤,精确匹配。例:"导演" 会只返回带 "导演" 标签的条目。
    /// 与 query 同时传时取交集。
    pub tag: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct GetEntryArgs {
    /// 条目 ID。从 list_entries 或 create_entry 的返回值取得。
    pub id: i64,
}

#[tool_router]
impl BlowupService {
    /// 探测 desktop app 是否在线 — 调试用。
    #[tool]
    pub async fn ping(&self) -> Result<String, McpError> {
        let _: serde_json::Value = self.client.get("/api/v1/health", None).await?;
        Ok("ok".to_string())
    }

    /// 列出知识库条目。可选按名称子串(query)和/或标签(tag)过滤。
    /// 写新条目前必须先用此工具查重 — 同名条目存在时应改为 update_wiki。
    #[tool]
    pub async fn list_entries(&self, args: ListEntriesArgs) -> Result<Vec<EntrySummary>, McpError> {
        let mut path = String::from("/api/v1/entries");
        let mut params: Vec<(String, String)> = Vec::new();
        if let Some(q) = args.query {
            params.push(("query".into(), q));
        }
        if let Some(t) = args.tag {
            params.push(("tag".into(), t));
        }
        if !params.is_empty() {
            let qs = params
                .into_iter()
                .map(|(k, v)| format!("{}={}", k, urlencoding::encode(&v)))
                .collect::<Vec<_>>()
                .join("&");
            path.push('?');
            path.push_str(&qs);
        }
        self.client.get(&path, None).await
    }

    /// 获取单个条目的完整内容(包括 wiki markdown、标签、关系)。
    /// 若返回 NotFound,先用 list_entries 查询正确的 ID。
    #[tool]
    pub async fn get_entry(&self, args: GetEntryArgs) -> Result<EntryDetail, McpError> {
        let path = format!("/api/v1/entries/{}", args.id);
        self.client
            .get(&path, Some("先用 list_entries 查询正确的 ID"))
            .await
    }

    /// 列出知识库中已使用的全部标签。写条目前调用一次,从中挑选最匹配的现有标签,
    /// 只在确实没有合适标签时才用 add_tag 创建新标签。
    /// 这避免标签碎片化("导演" / "电影导演" / "导演角色" 同时存在)。
    #[tool]
    pub async fn list_all_tags(&self) -> Result<Vec<String>, McpError> {
        self.client.get("/api/v1/entries/tags", None).await
    }

    /// 列出知识库中已使用的全部关系类型(如 "导演了"、"主演了"、"属于流派")。
    /// 调用 add_relation 前必须先用此工具查询,关系类型是用户自定义的字符串,
    /// 没有固定枚举,但要复用现有的而不是发明新的。
    #[tool]
    pub async fn list_relation_types(&self) -> Result<Vec<String>, McpError> {
        self.client
            .get("/api/v1/entries/relation-types", None)
            .await
    }
}
```

- [ ] **Step 3: Add `urlencoding` to deps**

Modify `crates/mcp/Cargo.toml` `[dependencies]`:

```toml
urlencoding = "2"
```

- [ ] **Step 4: Verify it builds**

Run: `cargo build -p blowup-mcp`
Expected: `Finished`. If rmcp macros complain about the args struct shape, the macro may need a different attribute style — verify against rmcp examples.

- [ ] **Step 5: Run all existing tests**

Run: `cargo test -p blowup-mcp`
Expected: All previous tests still pass.

- [ ] **Step 6: Commit**

```bash
git add crates/mcp/
git commit -m "feat(mcp): add 4 read tools (list_entries, get_entry, list_all_tags, list_relation_types)

Each tool's doc comment is the description Claude sees. Args structs
use #[derive(JsonSchema)] so rmcp generates the parameter schema
automatically. Tool descriptions include the workflow constraint
(\"call list_all_tags before add_tag\" etc).

Co-Authored-By: Claude Opus 4.6 (1M context) <noreply@anthropic.com>"
```

---

### Task 9: Write tools — `create_entry`, `update_wiki`, `update_name`, `add_tag`, `add_relation`

5 mutation tools. Each calls POST or PUT on the existing entries router. We also remove the `ping` debug tool here — it's served its purpose.

**Files:**

- Modify: `crates/mcp/src/service.rs`

- [ ] **Step 1: Add the 5 write tools (and remove ping)**

Modify `crates/mcp/src/service.rs` — extend the impl block with these tools, and delete the `ping` method:

```rust
#[derive(Debug, Deserialize, JsonSchema)]
pub struct CreateEntryArgs {
    /// 条目名称,中文,不含书名号 / 引号 / 年份后缀。
    /// 例:`情书` (✓), `《情书》` (✗), `情书 (1995)` (✗)
    /// 创建前必须先用 list_entries(query=name) 查重,
    /// 同名条目存在时应改为 update_wiki 而不是新建。
    pub name: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct UpdateWikiArgs {
    /// 条目 ID。从 list_entries 或 create_entry 的返回值取得。
    pub id: i64,
    /// Wiki 内容,Markdown 格式,中文。会**完全覆盖**条目现有的 wiki 内容,
    /// 不是 append。先用 get_entry 拿到现有 wiki 再合并,如果是更新而非新写。
    pub wiki: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct UpdateNameArgs {
    /// 条目 ID。
    pub id: i64,
    /// 新的条目名称,规则同 create_entry.name。
    pub name: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct AddTagArgs {
    /// 条目 ID。
    pub entry_id: i64,
    /// 标签字符串。优先使用 list_all_tags 返回的现有标签,
    /// 只在确实没有合适标签时才创建新标签。
    pub tag: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct AddRelationArgs {
    /// 关系起点条目 ID。
    pub from_id: i64,
    /// 关系终点条目 ID。
    pub to_id: i64,
    /// 关系类型字符串。必须先用 list_relation_types 查询现有类型并复用,
    /// 不要发明新类型(例如不要用 "拍了" 当 "导演了" 已经存在时)。
    pub relation_type: String,
}

// Add inside the existing #[tool_router] impl block, after the read tools:

    /// 创建一个新的知识库条目并返回其 ID。
    /// **调用前必须**先用 list_entries(query=name) 查重 —
    /// 同名条目存在时应改为 update_wiki,而不是新建。
    #[tool]
    pub async fn create_entry(&self, args: CreateEntryArgs) -> Result<i64, McpError> {
        #[derive(serde::Serialize)]
        struct Body<'a> {
            name: &'a str,
        }
        self.client
            .post(
                "/api/v1/entries",
                &Body { name: &args.name },
                Some("先用 list_entries(query=name) 查重"),
            )
            .await
    }

    /// 更新条目的 wiki markdown 内容。**完全覆盖**,不是 append。
    /// 若是更新而非新写,先用 get_entry 拿到现有内容再合并。
    #[tool]
    pub async fn update_wiki(&self, args: UpdateWikiArgs) -> Result<(), McpError> {
        #[derive(serde::Serialize)]
        struct Body<'a> {
            wiki: &'a str,
        }
        let path = format!("/api/v1/entries/{}/wiki", args.id);
        self.client
            .put(
                &path,
                &Body { wiki: &args.wiki },
                Some("条目 ID 不存在时,先用 list_entries 查询"),
            )
            .await
    }

    /// 更新条目的名称。规则同 create_entry。
    #[tool]
    pub async fn update_name(&self, args: UpdateNameArgs) -> Result<(), McpError> {
        #[derive(serde::Serialize)]
        struct Body<'a> {
            name: &'a str,
        }
        let path = format!("/api/v1/entries/{}/name", args.id);
        self.client
            .put(&path, &Body { name: &args.name }, None)
            .await
    }

    /// 给条目添加一个标签。优先使用 list_all_tags 返回的现有标签,
    /// 只在确实需要时创建新标签。
    #[tool]
    pub async fn add_tag(&self, args: AddTagArgs) -> Result<(), McpError> {
        #[derive(serde::Serialize)]
        struct Body<'a> {
            tag: &'a str,
        }
        let path = format!("/api/v1/entries/{}/tags", args.entry_id);
        self.client
            .post(
                &path,
                &Body { tag: &args.tag },
                Some("先用 list_all_tags 检查是否有合适的现有标签"),
            )
            .await
    }

    /// 在两个条目之间添加一条关系,返回关系 ID。
    /// **调用前必须**先用 list_relation_types 查询现有类型并复用,
    /// 不要发明同义的新类型。
    #[tool]
    pub async fn add_relation(&self, args: AddRelationArgs) -> Result<i64, McpError> {
        #[derive(serde::Serialize)]
        struct Body<'a> {
            from_id: i64,
            to_id: i64,
            relation_type: &'a str,
        }
        self.client
            .post(
                "/api/v1/entries/relations",
                &Body {
                    from_id: args.from_id,
                    to_id: args.to_id,
                    relation_type: &args.relation_type,
                },
                Some("先用 list_relation_types 查询现有类型并复用"),
            )
            .await
    }
```

- [ ] **Step 2: Build**

Run: `cargo build -p blowup-mcp`
Expected: `Finished`.

- [ ] **Step 3: Run all tests**

Run: `cargo test -p blowup-mcp`
Expected: All pass.

- [ ] **Step 4: Commit**

```bash
git add crates/mcp/src/service.rs
git commit -m "feat(mcp): add 5 write tools (create/update/add_tag/add_relation)

Each tool's doc comment includes the workflow constraint as plain
Chinese text — Claude sees this as the tool description and uses
it to plan correctly without trial-and-error. The ping debug tool
is removed; the bridge now exposes exactly the 9 entries tools.

Co-Authored-By: Claude Opus 4.6 (1M context) <noreply@anthropic.com>"
```

---

## Phase 4 — Cross-process smoke test (U3)

### Task 10: Add `tests/smoke.rs` with one read-roundtrip test

This is the closest we get to "Claude actually calls a tool" without launching Claude Code itself. The test spawns the real `blowup-mcp` binary as a subprocess, talks to it via stdin/stdout JSON-RPC, and verifies it correctly proxies through the Unix socket to a tempdir-mounted axum router.

**Files:**

- Create: `crates/mcp/tests/smoke.rs`
- Modify: `crates/mcp/Cargo.toml`

- [ ] **Step 1: Add tokio with `process` feature to dev-deps**

Modify `crates/mcp/Cargo.toml` `[dev-dependencies]`:

```toml
[dev-dependencies]
tempfile = "3"
serial_test = "3"
axum = "0.8"
tokio = { version = "1", features = ["macros", "rt-multi-thread", "process", "io-util", "net", "sync", "time"] }
```

- [ ] **Step 2: Write the smoke test**

Create `crates/mcp/tests/smoke.rs`:

```rust
//! Cross-process smoke test for blowup-mcp.
//!
//! Spawns the real bridge binary as a child process, mounts a minimal
//! axum router on a tempdir Unix socket, and sends JSON-RPC requests
//! through the bridge's stdin to verify the full chain
//! (stdio → rmcp → hyperlocal → axum) works end-to-end.

#![cfg(unix)]

use serde_json::{json, Value};
use std::process::Stdio;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::Command;
use tokio::sync::oneshot;

const BRIDGE_BIN: &str = env!("CARGO_BIN_EXE_blowup-mcp");

async fn start_test_router(socket_path: &std::path::Path) -> oneshot::Sender<()> {
    use axum::routing::get;
    use axum::Router;

    let router: Router = Router::new()
        .route("/api/v1/health", get(|| async { axum::Json(json!({ "ok": true })) }))
        .route("/api/v1/entries", get(|| async { axum::Json(json!([
            { "id": 1, "name": "测试条目", "tags": ["测试"] }
        ])) }));

    let listener = tokio::net::UnixListener::bind(socket_path).unwrap();
    let (tx, rx) = oneshot::channel::<()>();
    tokio::spawn(async move {
        axum::serve(listener, router)
            .with_graceful_shutdown(async move {
                let _ = rx.await;
            })
            .await
            .unwrap();
    });
    // Tiny pause for bind to settle
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    tx
}

#[tokio::test(flavor = "multi_thread")]
async fn end_to_end_list_entries_through_bridge() {
    let tmp = tempfile::tempdir().unwrap();
    let socket_path = tmp.path().join("test.sock");
    let _shutdown = start_test_router(&socket_path).await;

    // Spawn bridge with the override env
    let mut child = Command::new(BRIDGE_BIN)
        .env("BLOWUP_MCP_SOCKET_OVERRIDE", &socket_path)
        .env("RUST_LOG", "blowup_mcp=warn")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .spawn()
        .expect("spawn bridge");

    let stdin = child.stdin.as_mut().unwrap();
    let stdout = child.stdout.take().unwrap();
    let mut reader = BufReader::new(stdout).lines();

    // 1. initialize
    let init = json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "initialize",
        "params": {
            "protocolVersion": "2024-11-05",
            "capabilities": {},
            "clientInfo": { "name": "smoke", "version": "0" }
        }
    });
    stdin.write_all(format!("{init}\n").as_bytes()).await.unwrap();
    stdin.flush().await.unwrap();

    let init_resp_line = tokio::time::timeout(
        std::time::Duration::from_secs(5),
        reader.next_line(),
    )
    .await
    .expect("initialize timeout")
    .unwrap()
    .expect("eof on initialize");
    let init_resp: Value = serde_json::from_str(&init_resp_line).unwrap();
    assert_eq!(init_resp["id"], 1);
    assert!(init_resp["error"].is_null(), "initialize error: {init_resp}");

    // Some MCP servers expect an "initialized" notification before tool calls
    let initialized = json!({
        "jsonrpc": "2.0",
        "method": "notifications/initialized"
    });
    stdin.write_all(format!("{initialized}\n").as_bytes()).await.unwrap();
    stdin.flush().await.unwrap();

    // 2. tools/list — verify our 9 tools are there
    let list_req = json!({
        "jsonrpc": "2.0",
        "id": 2,
        "method": "tools/list"
    });
    stdin.write_all(format!("{list_req}\n").as_bytes()).await.unwrap();
    stdin.flush().await.unwrap();

    let list_resp_line = tokio::time::timeout(
        std::time::Duration::from_secs(5),
        reader.next_line(),
    )
    .await
    .expect("tools/list timeout")
    .unwrap()
    .expect("eof on tools/list");
    let list_resp: Value = serde_json::from_str(&list_resp_line).unwrap();
    let tools = list_resp["result"]["tools"].as_array().expect("tools array");
    let names: Vec<&str> = tools.iter().filter_map(|t| t["name"].as_str()).collect();
    for expected in &[
        "list_entries", "get_entry", "list_all_tags", "list_relation_types",
        "create_entry", "update_wiki", "update_name", "add_tag", "add_relation",
    ] {
        assert!(names.contains(expected), "tool {expected} missing from {names:?}");
    }

    // 3. tools/call list_entries
    let call_req = json!({
        "jsonrpc": "2.0",
        "id": 3,
        "method": "tools/call",
        "params": {
            "name": "list_entries",
            "arguments": { "query": null, "tag": null }
        }
    });
    stdin.write_all(format!("{call_req}\n").as_bytes()).await.unwrap();
    stdin.flush().await.unwrap();

    let call_resp_line = tokio::time::timeout(
        std::time::Duration::from_secs(5),
        reader.next_line(),
    )
    .await
    .expect("tools/call timeout")
    .unwrap()
    .expect("eof on tools/call");
    let call_resp: Value = serde_json::from_str(&call_resp_line).unwrap();
    assert!(call_resp["error"].is_null(), "tools/call error: {call_resp}");

    // The response wraps the tool's return value in a content array.
    // We just verify the call succeeded — actual structure check is
    // less important than "the chain works".
    let content = &call_resp["result"]["content"];
    assert!(!content.is_null(), "content missing from tools/call result");

    let _ = child.kill().await;
}
```

> **Note:** rmcp may format `tools/call` responses slightly differently from spec — if the assertion `call_resp["error"].is_null()` fails because rmcp wraps errors in `result.isError: true` instead of JSON-RPC-level `error`, accommodate both shapes.

- [ ] **Step 3: Run the smoke test**

Run: `cargo test -p blowup-mcp --test smoke -- --nocapture`
Expected: PASS. If the first run fails because rmcp expects a different `initialize` payload shape, adjust the request to match what rmcp actually accepts (look at the bridge stderr output for hints).

- [ ] **Step 4: Commit**

```bash
git add crates/mcp/
git commit -m "test(mcp): add cross-process smoke test (U3)

Spawns the real blowup-mcp binary, mounts a tempdir axum router on
a Unix socket, and walks the JSON-RPC handshake (initialize +
notifications/initialized + tools/list + tools/call list_entries).
This is the closest automated verification we get to running
Claude Code itself.

Co-Authored-By: Claude Opus 4.6 (1M context) <noreply@anthropic.com>"
```

---

## Phase 5 — Tauri integration: skill bridge lifecycle

### Task 11: Add `SkillBridgeState` + module skeleton

Following the deviation note in the plan header, `SkillBridgeState` lives in a new `crates/tauri/src/skill_bridge/` module — not in `AppContext`.

**Files:**

- Create: `crates/tauri/src/skill_bridge/mod.rs`
- Create: `crates/tauri/src/skill_bridge/state.rs`
- Modify: `crates/tauri/src/lib.rs`

- [ ] **Step 1: Create the module skeleton**

Create `crates/tauri/src/skill_bridge/mod.rs`:

```rust
//! Skill bridge: a Unix-domain-socket axum listener that shares the
//! same router as the in-process blowup-server, but is gated by a
//! session-only Settings switch instead of a bearer token.
//!
//! See `docs/superpowers/specs/2026-04-14-skill-bridge-design.md`
//! for the design rationale.

pub mod state;
```

- [ ] **Step 2: Create `state.rs`**

Create `crates/tauri/src/skill_bridge/state.rs`:

```rust
//! Tauri-side runtime state for the skill bridge.
//!
//! Held inside a `Mutex<Option<SkillBridgeHandle>>` and managed by
//! the Tauri app handle. Created when the Settings switch turns ON,
//! taken+dropped when it turns OFF or the app exits.

use parking_lot::Mutex;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::oneshot;
use tokio::task::JoinHandle;

pub struct SkillBridgeHandle {
    pub task: JoinHandle<()>,
    pub shutdown_tx: oneshot::Sender<()>,
    pub socket_path: PathBuf,
}

#[derive(Clone, Default)]
pub struct SkillBridgeState(pub Arc<Mutex<Option<SkillBridgeHandle>>>);

impl SkillBridgeState {
    pub fn new() -> Self {
        Self(Arc::new(Mutex::new(None)))
    }

    pub fn is_running(&self) -> bool {
        self.0.lock().is_some()
    }

    pub fn current_socket_path(&self) -> Option<PathBuf> {
        self.0.lock().as_ref().map(|h| h.socket_path.clone())
    }
}
```

- [ ] **Step 3: Wire the module into `lib.rs`**

Modify `crates/tauri/src/lib.rs` — add `pub mod skill_bridge;` near the other top-level module declarations (next to `pub mod commands;`).

In the `setup` closure, after `handle.manage(ctx.clone());`, also manage a fresh `SkillBridgeState`:

```rust
handle.manage(crate::skill_bridge::state::SkillBridgeState::new());
```

- [ ] **Step 4: Build**

Run: `cargo build -p blowup-tauri`
Expected: `Finished`.

- [ ] **Step 5: Commit**

```bash
git add crates/tauri/
git commit -m "feat(tauri): add SkillBridgeState scaffold

State for the skill bridge lives in a Tauri-side struct (not in
AppContext) because JoinHandle/oneshot::Sender are tokio runtime
types that don't belong in blowup-core. Managed via tauri::Manager
and accessed by skill_bridge_* commands in the next task.

Co-Authored-By: Claude Opus 4.6 (1M context) <noreply@anthropic.com>"
```

---

### Task 12: `skill_bridge_status` command + register

The simplest of the 5 commands — read-only.

**Files:**

- Create: `crates/tauri/src/commands/skill.rs`
- Modify: `crates/tauri/src/commands/mod.rs`
- Modify: `crates/tauri/src/lib.rs` (invoke handler list)

- [ ] **Step 1: Create the command file with `status`**

Create `crates/tauri/src/commands/skill.rs`:

```rust
//! Tauri commands for the skill bridge feature.
//!
//! 5 commands: status / start / stop / install_to_claude_code / get_install_snippets.
//! All operate on the Tauri-managed `SkillBridgeState`.

use crate::skill_bridge::state::SkillBridgeState;
use serde::Serialize;

#[derive(Serialize)]
pub struct SkillBridgeStatus {
    pub running: bool,
    pub socket_path: Option<String>,
    /// `false` on Windows for now — see plan "out of scope".
    pub supported: bool,
}

#[tauri::command]
pub async fn skill_bridge_status(
    state: tauri::State<'_, SkillBridgeState>,
) -> Result<SkillBridgeStatus, String> {
    Ok(SkillBridgeStatus {
        running: state.is_running(),
        socket_path: state
            .current_socket_path()
            .map(|p| p.to_string_lossy().into_owned()),
        supported: cfg!(unix),
    })
}
```

- [ ] **Step 2: Register the module**

Modify `crates/tauri/src/commands/mod.rs` — add `pub mod skill;` alphabetically.

- [ ] **Step 3: Add to the invoke handler**

In `crates/tauri/src/lib.rs`, find the `tauri::generate_handler!(...)` invocation and add `commands::skill::skill_bridge_status` to the list (in the same position as other commands — alphabetically or grouped).

- [ ] **Step 4: Build**

Run: `cargo build -p blowup-tauri`
Expected: `Finished`.

- [ ] **Step 5: Commit**

```bash
git add crates/tauri/
git commit -m "feat(tauri): add skill_bridge_status command

Returns running state, current socket path, and a 'supported' flag
that's false on non-unix platforms so the Settings UI can show
'not yet supported' instead of a non-functional switch.

Co-Authored-By: Claude Opus 4.6 (1M context) <noreply@anthropic.com>"
```

---

### Task 13: `skill_bridge_start` command (with stale-socket recovery)

The command that does the real work: stale-socket recovery, parent-dir creation, bind, chmod 0600, spawn axum task.

**Files:**

- Modify: `crates/tauri/src/commands/skill.rs`
- Modify: `crates/tauri/src/lib.rs` (handler registration)
- Modify: `crates/tauri/Cargo.toml` (add `blowup-mcp` dep)

- [ ] **Step 1: Add `blowup-mcp` as path dep so we can call `socket::resolve_socket_path()`**

Modify `crates/tauri/Cargo.toml` `[dependencies]`:

```toml
blowup-mcp = { path = "../mcp" }
```

This adds the `blowup-mcp` library crate (lib target, not the binary) so the Tauri side reuses the same socket-path resolution function. **Critical** — this is how the bridge and desktop agree on the path.

- [ ] **Step 2: Implement `skill_bridge_start`**

Add to `crates/tauri/src/commands/skill.rs`:

```rust
use crate::skill_bridge::state::SkillBridgeHandle;
use blowup_core::AppContext;
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::oneshot;

#[tauri::command]
pub async fn skill_bridge_start(
    state: tauri::State<'_, SkillBridgeState>,
    ctx: tauri::State<'_, Arc<AppContext>>,
) -> Result<(), String> {
    if !cfg!(unix) {
        return Err("Skill bridge 在 Windows 上暂未支持".to_string());
    }

    if state.is_running() {
        return Err("Skill bridge 已经在运行中".to_string());
    }

    let socket_path = blowup_mcp::socket::resolve_socket_path();
    ensure_parent_dir(&socket_path)?;
    handle_stale_socket(&socket_path).await?;

    // Bind in a blocking call (UnixListener::bind is sync but fast)
    let listener = std::os::unix::net::UnixListener::bind(&socket_path)
        .map_err(|e| format!("bind {} 失败: {e}", socket_path.display()))?;
    listener
        .set_nonblocking(true)
        .map_err(|e| format!("set_nonblocking 失败: {e}"))?;
    let listener = tokio::net::UnixListener::from_std(listener)
        .map_err(|e| format!("from_std 失败: {e}"))?;

    // 0600 perms
    let perms = std::fs::Permissions::from_mode(0o600);
    std::fs::set_permissions(&socket_path, perms)
        .map_err(|e| format!("chmod 0600 失败: {e}"))?;

    // Spawn the serve task
    let (shutdown_tx, shutdown_rx) = oneshot::channel::<()>();
    let app_state: blowup_server::AppState = (**ctx).clone();
    let task = tokio::spawn(async move {
        // Re-implement the body of serve_unix here because we already
        // have the UnixListener bound — passing a path would re-bind.
        let router = blowup_server::build_router(app_state);
        if let Err(e) = axum::serve(listener, router)
            .with_graceful_shutdown(async move {
                let _ = shutdown_rx.await;
            })
            .await
        {
            tracing::warn!(error = %e, "skill bridge serve exited");
        }
    });

    *state.0.lock() = Some(SkillBridgeHandle {
        task,
        shutdown_tx,
        socket_path,
    });
    Ok(())
}

fn ensure_parent_dir(socket_path: &std::path::Path) -> Result<(), String> {
    let parent = socket_path
        .parent()
        .ok_or_else(|| "socket path has no parent".to_string())?;
    std::fs::create_dir_all(parent)
        .map_err(|e| format!("mkdir {} 失败: {e}", parent.display()))?;
    let perms = std::fs::Permissions::from_mode(0o700);
    let _ = std::fs::set_permissions(parent, perms);
    Ok(())
}

/// If the socket file already exists, try to connect to it. If we
/// can connect, another desktop instance is using it — bail. If we
/// can't, it's an orphan from a previous crash — unlink it.
async fn handle_stale_socket(socket_path: &PathBuf) -> Result<(), String> {
    if !socket_path.exists() {
        return Ok(());
    }
    match tokio::net::UnixStream::connect(socket_path).await {
        Ok(_) => Err(format!(
            "{} 已被另一个进程占用",
            socket_path.display()
        )),
        Err(_) => {
            std::fs::remove_file(socket_path)
                .map_err(|e| format!("清理孤儿 socket 失败: {e}"))?;
            Ok(())
        }
    }
}
```

- [ ] **Step 3: Update the invoke handler to register `skill_bridge_start`**

Modify `crates/tauri/src/lib.rs` — add `commands::skill::skill_bridge_start` to `tauri::generate_handler!`.

- [ ] **Step 4: Build**

Run: `cargo build -p blowup-tauri`
Expected: `Finished`. If the `(**ctx).clone()` line fails because `ctx` is wrapped differently, look at how other commands access `Arc<AppContext>` and match the pattern.

- [ ] **Step 5: Manual smoke (no test, manual verify in next phase)**

We won't write a unit test for this — it requires a real Tauri app context. End-to-end verification happens in the manual U4 step at the end.

- [ ] **Step 6: Commit**

```bash
git add crates/tauri/
git commit -m "feat(tauri): add skill_bridge_start command

Handles the full lifecycle: parent dir mkdir 0700, stale-socket
recovery (connect-probe + unlink), bind, chmod 0600, spawn serve
task. Stores the JoinHandle + shutdown_tx + socket_path in
SkillBridgeState for the corresponding stop command to take.

Co-Authored-By: Claude Opus 4.6 (1M context) <noreply@anthropic.com>"
```

---

### Task 14: `skill_bridge_stop` command + Drop guard for app exit

**Files:**

- Modify: `crates/tauri/src/commands/skill.rs`
- Modify: `crates/tauri/src/skill_bridge/state.rs`
- Modify: `crates/tauri/src/lib.rs` (handler registration + window close hook)

- [ ] **Step 1: Implement `skill_bridge_stop`**

Add to `crates/tauri/src/commands/skill.rs`:

```rust
#[tauri::command]
pub async fn skill_bridge_stop(
    state: tauri::State<'_, SkillBridgeState>,
) -> Result<(), String> {
    let handle = state.0.lock().take();
    let Some(h) = handle else {
        return Ok(()); // already stopped, idempotent
    };
    let _ = h.shutdown_tx.send(());
    let _ = tokio::time::timeout(std::time::Duration::from_secs(2), h.task).await;
    let _ = std::fs::remove_file(&h.socket_path);
    Ok(())
}
```

- [ ] **Step 2: Register in invoke handler**

Modify `crates/tauri/src/lib.rs` — add `commands::skill::skill_bridge_stop` to `tauri::generate_handler!`.

- [ ] **Step 3: Add a sync helper for app exit cleanup (no async runtime needed)**

Add to `crates/tauri/src/skill_bridge/state.rs`:

```rust
impl SkillBridgeState {
    /// Sync best-effort cleanup, safe to call from a Drop or signal
    /// handler. Sends shutdown but does NOT await the task — that's
    /// up to the runtime.
    pub fn shutdown_blocking(&self) {
        if let Some(h) = self.0.lock().take() {
            let _ = h.shutdown_tx.send(());
            let _ = std::fs::remove_file(&h.socket_path);
        }
    }
}
```

- [ ] **Step 4: Wire app exit hook**

Modify `crates/tauri/src/lib.rs` — find the `tauri::Builder::default()` chain, and inside the `.setup(...)` or `.on_window_event(...)` chain add a `WindowEvent::CloseRequested` handler:

```rust
.on_window_event(|window, event| {
    if let tauri::WindowEvent::CloseRequested { .. } = event {
        if let Some(state) = window.try_state::<crate::skill_bridge::state::SkillBridgeState>() {
            state.shutdown_blocking();
        }
    }
})
```

If the existing code already has `on_window_event`, add the SkillBridgeState shutdown call inside the existing closure rather than overwriting it.

- [ ] **Step 5: Build**

Run: `cargo build -p blowup-tauri`
Expected: `Finished`.

- [ ] **Step 6: Commit**

```bash
git add crates/tauri/
git commit -m "feat(tauri): add skill_bridge_stop + window-close cleanup

Stop is idempotent (no-op if already stopped). Window close hook
calls shutdown_blocking which sends the shutdown signal and unlinks
the socket file synchronously — no orphaned files after a normal
quit. Crash recovery is handled by the start command's stale-socket
probe.

Co-Authored-By: Claude Opus 4.6 (1M context) <noreply@anthropic.com>"
```

---

## Phase 6 — Install flow

### Task 15: `skill_bridge_get_install_snippets` command

Returns a struct with one JSON snippet per known MCP client. The frontend renders these in collapsible panels with copy buttons.

**Files:**

- Modify: `crates/tauri/src/commands/skill.rs`
- Modify: `crates/tauri/src/lib.rs` (handler registration)

- [ ] **Step 1: Implement the command**

Add to `crates/tauri/src/commands/skill.rs`:

```rust
#[derive(Serialize)]
pub struct InstallSnippets {
    pub binary_path: String,
    pub claude_code: String,
    pub claude_desktop: String,
    pub cursor: String,
    pub cline: String,
}

#[tauri::command]
pub async fn skill_bridge_get_install_snippets(
    app: tauri::AppHandle,
) -> Result<InstallSnippets, String> {
    let bin = installed_binary_path(&app)?;
    let bin_str = bin.to_string_lossy().to_string();

    let claude_code = format!(
        "claude mcp add blowup-skill {}",
        shell_escape(&bin_str)
    );

    let json_block = serde_json::json!({
        "mcpServers": {
            "blowup-skill": {
                "command": bin_str.clone(),
                "args": []
            }
        }
    });
    let pretty = serde_json::to_string_pretty(&json_block)
        .map_err(|e| format!("serialize snippet: {e}"))?;

    Ok(InstallSnippets {
        binary_path: bin_str,
        claude_code,
        claude_desktop: pretty.clone(),
        cursor: pretty.clone(),
        cline: pretty,
    })
}

/// The path where `skill_bridge_install_to_claude_code` will copy the
/// binary. Same function used by both commands so the snippets and the
/// install action agree.
fn installed_binary_path(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    let _ = app; // unused on unix, kept for Windows future
    #[cfg(unix)]
    {
        let home = dirs::home_dir().ok_or_else(|| "no home dir".to_string())?;
        Ok(home.join(".local").join("share").join("blowup").join("blowup-mcp"))
    }
    #[cfg(not(unix))]
    {
        Err("Windows 暂不支持".to_string())
    }
}

fn shell_escape(s: &str) -> String {
    if s.chars().all(|c| c.is_alphanumeric() || "/-._".contains(c)) {
        s.to_string()
    } else {
        format!("'{}'", s.replace('\'', "'\\''"))
    }
}
```

- [ ] **Step 2: Add `dirs` to Tauri Cargo.toml if not already**

Check `crates/tauri/Cargo.toml` — if `dirs` is not listed in `[dependencies]`, add `dirs = "5"`.

- [ ] **Step 3: Register in invoke handler**

Modify `crates/tauri/src/lib.rs` — add `commands::skill::skill_bridge_get_install_snippets`.

- [ ] **Step 4: Build**

Run: `cargo build -p blowup-tauri`
Expected: `Finished`.

- [ ] **Step 5: Commit**

```bash
git add crates/tauri/
git commit -m "feat(tauri): add skill_bridge_get_install_snippets command

Returns one JSON snippet per known MCP client (Claude Code CLI command,
Claude Desktop / Cursor / Cline mcpServers JSON). All snippets reference
the same target binary path (\$HOME/.local/share/blowup/blowup-mcp on
unix) so they stay consistent with what install_to_claude_code copies.

Co-Authored-By: Claude Opus 4.6 (1M context) <noreply@anthropic.com>"
```

---

### Task 16: `skill_bridge_install_to_claude_code` command

Copies the bundled `blowup-mcp` binary to a stable location, copies the SKILL.md to `~/.claude/skills/blowup-wiki-writer/`, runs `claude mcp add` (best-effort), returns a report.

**Files:**

- Modify: `crates/tauri/src/commands/skill.rs`
- Modify: `crates/tauri/src/lib.rs` (handler registration)
- Modify: `crates/tauri/Cargo.toml` (add `sha2`)

- [ ] **Step 1: Add sha2 dep**

Modify `crates/tauri/Cargo.toml` `[dependencies]`:

```toml
sha2 = "0.10"
```

- [ ] **Step 2: Implement the command**

Add to `crates/tauri/src/commands/skill.rs`:

```rust
use sha2::{Digest, Sha256};

#[derive(Serialize)]
pub struct InstallReport {
    pub binary_path: String,
    pub skill_path: String,
    pub claude_added: bool,
    pub manual_command: Option<String>,
}

#[tauri::command]
pub async fn skill_bridge_install_to_claude_code(
    app: tauri::AppHandle,
) -> Result<InstallReport, String> {
    if !cfg!(unix) {
        return Err("Skill bridge 在 Windows 上暂未支持".to_string());
    }

    let resource_dir = app
        .path()
        .resource_dir()
        .map_err(|e| format!("resource_dir: {e}"))?;
    let bundled_binary = resource_dir.join("blowup-mcp");
    let bundled_skill = resource_dir
        .join("skills")
        .join("blowup-wiki-writer")
        .join("SKILL.md");

    if !bundled_binary.exists() {
        return Err(format!(
            "打包资源缺少 blowup-mcp 二进制(预期 {})。请用 `just build-mcp && just build` 重新打包",
            bundled_binary.display()
        ));
    }
    if !bundled_skill.exists() {
        return Err(format!(
            "打包资源缺少 SKILL.md(预期 {})",
            bundled_skill.display()
        ));
    }

    let target_binary = installed_binary_path(&app)?;
    if let Some(parent) = target_binary.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| format!("mkdir {} 失败: {e}", parent.display()))?;
    }
    copy_if_changed(&bundled_binary, &target_binary)?;
    let perms = std::fs::Permissions::from_mode(0o755);
    std::fs::set_permissions(&target_binary, perms)
        .map_err(|e| format!("chmod 0755 失败: {e}"))?;

    let home = dirs::home_dir().ok_or_else(|| "no home dir".to_string())?;
    let skill_dir = home
        .join(".claude")
        .join("skills")
        .join("blowup-wiki-writer");
    std::fs::create_dir_all(&skill_dir)
        .map_err(|e| format!("mkdir {} 失败: {e}", skill_dir.display()))?;
    let skill_target = skill_dir.join("SKILL.md");
    std::fs::copy(&bundled_skill, &skill_target)
        .map_err(|e| format!("copy SKILL.md 失败: {e}"))?;

    let manual_command = format!(
        "claude mcp add blowup-skill {}",
        shell_escape(&target_binary.to_string_lossy())
    );

    let claude_added = std::process::Command::new("claude")
        .args([
            "mcp",
            "add",
            "blowup-skill",
            target_binary.to_str().unwrap_or(""),
        ])
        .status()
        .map(|s| s.success())
        .unwrap_or(false);

    Ok(InstallReport {
        binary_path: target_binary.to_string_lossy().into_owned(),
        skill_path: skill_target.to_string_lossy().into_owned(),
        claude_added,
        manual_command: if claude_added { None } else { Some(manual_command) },
    })
}

fn copy_if_changed(src: &PathBuf, dst: &PathBuf) -> Result<(), String> {
    let src_hash = file_sha256(src)?;
    if dst.exists() {
        let dst_hash = file_sha256(dst)?;
        if src_hash == dst_hash {
            return Ok(());
        }
    }
    std::fs::copy(src, dst).map_err(|e| format!("copy 失败: {e}"))?;
    Ok(())
}

fn file_sha256(path: &PathBuf) -> Result<Vec<u8>, String> {
    let bytes = std::fs::read(path).map_err(|e| format!("read {}: {e}", path.display()))?;
    let mut hasher = Sha256::new();
    hasher.update(&bytes);
    Ok(hasher.finalize().to_vec())
}
```

- [ ] **Step 3: Register in invoke handler**

Modify `crates/tauri/src/lib.rs` — add `commands::skill::skill_bridge_install_to_claude_code`.

- [ ] **Step 4: Build**

Run: `cargo build -p blowup-tauri`
Expected: `Finished`.

- [ ] **Step 5: Commit**

```bash
git add crates/tauri/
git commit -m "feat(tauri): add skill_bridge_install_to_claude_code command

Copies bundled blowup-mcp binary to ~/.local/share/blowup/blowup-mcp
(skipping if SHA256 unchanged), copies SKILL.md to ~/.claude/skills/,
then best-effort runs 'claude mcp add'. If the CLI isn't installed,
returns the manual command for the user to copy.

Co-Authored-By: Claude Opus 4.6 (1M context) <noreply@anthropic.com>"
```

---

## Phase 7 — Resource bundling and build glue

### Task 17: Write `SKILL.md` content

The actual skill instructions Claude reads.

**Files:**

- Create: `crates/tauri/resources/skills/blowup-wiki-writer/SKILL.md`

- [ ] **Step 1: Write the skill file**

Create `crates/tauri/resources/skills/blowup-wiki-writer/SKILL.md`:

````markdown
---
name: blowup-wiki-writer
description: 给 blowup 知识库写 wiki 条目。用户会指定条目名称和(可选的)写作框架,
  skill 自动完成研究、生成内容、写库、加标签、加关系。需要 desktop app 启用
  skill bridge 开关。任何 [FATAL] 错误一律停止,不重试。
---

# Blowup Wiki Writer

你是 blowup 知识库的 wiki 撰写助手。blowup 是一个个人电影知识库管理工具,知识库
里的"条目"可以是电影、影人、流派、概念,任何用户认为值得记录的事物。条目之间通
过用户自定义的"关系"(导演了 / 主演了 / 属于流派 / 受...影响等)互相连接。

## 前置条件

调用任何 `blowup-skill` MCP 工具之前,确认 desktop app 的 "Skill Bridge" 开关已打开
(在 Settings → Skill Bridge 区域)。如果第一次工具调用返回以 `[FATAL]` 开头的错误,
**立即停止并把错误原样转告用户**,不重试,不尝试别的工具。

## 工作流

### 1. 解析用户意图

用户的请求会包含两部分:
- **条目名称**(必须):例如 "情书"、"岩井俊二"、"日本新浪潮"
- **写作框架/角度**(可选):例如 "按导演的方式写"、"按流派的角度写"、"重点
  讲它对后世的影响"。如果用户没给,你应该根据条目的性质自行判断 —— 例如人物
  条目通常包括生平、代表作、风格;电影条目通常包括制作背景、故事、影像、影响。

如果用户只给了一个名字、没有任何上下文,先简短问一句 "希望从哪个角度写?",
不要直接开写。

### 2. 查重

调用 `list_entries(query="<条目名称>")`。

- **已存在同名条目**: 把现有条目 ID 和当前 wiki 摘要告诉用户,问 "已有同名
  条目 #N,要在原有内容上更新,还是另起一个变体(例如 '情书 (2020 重映版)')?"
- **不存在**: 进入下一步

### 3. 了解上下文

并行调用以下读工具,把结果记在内存里:

- `list_all_tags()` — 看用户已有的标签习惯。后续 add_tag 时**优先复用现有
  标签**,不要发明同义新标签(例如不要在 "导演" 已经存在时用 "电影导演")
- `list_relation_types()` — 看用户已有的关系类型,同样优先复用

根据用户给的写作框架,决定要查找哪些相关条目:
- 写电影 → 查导演、主演、所属流派
- 写影人 → 查代表作品、合作过的人
- 写流派 → 查代表作品、代表导演

对每个相关概念调一次 `list_entries(query="<相关条目名称>")`,把找到的条目 ID
记下来,后面 add_relation 用。

### 4. 网络搜索

用 WebSearch / WebFetch 收集事实。优先级:
- 维基百科(中文 / 英文)
- 豆瓣电影 / IMDb / Letterboxd
- 影评类专业媒体

**禁止**:
- 把搜索结果整段照抄(版权 + 风格不一致)
- 把不确定的"传闻"当事实写
- 给电影/影人主观打分(除非明确引用他人评分)

### 5. 生成 Wiki Markdown

严格按用户给的写作框架(或你自行判断的结构)写。要求:

- **全部中文**
- 条目结构清晰:用 `## 二级标题` 分节
- 涉及具体年份、奖项、票房、人名等事实**必须有来源** —— 不要硬背,不确定就
  在文末用 "## 资料来源" 节列出
- **不主观吹捧** —— 避免 "经典"、"伟大"、"必看" 之类的词,除非引用他人观点
  并注明来源("被《电影手册》评为...")
- **信息冲突时标记不确定性**,不要选一个写。例:"上映年份各源不一(豆瓣 1995,
  IMDb 1996),以日本本土首映为准应为..."

### 6. 写库

按顺序调:

1. `create_entry(name="<条目名称>")` → 拿到新 entry id。**先查重**(已在第 2
   步做过)。若 create_entry 返回 [FATAL] 错误,停止。
2. `update_wiki(id=<id>, wiki=<markdown>)` → 写入正文。
3. 对每个适用的标签,调 `add_tag(entry_id=<id>, tag="<tag>")`。优先用
   `list_all_tags` 返回的现有标签,不要碎片化。
4. 对每个相关条目,调 `add_relation(from_id=<id>, to_id=<相关条目 id>,
   relation_type="<类型>")`。relation_type 必须复用 `list_relation_types` 返
   回的现有类型 —— 如果实在没有合适的,可以创建新类型,但要在写之前问用户
   "现有关系类型是 [...],你想给这条关系用哪个?"

### 7. 报告完成

简短一句:
> 已写入条目 #N 《<名称>》(7 个标签,3 条关系)。desktop app 应该已自动刷新,
> 你可以在 Library 或 Wiki 标签页查看。

## 失败处理

- 任何工具返回以 `[FATAL]` 开头的错误 → **立即停止,把错误原样告诉用户**。
  不重试,不尝试别的工具。
- 工具返回不带 `[FATAL]` 的中文错误(L3 业务错误) → 读 "提示:" 段,据此调整
  参数,**最多重试一次**。第二次还失败就停止报告。
- 网络搜索失败 → 在 wiki 内容里标注 "[资料缺失]" 并继续,不要捏造。
- 不要为了"完成任务"而捏造内容 —— 准确性比覆盖度重要。
````

- [ ] **Step 2: Verify the file is well-formed markdown**

Run: `head -5 crates/tauri/resources/skills/blowup-wiki-writer/SKILL.md`
Expected: Frontmatter `---` block visible.

- [ ] **Step 3: Commit**

```bash
git add crates/tauri/resources/
git commit -m "feat(skill): add blowup-wiki-writer SKILL.md

The actual instructions Claude reads to drive the wiki-writing
workflow. Mirrors the tool descriptions (which Claude also sees)
as a redundant safety net for one-shot success.

Co-Authored-By: Claude Opus 4.6 (1M context) <noreply@anthropic.com>"
```

---

### Task 18: `tauri.conf.json` bundle resources + justfile build glue

We need the build to produce `target/release/blowup-mcp` and copy it to `crates/tauri/resources/blowup-mcp` before the Tauri bundler picks it up.

**Files:**

- Modify: `crates/tauri/tauri.conf.json`
- Modify: `justfile`
- Create: `crates/tauri/resources/.gitignore`

- [ ] **Step 1: Add bundle.resources to tauri.conf.json**

Read the existing `crates/tauri/tauri.conf.json`. Find the `bundle` section. Add:

```json
"resources": [
  "resources/blowup-mcp",
  "resources/skills/**/*"
]
```

If `bundle.resources` already exists, merge — don't overwrite.

- [ ] **Step 2: Add a .gitignore for the binary copy**

Create `crates/tauri/resources/.gitignore`:

```
blowup-mcp
```

(The binary is build output, not source. The skills/ subdir IS source and should be tracked.)

- [ ] **Step 3: Add `build-mcp` recipe to justfile + integrate**

Read the existing `justfile`. Find the `build` recipe (or whatever runs `tauri build`). Add a new recipe before it:

```just
# Build the blowup-mcp bridge binary and copy it into Tauri resources
# so the bundler picks it up. Always runs in release mode — debug
# bridge binaries are huge and have no practical use here.
build-mcp:
    cargo build --release -p blowup-mcp
    mkdir -p crates/tauri/resources
    cp target/release/blowup-mcp crates/tauri/resources/blowup-mcp
```

Then make `build` depend on `build-mcp`:

```just
build: build-mcp
    cargo tauri build
```

(If the existing `build` recipe has a different body, just prepend `build-mcp` to its dependency list.)

- [ ] **Step 4: Test the build-mcp recipe**

Run: `just build-mcp`
Expected: Builds blowup-mcp in release mode and creates `crates/tauri/resources/blowup-mcp`.

Run: `ls -la crates/tauri/resources/blowup-mcp`
Expected: File exists, is an executable.

- [ ] **Step 5: Commit**

```bash
git add crates/tauri/tauri.conf.json crates/tauri/resources/.gitignore justfile
git commit -m "build: bundle blowup-mcp + skill resources into Tauri app

just build-mcp builds the bridge binary in release mode and copies
it into crates/tauri/resources/. just build depends on build-mcp,
so 'just build' picks up the bridge automatically. The binary is
gitignored (build output); the skills/ subdir IS source.

Co-Authored-By: Claude Opus 4.6 (1M context) <noreply@anthropic.com>"
```

---

## Phase 8 — Frontend Settings UI

### Task 19: Add `skillBridge` namespace to `src/lib/tauri.ts`

**Files:**

- Modify: `src/lib/tauri.ts`

- [ ] **Step 1: Add the types and invoke wrappers**

Read `src/lib/tauri.ts` to see the existing pattern, then add a new section near the bottom (before any default export):

```typescript
// ── Skill Bridge ────────────────────────────────────────────────

export type SkillBridgeStatus = {
  running: boolean;
  socket_path: string | null;
  supported: boolean;
};

export type InstallReport = {
  binary_path: string;
  skill_path: string;
  claude_added: boolean;
  manual_command: string | null;
};

export type InstallSnippets = {
  binary_path: string;
  claude_code: string;
  claude_desktop: string;
  cursor: string;
  cline: string;
};

export const skillBridge = {
  status: () => invoke<SkillBridgeStatus>("skill_bridge_status"),
  start: () => invoke<void>("skill_bridge_start"),
  stop: () => invoke<void>("skill_bridge_stop"),
  installToClaudeCode: () =>
    invoke<InstallReport>("skill_bridge_install_to_claude_code"),
  getInstallSnippets: () =>
    invoke<InstallSnippets>("skill_bridge_get_install_snippets"),
};
```

- [ ] **Step 2: Type-check**

Run: `bunx tsc --noEmit`
Expected: No errors.

- [ ] **Step 3: Commit**

```bash
git add src/lib/tauri.ts
git commit -m "feat(frontend): add skillBridge invoke wrappers

Typed wrappers for the 5 skill_bridge_* Tauri commands plus the
SkillBridgeStatus / InstallReport / InstallSnippets types.

Co-Authored-By: Claude Opus 4.6 (1M context) <noreply@anthropic.com>"
```

---

### Task 20: Add Skill Bridge section to Settings.tsx

**Files:**

- Modify: `src/pages/Settings.tsx`

- [ ] **Step 1: Read the existing Settings structure**

Run: `head -200 src/pages/Settings.tsx` to see the section pattern (Title + Stack + form rows).

- [ ] **Step 2: Add the section component**

In `src/pages/Settings.tsx`, near the top of the file (after the existing imports), add a new component:

```tsx
import {
  ActionIcon,
  Box,
  Button,
  Code,
  Collapse,
  Group,
  Stack,
  Switch,
  Text,
  Textarea,
  Title,
} from "@mantine/core";
import { useEffect, useState } from "react";
import {
  skillBridge,
  type InstallSnippets,
  type SkillBridgeStatus,
} from "../lib/tauri";

function SkillBridgeSection() {
  const [status, setStatus] = useState<SkillBridgeStatus | null>(null);
  const [snippets, setSnippets] = useState<InstallSnippets | null>(null);
  const [snippetsOpen, setSnippetsOpen] = useState(false);
  const [busy, setBusy] = useState(false);
  const [installMsg, setInstallMsg] = useState<string | null>(null);

  const refresh = async () => {
    setStatus(await skillBridge.status());
  };

  useEffect(() => {
    refresh();
    skillBridge.getInstallSnippets().then(setSnippets).catch(() => {});
  }, []);

  const toggle = async (on: boolean) => {
    setBusy(true);
    try {
      if (on) await skillBridge.start();
      else await skillBridge.stop();
      await refresh();
    } catch (e) {
      alert(`Skill Bridge 操作失败: ${e}`);
    } finally {
      setBusy(false);
    }
  };

  const install = async () => {
    setBusy(true);
    setInstallMsg(null);
    try {
      const report = await skillBridge.installToClaudeCode();
      if (report.claude_added) {
        setInstallMsg(
          `✓ 已安装。二进制: ${report.binary_path};Skill: ${report.skill_path}`,
        );
      } else {
        setInstallMsg(
          `二进制已就位但 'claude mcp add' 未运行(可能未安装 claude CLI)。请手动运行:\n${report.manual_command}`,
        );
      }
    } catch (e) {
      setInstallMsg(`安装失败: ${e}`);
    } finally {
      setBusy(false);
    }
  };

  if (status && !status.supported) {
    return (
      <Box>
        <Title order={3} mb="0.5rem">Skill Bridge</Title>
        <Text c="dimmed" size="sm">
          Skill bridge 在 Windows 上暂未支持。
        </Text>
      </Box>
    );
  }

  return (
    <Box>
      <Title order={3} mb="0.5rem">Skill Bridge</Title>
      <Text c="dimmed" size="xs" mb="0.75rem">
        启用后,本机的 MCP 客户端(Claude Code、Cursor、Cline 等)可以通过
        Unix 域套接字调用 blowup 的知识库 API。开关关闭时不暴露任何端口或服务。
      </Text>
      <Stack gap="0.6rem">
        <Group justify="space-between">
          <div>
            <Text size="sm" fw={500}>启用</Text>
            <Text size="xs" c="dimmed">
              {status?.running
                ? `运行中 — ${status.socket_path}`
                : "已停止"}
            </Text>
          </div>
          <Switch
            checked={status?.running ?? false}
            disabled={busy}
            onChange={(e) => toggle(e.currentTarget.checked)}
          />
        </Group>

        <Group>
          <Button size="xs" variant="default" onClick={install} loading={busy}>
            安装到 Claude Code
          </Button>
          <Button
            size="xs"
            variant="subtle"
            onClick={() => setSnippetsOpen(!snippetsOpen)}
          >
            {snippetsOpen ? "隐藏" : "显示"}其他客户端配置
          </Button>
        </Group>

        {installMsg && (
          <Text size="xs" c="dimmed" style={{ whiteSpace: "pre-wrap" }}>
            {installMsg}
          </Text>
        )}

        <Collapse in={snippetsOpen}>
          {snippets && (
            <Stack gap="0.5rem">
              <SnippetBlock title="Claude Code (CLI)" body={snippets.claude_code} />
              <SnippetBlock title="Claude Desktop" body={snippets.claude_desktop} />
              <SnippetBlock title="Cursor" body={snippets.cursor} />
              <SnippetBlock title="Cline / Continue / Zed" body={snippets.cline} />
            </Stack>
          )}
        </Collapse>
      </Stack>
    </Box>
  );
}

function SnippetBlock({ title, body }: { title: string; body: string }) {
  const [copied, setCopied] = useState(false);
  return (
    <Box>
      <Group justify="space-between" mb={4}>
        <Text size="xs" fw={500}>{title}</Text>
        <ActionIcon
          size="xs"
          variant="subtle"
          onClick={() => {
            navigator.clipboard.writeText(body);
            setCopied(true);
            setTimeout(() => setCopied(false), 1500);
          }}
        >
          {copied ? "✓" : "📋"}
        </ActionIcon>
      </Group>
      <Code block>{body}</Code>
    </Box>
  );
}
```

- [ ] **Step 3: Render the section**

Find the main `Settings` component's return value and add `<SkillBridgeSection />` in the appropriate place (e.g., after the existing sections, before the bottom of the Stack/ScrollArea).

- [ ] **Step 4: Type-check**

Run: `bunx tsc --noEmit`
Expected: No errors.

- [ ] **Step 5: Visual smoke (manual)**

Run: `just dev`
Open Settings page, scroll down, verify the Skill Bridge section appears with the switch off.

- [ ] **Step 6: Commit**

```bash
git add src/pages/Settings.tsx
git commit -m "feat(frontend): add Skill Bridge section to Settings

Switch + status row + install button + collapsible 'other clients'
panel showing JSON snippets for Claude Desktop / Cursor / Cline.
Each snippet has a copy button. On Windows the section shows a
'not yet supported' message instead of the controls.

Co-Authored-By: Claude Opus 4.6 (1M context) <noreply@anthropic.com>"
```

---

## Phase 9 — Manual end-to-end verification (U4)

### Task 21: U4 manual checklist

This is the final gate. Run through each item, fix any failures, and record the results in the spec's "验证记录" section.

**Files:**

- Modify: `docs/superpowers/specs/2026-04-14-skill-bridge-design.md` (验证记录 section at the bottom)

- [ ] **Step 1: Build everything fresh**

Run: `just build-mcp && cargo build -p blowup-tauri`
Expected: Both succeed. blowup-mcp binary at `crates/tauri/resources/blowup-mcp`.

- [ ] **Step 2: Start desktop in dev mode**

Run: `just dev`
Expected: Desktop app opens.

- [ ] **Step 3: Verify Settings UI**

Open Settings → scroll to "Skill Bridge" section → verify:
- Switch shows "已停止"
- "安装到 Claude Code" button is enabled
- "显示其他客户端配置" button shows snippets when clicked
- Snippets contain a path like `~/.local/share/blowup/blowup-mcp`

- [ ] **Step 4: Click "安装到 Claude Code"**

- Verify the install message shows "已安装" or the manual command fallback.
- Verify `~/.local/share/blowup/blowup-mcp` exists and is executable: `ls -la ~/.local/share/blowup/blowup-mcp`
- Verify `~/.claude/skills/blowup-wiki-writer/SKILL.md` exists.

- [ ] **Step 5: If the install reported manual fallback, run it manually**

Run the suggested `claude mcp add blowup-skill ...` command. Verify it succeeds.

- [ ] **Step 6: Toggle the switch ON**

- Verify status shows "运行中 — ~/Library/Application Support/blowup/skill.sock" (macOS) or equivalent.
- Verify the socket file exists: `ls -la ~/Library/Application\ Support/blowup/skill.sock`
- Verify perms are `srw-------` (0600 + socket type).

- [ ] **Step 7: Smoke-test from outside via the bridge binary directly**

In another terminal, run:
```bash
echo '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2024-11-05","capabilities":{},"clientInfo":{"name":"test","version":"0"}}}' | ~/.local/share/blowup/blowup-mcp
```
Expected: A JSON-RPC initialize response on stdout. (Bridge will hang waiting for more input — Ctrl-C is fine.)

- [ ] **Step 8: Real Claude Code test**

In a fresh terminal, run `claude` and try:
```
列出 blowup 知识库现有的所有标签
```
Expected: Claude calls `mcp__blowup-skill__list_all_tags` (or similar) and shows the results.

- [ ] **Step 9: Real wiki write test**

In Claude:
```
帮我写一个 blowup wiki 条目:岩井俊二,按导演的角度写
```
Expected: Claude follows the SKILL.md workflow — first checks if the entry exists, lists tags + relation types, searches the web, drafts content, creates the entry, writes wiki, adds tags + relations. Then reports completion.

Switch to the desktop app's Library/Wiki page and verify the new entry appears with the expected content (event bus should auto-refresh).

- [ ] **Step 10: Toggle the switch OFF**

- Verify the socket file is gone.
- Try the JSON-RPC echo from Step 7 again — expect the bridge to return a `[FATAL] blowup app 未启用 skill bridge` error.
- Try the same Claude prompt from Step 8 — expect Claude to see the FATAL error and stop (not loop).

- [ ] **Step 11: Stale socket recovery test**

- Toggle ON.
- Force-kill the desktop process: `pkill -9 blowup-tauri` (or close terminal running just dev with no clean shutdown).
- Verify the socket file is left behind: `ls ~/Library/Application\ Support/blowup/skill.sock`
- Restart `just dev`, toggle ON.
- Verify it succeeds (stale socket was recovered).

- [ ] **Step 12: Record results in spec**

Edit `docs/superpowers/specs/2026-04-14-skill-bridge-design.md`, find the "验证记录" section at the bottom, and replace it with:

```markdown
## 验证记录

**Date:** YYYY-MM-DD
**Tester:** lixuan
**Platform:** macOS X.Y.Z

| Step | Result | Notes |
|------|--------|-------|
| 3. Settings UI renders | ✓ | |
| 4. Install button works | ✓ | |
| 6. Switch ON binds socket | ✓ | |
| 7. Direct bridge JSON-RPC | ✓ | |
| 8. Claude lists tags | ✓ | |
| 9. End-to-end wiki write | ✓ | |
| 10. Switch OFF severs connection | ✓ | |
| 11. Stale socket recovery | ✓ | |
```

- [ ] **Step 13: Commit verification record**

```bash
git add docs/superpowers/specs/2026-04-14-skill-bridge-design.md
git commit -m "docs(spec): record skill bridge U4 manual verification

All 11 verification steps pass on macOS. Marks the skill-bridge
plan as feature-complete.

Co-Authored-By: Claude Opus 4.6 (1M context) <noreply@anthropic.com>"
```

---

## Self-Review Checklist (filled by writing-plans skill before handoff)

- [x] **Spec coverage** — All sections of the spec have a corresponding task: socket support (T2), skill router placeholder (T3), bridge crate (T4-T9), cross-process test (T10), AppContext deviation (T11), 5 commands (T12-T16), resources/build (T17-T18), Settings UI (T19-T20), manual U4 (T21).
- [x] **Placeholder scan** — No "TBD/TODO" in tasks. The two notes about rmcp macro syntax are bounded ("verify against rmcp docs if build fails") and don't leave the engineer guessing about the *intent*.
- [x] **Type consistency** — `SkillBridgeStatus`, `InstallReport`, `InstallSnippets`, `SkillBridgeHandle`, `SkillBridgeState`, `BlowupClient`, `McpError`, `ErrorCode` all match between definitions and usages. The `(**ctx).clone()` line in T13 has a "if this fails, look at sibling commands" note for the inevitable Tauri State unwrapping syntax variance.
- [x] **YAGNI** — Windows is explicitly deferred. Spec-mentioned but not-yet-needed items (skill_only routes, batch ops) are placeholder files only.
- [x] **TDD discipline** — T2, T4, T5, T6, T10 follow the failing-test-first pattern. T7-T9 use type-check-and-build as the verification gate (no test possible until U3 in T10). T11-T20 are integration code that can only be verified manually (U4 in T21).
