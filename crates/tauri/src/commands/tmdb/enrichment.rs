use blowup_core::infra::events::{DomainEvent, EventBus};
use blowup_core::library::index::{IndexEntry, LibraryIndex};
use blowup_core::tmdb::service;
use std::sync::Arc;

#[tauri::command]
pub async fn enrich_index_entry(
    events: tauri::State<'_, EventBus>,
    tmdb_id: u64,
    force: Option<bool>,
    index: tauri::State<'_, Arc<LibraryIndex>>,
) -> Result<IndexEntry, String> {
    let cfg = blowup_core::config::load_config();
    let library_root = std::path::PathBuf::from(shellexpand::tilde(&cfg.library.root_dir).as_ref());
    let client = reqwest::Client::new();

    let result = service::enrich_index_entry(
        &client,
        &cfg.tmdb.api_key,
        &library_root,
        index.inner(),
        tmdb_id,
        force.unwrap_or(false),
    )
    .await;

    if result.is_ok() {
        events.publish(DomainEvent::LibraryChanged);
    }
    result
}
