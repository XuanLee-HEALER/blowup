//! Bridge MCP service — one struct, one tool router impl block,
//! one method per exposed tool. The struct carries a `BlowupClient`
//! that points at the desktop app's Unix socket; methods are
//! `async fn` and return `Result<T, rmcp::ErrorData>`.
//!
//! rmcp 1.4 wiring notes (for T8/T9 to follow the same shape):
//!   - `#[tool_router]` on the inherent impl generates a hidden
//!     `Self::tool_router()` that the handler macro reads.
//!   - `#[tool_handler]` on `impl ServerHandler for BlowupService {}`
//!     auto-fills `call_tool`, `list_tools`, `get_tool`, and `get_info`
//!     (using `CARGO_CRATE_NAME` + `CARGO_PKG_VERSION`).
//!   - Tool methods return `Result<T, rmcp::ErrorData>` so `?` works
//!     directly on `BlowupClient` calls via `From<McpError>`.
//!   - `McpError::user_message()` carries the `[FATAL] ` prefix that
//!     skill prompts pattern-match on, so the conversion goes through
//!     it, not through `Display`.

use crate::client::BlowupClient;
use crate::error::McpError;
use rmcp::{ErrorData, ServerHandler, tool, tool_handler, tool_router};

/// Minimal service used to verify the rmcp wiring before adding the
/// 9 real tools. After Tasks 8/9 the `ping` method will be removed.
///
/// Not `Clone`: `BlowupClient` holds a hyper client that isn't cheaply
/// cloneable. rmcp's `ServerHandler` doesn't require `Clone` on the
/// service itself — `.serve()` takes `self` by value and the handler
/// is stored by the running service.
pub struct BlowupService {
    client: BlowupClient,
}

impl BlowupService {
    pub fn new() -> Self {
        Self {
            client: BlowupClient::new(),
        }
    }
}

impl Default for BlowupService {
    fn default() -> Self {
        Self::new()
    }
}

/// Convert our typed bridge error into rmcp's wire-level `ErrorData`.
///
/// The `user_message()` string (with its `[FATAL] ` prefix for
/// non-retryable errors, and inline `\n提示: ...` for retryable ones)
/// is what Claude's skill prompt pattern-matches on. Raw `Display`
/// would drop both, so we route through `user_message()`.
///
/// All variants map to JSON-RPC `INTERNAL_ERROR` (-32603). We
/// intentionally don't split BadRequest → INVALID_PARAMS: the MCP
/// error code isn't surfaced to Claude, only the message is, and
/// keeping a single code simplifies T8/T9.
impl From<McpError> for ErrorData {
    fn from(err: McpError) -> Self {
        ErrorData::internal_error(err.user_message(), None)
    }
}

#[tool_router]
impl BlowupService {
    /// 探测 desktop app 是否在线。无副作用,返回 desktop 的健康响应。
    /// 调试 skill bridge 时用。
    #[tool(
        description = "探测 blowup desktop app 是否在线。调用 /api/v1/health,成功返回 \"ok\"。无副作用,主要用于调试 skill bridge 是否已开启。"
    )]
    pub async fn ping(&self) -> Result<String, ErrorData> {
        let _: serde_json::Value = self.client.get("/api/v1/health", None).await?;
        Ok("ok".to_string())
    }
}

// `name = ...` is required because the macro's default for
// `Implementation::from_build_env()` is resolved *inside* the `rmcp`
// crate, so without an explicit name the server identifies itself as
// `rmcp 1.4.0` instead of `blowup-mcp`. Version is left unset so it
// falls back to `env!("CARGO_PKG_VERSION")` at our call site.
#[tool_handler(name = "blowup-mcp")]
impl ServerHandler for BlowupService {}
