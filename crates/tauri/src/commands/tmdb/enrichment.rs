use blowup_core::library::index::{IndexEntry, LibraryIndex};
use blowup_core::tmdb::service;
use tauri::Emitter;

#[tauri::command]
pub async fn enrich_index_entry(
    app: tauri::AppHandle,
    tmdb_id: u64,
    force: Option<bool>,
    index: tauri::State<'_, LibraryIndex>,
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

    if result.is_ok()
        && let Err(e) = app.emit("library:changed", ())
    {
        tracing::warn!(error = %e, "failed to emit library:changed");
    }
    result
}
