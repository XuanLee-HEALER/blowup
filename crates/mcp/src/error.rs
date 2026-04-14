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
    code: ErrorCode,
    message: String,
    hint: Option<String>,
}

impl McpError {
    pub fn code(&self) -> ErrorCode {
        self.code
    }

    pub fn message(&self) -> &str {
        &self.message
    }

    pub fn hint(&self) -> Option<&str> {
        self.hint.as_deref()
    }
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

/// `Display` renders the raw message only — no `[FATAL] ` prefix, no
/// hint. Use this for tracing/logging so the structured error type
/// stays clean. Reserve `user_message()` for the MCP boundary where
/// Claude needs the prefix to pattern-match.
impl fmt::Display for McpError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
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
        // Lock down the exact format — the skill markdown pattern-matches
        // on `\n提示: ` so any drift here silently breaks the prompt.
        assert_eq!(
            e.user_message(),
            "条目 #999 不存在\n提示: 请先用 list_entries 查询"
        );
    }

    #[test]
    fn display_does_not_leak_fatal_prefix() {
        // Display is for logging/tracing — keep the [FATAL] marker out
        // of structured logs. Only the MCP boundary (user_message) sees it.
        let e = McpError::bridge_offline();
        let displayed = format!("{e}");
        assert!(!displayed.contains("[FATAL]"));
        assert!(displayed.contains("blowup app 未启用 skill bridge"));
    }

    #[test]
    fn retryable_classification() {
        assert!(ErrorCode::BadRequest.retryable());
        assert!(ErrorCode::NotFound.retryable());
        assert!(!ErrorCode::BridgeOffline.retryable());
        assert!(!ErrorCode::Internal.retryable());
    }
}
