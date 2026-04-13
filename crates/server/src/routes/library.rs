use axum::extract::{Path, State};
use axum::{Json, Router, routing::delete, routing::get, routing::post};
use blowup_core::infra::events::DomainEvent;
use blowup_core::library::index::{IndexEntry, SubtitleDisplayConfig};
use blowup_core::library::items::{
    self as svc, LibraryItemDetail, LibraryItemSummary, LibraryStats, ScanResult,
};
use serde::Deserialize;
use std::collections::{BTreeMap, HashMap};

use crate::error::ApiResult;
use crate::state::AppState;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/library/items", get(list_items).post(add_item))
        .route("/library/items/{id}", get(get_item).delete(remove_item))
        .route("/library/items/scan", post(scan))
        .route("/library/items/{id}/assets", post(add_asset))
        .route("/library/assets/{id}", delete(remove_asset))
        .route("/library/stats", get(stats))
        .route("/library/index", get(list_index))
        .route("/library/index/by-director", get(index_by_director))
        .route("/library/index/search", post(search_index))
        .route("/library/index/rebuild", post(rebuild))
        .route("/library/resources", post(delete_resource))
        .route("/library/index/{tmdb_id}/refresh", post(refresh_entry))
        .route("/library/index/{tmdb_id}", delete(delete_film_directory))
        .route(
            "/library/index/{tmdb_id}/subtitle-configs",
            post(save_subtitle_configs),
        )
}

#[derive(Deserialize)]
pub struct FilePathBody {
    pub file_path: String,
}

async fn add_item(
    State(state): State<AppState>,
    Json(req): Json<FilePathBody>,
) -> ApiResult<Json<i64>> {
    let id = svc::add_library_item(&state.db, &req.file_path)
        .await
        .map_err(crate::error::ApiError::Internal)?;
    state.events.publish(DomainEvent::LibraryChanged);
    Ok(Json(id))
}

async fn list_items(State(state): State<AppState>) -> ApiResult<Json<Vec<LibraryItemSummary>>> {
    let items = svc::list_library_items(&state.db)
        .await
        .map_err(crate::error::ApiError::Internal)?;
    Ok(Json(items))
}

