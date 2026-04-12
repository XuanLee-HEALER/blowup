//! Task registry commands — list and dismiss long-running jobs.
//!
//! Tasks themselves are started by the domain-specific commands
//! (e.g. `align_to_audio_cmd`) which call
//! `blowup_core::tasks::service::run_*`. These two commands are
//! just the query / cleanup surface exposed to the frontend.

use blowup_core::infra::events::{DomainEvent, EventBus};
use blowup_core::tasks::{TaskRecord, TaskRegistry};

#[tauri::command]
pub async fn list_tasks(tasks: tauri::State<'_, TaskRegistry>) -> Result<Vec<TaskRecord>, String> {
    Ok(tasks.list().await)
}

#[tauri::command]
pub async fn dismiss_task(
    tasks: tauri::State<'_, TaskRegistry>,
    events: tauri::State<'_, EventBus>,
    id: String,
) -> Result<(), String> {
    tasks.dismiss(&id).await;
    events.publish(DomainEvent::TasksChanged);
    Ok(())
}
