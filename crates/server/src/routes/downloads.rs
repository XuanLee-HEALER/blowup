use axum::extract::{Path, State};
use axum::{Json, Router, routing::get, routing::post};
use blowup_core::torrent::download::{self as svc, DownloadRecord, StartDownloadRequest};
use blowup_core::torrent::manager::TorrentFileInfo;
use serde::Deserialize;

use crate::error::{ApiError, ApiResult};
use crate::state::AppState;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/downloads", get(list_downloads).post(start_download))
        .route("/downloads/torrent-files", post(get_torrent_files))
        .route("/downloads/{id}/pause", post(pause))
        .route("/downloads/{id}/resume", post(resume))
        .route("/downloads/{id}", axum::routing::delete(delete_download))
        .route("/downloads/{id}/redownload", post(redownload))
        .route("/downloads/{id}/existing-files", get(list_existing_files))
}

async fn list_downloads(
    State(state): State<AppState>,
) -> ApiResult<Json<Vec<DownloadRecord>>> {
    let records = svc::list_downloads(&state.db)
        .await
        .map_err(ApiError::Internal)?;
    Ok(Json(records))
}

#[derive(Deserialize)]
pub struct TargetBody {
    pub target: String,
}

async fn get_torrent_files(
    State(state): State<AppState>,
    Json(req): Json<TargetBody>,
) -> ApiResult<Json<Vec<TorrentFileInfo>>> {
    let tm = state.torrent().map_err(ApiError::Internal)?;
    let files = tm
        .get_torrent_files(&req.target)
        .await
        .map_err(ApiError::Internal)?;
    Ok(Json(files))
}

async fn start_download(
    State(state): State<AppState>,
    Json(req): Json<StartDownloadRequest>,
) -> ApiResult<Json<i64>> {
    let tm = state.torrent().map_err(ApiError::Internal)?;
    svc::validate_library_root(&state.library_index).map_err(ApiError::Internal)?;
    let output_folder = state
        .library_index
        .compute_download_path(&req.director, req.tmdb_id);

    let download_id = svc::insert_download_record(&state.db, &req).await?;

    let trackers = state.tracker.hot_trackers().await;
    let (torrent_id, _handle) = match tm
        .start_download(
            &req.target,
            output_folder.clone(),
            req.only_files.clone(),
            Some(trackers),
        )
        .await
    {
        Ok(r) => r,
        Err(e) => {
            svc::mark_failed(&state.db, download_id, &e).await;
            return Err(ApiError::Internal(e));
        }
    };

    svc::set_torrent_id(&state.db, download_id, torrent_id as i64).await?;
    // Note: standalone server mode does not spawn a progress monitor task yet —
    // iOS/LAN clients poll list_downloads for updates. Embedded-mode Tauri
    // continues to run its own monitor that writes back to the same DB.
    Ok(Json(download_id))
}

async fn pause(State(state): State<AppState>, Path(id): Path<i64>) -> ApiResult<()> {
    let record = svc::get_active_download(&state.db, id, "downloading")
        .await
        .map_err(ApiError::from)?;
    let tm = state.torrent().map_err(ApiError::Internal)?;
    if let Some(tid) = record.torrent_id {
        tm.pause(tid as usize).await.ok();
    }
    svc::mark_paused(&state.db, id).await?;
    Ok(())
}

async fn resume(State(state): State<AppState>, Path(id): Path<i64>) -> ApiResult<()> {
    let record = svc::get_active_download(&state.db, id, "paused")
        .await
        .map_err(ApiError::from)?;
    let tm = state.torrent().map_err(ApiError::Internal)?;
    let resumed = match record.torrent_id {
        Some(tid) => tm.unpause(tid as usize).await.is_ok(),
        None => false,
    };
    if resumed {
        svc::mark_resumed(&state.db, id).await?;
        return Ok(());
    }

    let tmdb_id = record.tmdb_id.unwrap_or(0) as u64;
    let director = record.director.unwrap_or_else(|| "Unknown".to_string());
    let output_folder = state.library_index.compute_download_path(&director, tmdb_id);

    let trackers = state.tracker.hot_trackers().await;
    let (torrent_id, _handle) = tm
        .start_download(&record.target, output_folder, None, Some(trackers))
        .await
        .map_err(ApiError::Internal)?;
    svc::mark_resumed_with_new_torrent(&state.db, id, torrent_id as i64).await?;
    Ok(())
}

async fn delete_download(State(state): State<AppState>, Path(id): Path<i64>) -> ApiResult<()> {
    let record = svc::get_download_record(&state.db, id)
        .await
        .map_err(ApiError::from)?;

    let is_active = matches!(record.status.as_str(), "downloading" | "paused" | "pending");

    if is_active {
        if let Ok(tm) = state.torrent()
            && let Some(tid) = record.torrent_id
        {
            tm.remove(tid as usize).await.ok();
        }
        let tmdb_id = record.tmdb_id.unwrap_or(0) as u64;
        let director = record.director.as_deref().unwrap_or("Unknown");
        let dir = state.library_index.compute_download_path(director, tmdb_id);
        if dir.exists() {
            std::fs::remove_dir_all(&dir).ok();
        }
        state.library_index.remove_entry(tmdb_id);
    }

    svc::delete_download_record(&state.db, id).await?;
    Ok(())
}

#[derive(Deserialize)]
pub struct RedownloadBody {
    pub only_files: Option<Vec<usize>>,
}

async fn redownload(
    State(state): State<AppState>,
    Path(id): Path<i64>,
    Json(req): Json<RedownloadBody>,
) -> ApiResult<()> {
    let record = svc::get_redownload_record(&state.db, id)
        .await
        .map_err(ApiError::from)?;
    let tmdb_id = record.tmdb_id.unwrap_or(0) as u64;
    svc::validate_library_root(&state.library_index).map_err(ApiError::Internal)?;
    let director = record.director.unwrap_or_else(|| "Unknown".to_string());
    let output_folder = state.library_index.compute_download_path(&director, tmdb_id);

    let tm = state.torrent().map_err(ApiError::Internal)?;
    let trackers = state.tracker.hot_trackers().await;
    let (torrent_id, _handle) = tm
        .start_download(&record.target, output_folder, req.only_files, Some(trackers))
        .await
        .map_err(ApiError::Internal)?;

    svc::reset_for_redownload(&state.db, id, torrent_id as i64).await?;
    Ok(())
}

async fn list_existing_files(
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> ApiResult<Json<Vec<String>>> {
    let record = svc::get_download_record(&state.db, id)
        .await
        .map_err(ApiError::from)?;

    let tmdb_id = record.tmdb_id.unwrap_or(0) as u64;
    let director = record.director.as_deref().unwrap_or("Unknown");
    let dir = state.library_index.compute_download_path(director, tmdb_id);

    if !dir.exists() {
        return Ok(Json(Vec::new()));
    }

    let files = std::fs::read_dir(&dir)
        .map_err(|e| ApiError::Internal(e.to_string()))?
        .filter_map(|e| e.ok())
        .filter(|e| e.path().is_file())
        .filter_map(|e| e.file_name().into_string().ok())
        .collect();

    Ok(Json(files))
}

