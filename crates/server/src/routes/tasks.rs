use axum::extract::{Path, State};
use axum::{Json, Router, routing::delete, routing::get};
use blowup_core::tasks::TaskRecord;

use crate::error::ApiResult;
use crate::state::AppState;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/tasks", get(list_tasks))
        .route("/tasks/{id}", delete(dismiss_task))
}

async fn list_tasks(State(state): State<AppState>) -> Json<Vec<TaskRecord>> {
    Json(state.tasks.list().await)
}

async fn dismiss_task(State(state): State<AppState>, Path(id): Path<String>) -> ApiResult<()> {
    state.tasks.dismiss(&id).await;
    // Always publish — dismissing a non-existent id is a no-op but
    // the frontend querying to refresh after its own click is harmless.
    state
        .events
        .publish(blowup_core::infra::events::DomainEvent::TasksChanged);
    Ok(())
}