async fn get_item(
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> ApiResult<Json<LibraryItemDetail>> {
    let detail = svc::get_library_item(&state.db, id)
        .await
        .map_err(crate::error::ApiError::from)?;
    Ok(Json(detail))
}

async fn remove_item(State(state): State<AppState>, Path(id): Path<i64>) -> ApiResult<()> {
    svc::remove_library_item(&state.db, id)
        .await
        .map_err(crate::error::ApiError::Internal)?;
    state.events.publish(DomainEvent::LibraryChanged);
    Ok(())
}

#[derive(Deserialize)]
pub struct ScanBody {
    pub dir_path: String,
}

async fn scan(
    State(state): State<AppState>,
    Json(req): Json<ScanBody>,
) -> ApiResult<Json<ScanResult>> {
    let result = svc::scan_library_directory(&state.db, &req.dir_path)
        .await
        .map_err(crate::error::ApiError::Internal)?;
    state.events.publish(DomainEvent::LibraryChanged);
    Ok(Json(result))
}

#[derive(Deserialize)]
pub struct AssetBody {
    pub asset_type: String,
    pub file_path: String,
    pub lang: Option<String>,
}

async fn add_asset(
    State(state): State<AppState>,
    Path(item_id): Path<i64>,
    Json(req): Json<AssetBody>,
) -> ApiResult<Json<i64>> {
    let id = svc::add_library_asset(
        &state.db,
        item_id,
        &req.asset_type,
        &req.file_path,
        req.lang.as_deref(),
    )
    .await
    .map_err(crate::error::ApiError::Internal)?;
    state.events.publish(DomainEvent::LibraryChanged);
    Ok(Json(id))
}

async fn remove_asset(State(state): State<AppState>, Path(id): Path<i64>) -> ApiResult<()> {
    svc::remove_library_asset(&state.db, id)
        .await
        .map_err(crate::error::ApiError::Internal)?;
    state.events.publish(DomainEvent::LibraryChanged);
    Ok(())
}

async fn stats(State(state): State<AppState>) -> ApiResult<Json<LibraryStats>> {
    let stats = svc::get_library_stats(&state.db)
        .await
        .map_err(crate::error::ApiError::Internal)?;
    Ok(Json(stats))
}

// ── In-memory library index endpoints ────────────────────────────

async fn list_index(State(state): State<AppState>) -> Json<Vec<IndexEntry>> {
    Json(state.library_index.list_entries())
}

async fn index_by_director(
    State(state): State<AppState>,
) -> Json<BTreeMap<String, Vec<IndexEntry>>> {
    Json(state.library_index.list_by_director())
}

#[derive(Deserialize)]
pub struct IndexSearchBody {
    pub query: Option<String>,
    pub year_from: Option<u32>,
    pub year_to: Option<u32>,
    pub genre: Option<String>,
}

async fn search_index(
    State(state): State<AppState>,
    Json(req): Json<IndexSearchBody>,
) -> Json<Vec<IndexEntry>> {
    Json(state.library_index.search(
        req.query.as_deref(),
        req.year_from,
        req.year_to,
        req.genre.as_deref(),
    ))
}

async fn rebuild(State(state): State<AppState>) -> Json<()> {
    state.library_index.rebuild_from_disk();
    state.events.publish(DomainEvent::LibraryChanged);
    Json(())
}

async fn delete_resource(
    State(state): State<AppState>,
    Json(req): Json<FilePathBody>,
) -> ApiResult<()> {
    svc::delete_library_resource(&state.db, &req.file_path)
        .await
        .map_err(crate::error::ApiError::Internal)?;
    state.events.publish(DomainEvent::LibraryChanged);
    Ok(())
}

async fn refresh_entry(State(state): State<AppState>, Path(tmdb_id): Path<u64>) -> Json<()> {
    state.library_index.update_files(tmdb_id);
    state.events.publish(DomainEvent::LibraryChanged);
    Json(())
}

/// Delete a film directory from disk + index. Server mode has no
/// player to guard against, so it removes unconditionally.
async fn delete_film_directory(
    State(state): State<AppState>,
    Path(tmdb_id): Path<u64>,
) -> ApiResult<()> {
    let entry = state
        .library_index
        .get_entry(tmdb_id)
        .ok_or_else(|| crate::error::ApiError::NotFound("索引中未找到该电影".to_string()))?;

    // Defensive: `.index.json` is user-owned and could contain a path
    // with `..` segments. Refuse to touch anything that isn't a plain
    // relative path underneath the library root.
    if !crate::path_guard::is_safe_relative_path(&entry.path) {
        return Err(crate::error::ApiError::Internal(format!(
            "library index entry {} has unsafe path: {}",
            tmdb_id, entry.path
        )));
    }

    let cfg = blowup_core::config::load_config();
    let root_dir = shellexpand::tilde(&cfg.library.root_dir).to_string();
    let film_dir = std::path::Path::new(&root_dir).join(&entry.path);
    if let Err(e) = std::fs::remove_dir_all(&film_dir) {
        tracing::warn!(
            error = %e,
            dir = %film_dir.display(),
            "failed to remove film directory"
        );
    }
    state.library_index.remove_entry(tmdb_id);
    state.events.publish(DomainEvent::LibraryChanged);
    Ok(())
}

async fn save_subtitle_configs(
    State(state): State<AppState>,
    Path(tmdb_id): Path<u64>,
    Json(configs): Json<HashMap<String, SubtitleDisplayConfig>>,
) -> Json<()> {
    state.library_index.save_subtitle_configs(tmdb_id, configs);
    Json(())
}
