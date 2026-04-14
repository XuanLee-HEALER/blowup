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
use rmcp::handler::server::wrapper::{Json, Parameters};
use rmcp::schemars::JsonSchema;
use rmcp::{ErrorData, ServerHandler, tool, tool_handler, tool_router};
use serde::{Deserialize, Serialize};

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

// ── Bridge-side DTOs ───────────────────────────────────────────────
//
// These mirror the JSON shapes returned by `crates/server/src/routes/entries.rs`
// (which re-exports the types in `blowup_core::entries::model`). The server types
// don't derive `Deserialize` (they're write-only on the server side), so we
// redeclare them here with the shapes Claude cares about.
//
// Keep in sync with `crates/core/src/entries/model.rs`. If that file adds a
// field, either add it here too or let serde ignore it by default (extra JSON
// fields are silently dropped during deserialization — but missing fields the
// struct declares will fail, so we match what the server actually returns).

/// Mirrors `blowup_core::entries::model::EntrySummary`.
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct EntrySummary {
    pub id: i64,
    pub name: String,
    pub tags: Vec<String>,
    pub updated_at: String,
}

/// Wrapper for `list_entries` tool output. The MCP spec requires every
/// tool's `outputSchema` root to be an `object`, so a bare `Vec<T>` at
/// the top level is rejected at `serve` time — wrap it in a struct with
/// a named `entries` field.
#[derive(Debug, Serialize, JsonSchema)]
pub struct EntryList {
    pub entries: Vec<EntrySummary>,
}

/// Wrapper for `list_all_tags` / `list_relation_types` — same MCP root
/// constraint as `EntryList`.
#[derive(Debug, Serialize, JsonSchema)]
pub struct StringList {
    pub items: Vec<String>,
}

/// Mirrors `blowup_core::entries::model::RelationEntry`.
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct RelationEntry {
    pub id: i64,
    pub target_id: i64,
    pub target_name: String,
    /// "outgoing" or "incoming" (relative to the entry being viewed).
    pub direction: String,
    pub relation_type: String,
}

/// Mirrors `blowup_core::entries::model::EntryDetail`.
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct EntryDetail {
    pub id: i64,
    pub name: String,
    pub wiki: String,
    pub tags: Vec<String>,
    pub relations: Vec<RelationEntry>,
    pub created_at: String,
    pub updated_at: String,
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
    /// 探测 desktop app 是否在线。无副作用,返回 desktop 的健康响应。
    /// 调试 skill bridge 时用。
    #[tool(
        description = "探测 blowup desktop app 是否在线。调用 /api/v1/health,成功返回 \"ok\"。无副作用,主要用于调试 skill bridge 是否已开启。"
    )]
    pub async fn ping(&self) -> Result<String, ErrorData> {
        let _: serde_json::Value = self.client.get("/api/v1/health", None).await?;
        Ok("ok".to_string())
    }

    /// 列出知识库条目。可选按名称子串(query)和/或标签(tag)过滤。
    /// 写新条目前必须先用此工具查重 — 同名条目存在时应改为 update_wiki。
    #[tool(
        description = "列出知识库条目。可选按名称子串(query)和/或标签(tag)过滤。写新条目前必须先用此工具查重 — 同名条目存在时应改为 update_wiki,不要新建。"
    )]
    pub async fn list_entries(
        &self,
        Parameters(args): Parameters<ListEntriesArgs>,
    ) -> Result<Json<EntryList>, ErrorData> {
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
        let entries: Vec<EntrySummary> = self.client.get(&path, None).await?;
        Ok(Json(EntryList { entries }))
    }

    /// 获取单个条目的完整内容(包括 wiki markdown、标签、关系)。
    /// 若返回 NotFound,先用 list_entries 查询正确的 ID。
    #[tool(
        description = "获取单个条目的完整内容(包括 wiki markdown、标签、关系)。参数 id 来自 list_entries 或 create_entry 的返回值。若返回 NotFound,先用 list_entries 查询正确的 ID。"
    )]
    pub async fn get_entry(
        &self,
        Parameters(args): Parameters<GetEntryArgs>,
    ) -> Result<Json<EntryDetail>, ErrorData> {
        let path = format!("/api/v1/entries/{}", args.id);
        let detail: EntryDetail = self
            .client
            .get(&path, Some("先用 list_entries 查询正确的 ID"))
            .await?;
        Ok(Json(detail))
    }

    /// 列出知识库中已使用的全部标签。
    #[tool(
        description = "列出知识库中已使用的全部标签。写条目前调用一次,从中挑选最匹配的现有标签,只在确实没有合适标签时才用 add_tag 创建新标签。这避免标签碎片化(\"导演\" / \"电影导演\" / \"导演角色\" 同时存在)。"
    )]
    pub async fn list_all_tags(&self) -> Result<Json<StringList>, ErrorData> {
        let items: Vec<String> = self.client.get("/api/v1/entries/tags", None).await?;
        Ok(Json(StringList { items }))
    }

    /// 列出知识库中已使用的全部关系类型。
    #[tool(
        description = "列出知识库中已使用的全部关系类型(如 \"导演了\"、\"主演了\"、\"属于流派\")。调用 add_relation 前必须先用此工具查询 — 关系类型是用户自定义字符串,没有固定枚举,但要复用现有的而不是发明新的。"
    )]
    pub async fn list_relation_types(&self) -> Result<Json<StringList>, ErrorData> {
        let items: Vec<String> = self
            .client
            .get("/api/v1/entries/relation-types", None)
            .await?;
        Ok(Json(StringList { items }))
    }
}

// `name = ...` is required because the macro's default for
// `Implementation::from_build_env()` is resolved *inside* the `rmcp`
// crate, so without an explicit name the server identifies itself as
// `rmcp 1.4.0` instead of `blowup-mcp`. Version is left unset so it
// falls back to `env!("CARGO_PKG_VERSION")` at our call site.
#[tool_handler(name = "blowup-mcp")]
impl ServerHandler for BlowupService {}
