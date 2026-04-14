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
