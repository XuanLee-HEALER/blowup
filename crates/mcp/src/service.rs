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

/// Bridge service exposing the 9 knowledge-base tools over stdio MCP.
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

/// Wrapper for `create_entry` tool output. MCP outputSchema root must be object.
#[derive(Debug, Serialize, JsonSchema)]
pub struct EntryIdResponse {
    pub id: i64,
}

/// Wrapper for `add_relation` tool output. MCP outputSchema root must be object.
#[derive(Debug, Serialize, JsonSchema)]
pub struct RelationIdResponse {
    pub id: i64,
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

// Each tool's user-visible documentation lives in its
// `#[tool(description = "...")]` attribute — that string lands in
// `tools/list` verbatim and is what Claude actually reads. We
// deliberately don't duplicate it as a rustdoc comment because the
// rustdoc gets stripped at compile time and never reaches the wire,
// so the two would drift out of sync.
#[tool_router]
impl BlowupService {
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

    #[tool(
        description = "列出知识库中已使用的全部标签。写条目前调用一次,从中挑选最匹配的现有标签,只在确实没有合适标签时才用 add_tag 创建新标签。这避免标签碎片化(\"导演\" / \"电影导演\" / \"导演角色\" 同时存在)。"
    )]
    pub async fn list_all_tags(&self) -> Result<Json<StringList>, ErrorData> {
        let items: Vec<String> = self.client.get("/api/v1/entries/tags", None).await?;
        Ok(Json(StringList { items }))
    }

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

    #[tool(
        description = "创建一个新的知识库条目并返回其 ID。**调用前必须**先用 list_entries(query=name) 查重 — 同名条目存在时应改为 update_wiki,而不是新建。条目名称规则:中文,不含书名号、引号、年份后缀。"
    )]
    pub async fn create_entry(
        &self,
        Parameters(args): Parameters<CreateEntryArgs>,
    ) -> Result<Json<EntryIdResponse>, ErrorData> {
        #[derive(serde::Serialize)]
        struct Body<'a> {
            name: &'a str,
        }
        let id: i64 = self
            .client
            .post(
                "/api/v1/entries",
                &Body { name: &args.name },
                Some("先用 list_entries(query=name) 查重"),
            )
            .await?;
        Ok(Json(EntryIdResponse { id }))
    }

    // ── Void-write convention ────────────────────────────────────
    //
    // The three void writes (update_wiki, update_name, add_tag)
    // return `Result<String, ErrorData>` with a literal `"ok"` on
    // success rather than `Result<Json<OkResponse>, ErrorData>`.
    //
    // Why: rmcp's MCP outputSchema constraint requires the root JSON
    // to be an object, so we can't return a bare `()`/`null`. The
    // alternatives are (a) wrap "ok" in a one-field struct, or
    // (b) return a `String` literal — rmcp's `IntoCallToolResult` for
    // `String` produces `{ "content": [{ "type": "text", "text": "ok" }] }`
    // which IS a valid object root. (b) is fewer types and equally clear
    // to Claude. If rmcp ever changes how `String` results are encoded
    // in a way that breaks the object-root guarantee, switch to (a).
    #[tool(
        description = "更新条目的 wiki markdown 内容。**完全覆盖**,不是 append。若是更新而非新写,先用 get_entry 拿到现有内容再合并。Wiki 内容应为中文 Markdown。"
    )]
    pub async fn update_wiki(
        &self,
        Parameters(args): Parameters<UpdateWikiArgs>,
    ) -> Result<String, ErrorData> {
        #[derive(serde::Serialize)]
        struct Body<'a> {
            wiki: &'a str,
        }
        let path = format!("/api/v1/entries/{}/wiki", args.id);
        let _: () = self
            .client
            .put(
                &path,
                &Body { wiki: &args.wiki },
                Some("条目 ID 不存在时,先用 list_entries 查询"),
            )
            .await?;
        Ok("ok".to_string())
    }

    #[tool(
        description = "更新条目的名称。规则同 create_entry:中文,不含书名号、引号、年份后缀。"
    )]
    pub async fn update_name(
        &self,
        Parameters(args): Parameters<UpdateNameArgs>,
    ) -> Result<String, ErrorData> {
        #[derive(serde::Serialize)]
        struct Body<'a> {
            name: &'a str,
        }
        let path = format!("/api/v1/entries/{}/name", args.id);
        let _: () = self
            .client
            .put(&path, &Body { name: &args.name }, None)
            .await?;
        Ok("ok".to_string())
    }

    #[tool(
        description = "给条目添加一个标签。**优先使用 list_all_tags 返回的现有标签**,只在确实没有合适标签时才创建新标签 — 这避免标签碎片化。"
    )]
    pub async fn add_tag(
        &self,
        Parameters(args): Parameters<AddTagArgs>,
    ) -> Result<String, ErrorData> {
        #[derive(serde::Serialize)]
        struct Body<'a> {
            tag: &'a str,
        }
        let path = format!("/api/v1/entries/{}/tags", args.entry_id);
        let _: () = self
            .client
            .post(
                &path,
                &Body { tag: &args.tag },
                Some("先用 list_all_tags 检查是否有合适的现有标签"),
            )
            .await?;
        Ok("ok".to_string())
    }

    #[tool(
        description = "在两个条目之间添加一条关系,返回关系 ID。**调用前必须**先用 list_relation_types 查询现有类型并复用,不要发明同义的新类型(例如不要用 \"拍了\" 当 \"导演了\" 已经存在)。"
    )]
    pub async fn add_relation(
        &self,
        Parameters(args): Parameters<AddRelationArgs>,
    ) -> Result<Json<RelationIdResponse>, ErrorData> {
        #[derive(serde::Serialize)]
        struct Body<'a> {
            from_id: i64,
            to_id: i64,
            relation_type: &'a str,
        }
        let id: i64 = self
            .client
            .post(
                "/api/v1/entries/relations",
                &Body {
                    from_id: args.from_id,
                    to_id: args.to_id,
                    relation_type: &args.relation_type,
                },
                Some("先用 list_relation_types 查询现有类型并复用"),
            )
            .await?;
        Ok(Json(RelationIdResponse { id }))
    }
}

// `name = ...` is required because the macro's default for
// `Implementation::from_build_env()` is resolved *inside* the `rmcp`
// crate, so without an explicit name the server identifies itself as
// `rmcp 1.4.0` instead of `blowup-mcp`. Version is left unset so it
// falls back to `env!("CARGO_PKG_VERSION")` at our call site.
#[tool_handler(name = "blowup-mcp")]
impl ServerHandler for BlowupService {}
