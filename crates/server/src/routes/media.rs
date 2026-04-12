use axum::extract::{Query, State};
use axum::{Json, Router, routing::get, routing::post};
use blowup_core::library::index::FileMediaInfo;
use blowup_core::media::service::{self, MediaInfo};
use serde::Deserialize;

use crate::error::ApiResult;
use crate::state::AppState;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/media/probe", get(probe_media))
        .route("/media/probe-detail", get(probe_media_detail))
        .route("/media/probe-and-cache", post(probe_and_cache))
}

#[derive(Deserialize)]
pub struct FileQuery {
    pub file: String,
}

async fn probe_media(Query(q): Query<FileQuery>) -> ApiResult<Json<String>> {
    let stdout = service::probe_media(&q.file)
        .await
        .map_err(crate::error::ApiError::Internal)?;
    Ok(Json(stdout))
}

async fn probe_media_detail(Query(q): Query<FileQuery>) -> ApiResult<Json<MediaInfo>> {
    let info = service::probe_media_detail(&q.file)
        .await
        .map_err(crate::error::ApiError::Internal)?;
    Ok(Json(info))
}

#[derive(Deserialize)]
pub struct ProbeAndCacheRequest {
    pub tmdb_id: u64,
    pub filename: String,
}

async fn probe_and_cache(
    State(state): State<AppState>,
    Json(req): Json<ProbeAndCacheRequest>,
) -> ApiResult<Json<FileMediaInfo>> {
    let info = service::probe_and_cache(&state.library_index, req.tmdb_id, &req.filename)
        .await
        .map_err(crate::error::ApiError::from)?;
    Ok(Json(info))
}
