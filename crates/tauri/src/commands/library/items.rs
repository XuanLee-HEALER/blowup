use blowup_core::infra::events::{DomainEvent, EventBus};
use blowup_core::library::index::{IndexEntry, LibraryIndex, SubtitleDisplayConfig};
use blowup_core::library::items::{
    self as svc, LibraryItemDetail, LibraryItemSummary, LibraryStats, ScanResult,
};
use sqlx::SqlitePool;
use std::collections::{BTreeMap, HashMap};
use std::sync::Arc;

// ── Library item commands ───────────────────────────────────────

#[tauri::command]
pub async fn add_library_item(
    file_path: String,
    pool: tauri::State<'_, SqlitePool>,
) -> Result<i64, String> {
    svc::add_library_item(pool.inner(), &file_path).await
}

#[tauri::command]
pub async fn list_library_items(
    pool: tauri::State<'_, SqlitePool>,
) -> Result<Vec<LibraryItemSummary>, String> {
    svc::list_library_items(pool.inner()).await
}

#[tauri::command]
pub async fn get_library_item(
    id: i64,
    pool: tauri::State<'_, SqlitePool>,
) -> Result<LibraryItemDetail, String> {
    svc::get_library_item(pool.inner(), id).await
}

#[tauri::command]
pub async fn remove_library_item(
    id: i64,
    pool: tauri::State<'_, SqlitePool>,
) -> Result<(), String> {
    svc::remove_library_item(pool.inner(), id).await
}

#[tauri::command]
pub async fn scan_library_directory(
    dir_path: String,
    pool: tauri::State<'_, SqlitePool>,
) -> Result<ScanResult, String> {
    svc::scan_library_directory(pool.inner(), &dir_path).await
}

#[tauri::command]
pub async fn add_library_asset(
    item_id: i64,
    asset_type: String,
    file_path: String,
    lang: Option<String>,
    pool: tauri::State<'_, SqlitePool>,
) -> Result<i64, String> {
    svc::add_library_asset(
        pool.inner(),
        item_id,
        &asset_type,
        &file_path,
        lang.as_deref(),
    )
    .await
}

#[tauri::command]
pub async fn remove_library_asset(
    id: i64,
    pool: tauri::State<'_, SqlitePool>,
) -> Result<(), String> {
    svc::remove_library_asset(pool.inner(), id).await
}

#[tauri::command]
pub async fn get_library_stats(pool: tauri::State<'_, SqlitePool>) -> Result<LibraryStats, String> {
    svc::get_library_stats(pool.inner()).await
}

// ── Library Index commands (in-memory, no DB) ────────────────────

#[tauri::command]
pub fn list_index_entries(
    index: tauri::State<'_, Arc<LibraryIndex>>,
) -> Result<Vec<IndexEntry>, String> {
    Ok(index.list_entries())
}

#[tauri::command]
pub fn list_index_by_director(
    index: tauri::State<'_, Arc<LibraryIndex>>,
) -> Result<BTreeMap<String, Vec<IndexEntry>>, String> {
    Ok(index.list_by_director())
}

#[tauri::command]
pub fn search_index(
    index: tauri::State<'_, Arc<LibraryIndex>>,
    query: Option<String>,
    year_from: Option<u32>,
    year_to: Option<u32>,
    genre: Option<String>,
) -> Result<Vec<IndexEntry>, String> {
    Ok(index.search(query.as_deref(), year_from, year_to, genre.as_deref()))
}

#[tauri::command]
pub fn rebuild_index(
    events: tauri::State<'_, EventBus>,
    index: tauri::State<'_, Arc<LibraryIndex>>,
) -> Result<(), String> {
    index.rebuild_from_disk();
    events.publish(DomainEvent::LibraryChanged);
    Ok(())
}

#[tauri::command]
pub fn save_subtitle_configs(
    tmdb_id: u64,
    configs: HashMap<String, SubtitleDisplayConfig>,
    index: tauri::State<'_, Arc<LibraryIndex>>,
) -> Result<(), String> {
    index.save_subtitle_configs(tmdb_id, configs);
    Ok(())
}

#[tauri::command]
pub async fn delete_library_resource(
    file_path: String,
    pool: tauri::State<'_, SqlitePool>,
) -> Result<(), String> {
    svc::delete_library_resource(pool.inner(), &file_path).await
}

#[tauri::command]
pub fn refresh_index_entry(
    events: tauri::State<'_, EventBus>,
    tmdb_id: u64,
    index: tauri::State<'_, Arc<LibraryIndex>>,
) -> Result<(), String> {
    index.update_files(tmdb_id);
    events.publish(DomainEvent::LibraryChanged);
    Ok(())
}

/// Delete a film's entire directory from disk + index.
/// Guards against deleting files currently open in the player (Tauri only).
#[tauri::command]
pub async fn delete_film_directory(
    events: tauri::State<'_, EventBus>,
    tmdb_id: u64,
    index: tauri::State<'_, Arc<LibraryIndex>>,
) -> Result<(), String> {
    let entry = index
        .get_entry(tmdb_id)
        .ok_or_else(|| "索引中未找到该电影".to_string())?;

    // `.index.json` is user-owned; refuse to touch anything that isn't
    // a plain relative path underneath the library root.
    if !blowup_core::infra::paths::is_safe_relative_path(&entry.path) {
        return Err(format!(
            "library index entry {} has unsafe path: {}",
            tmdb_id, entry.path
        ));
    }

    let cfg = blowup_core::config::load_config();
    let root_dir = shellexpand::tilde(&cfg.library.root_dir).to_string();
    let film_dir = std::path::Path::new(&root_dir).join(&entry.path);

    // Block the delete if the player currently has a file open that
    // lives under this directory. Compare by path components rather
    // than string prefix so "Director/12345" doesn't accidentally match
    // "Director/123".
    if let Some(current_file) = crate::player::get_current_file_path() {
        let current_path = std::path::Path::new(&current_file);
        if current_path.starts_with(&film_dir) {
            return Err("播放器正在播放该电影的文件，请先关闭播放器".to_string());
        }
    }

    if let Err(e) = std::fs::remove_dir_all(&film_dir) {
        tracing::warn!(
            error = %e,
            dir = %film_dir.display(),
            "failed to remove film directory"
        );
    }

    index.remove_entry(tmdb_id);
    events.publish(DomainEvent::LibraryChanged);
    Ok(())
}
