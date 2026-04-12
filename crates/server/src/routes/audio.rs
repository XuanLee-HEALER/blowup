use axum::extract::Query;
use axum::{Json, Router, routing::get, routing::post};
use blowup_core::audio::service::{self, AudioStreamInfo};
use serde::Deserialize;
use std::path::Path;

use crate::error::ApiResult;
use crate::state::AppState;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/audio/streams", get(list_streams))
        .route("/audio/extract", post(extract_audio))
}

#[derive(Deserialize)]
pub struct FileQuery {
    pub file: String,
}

async fn list_streams(Query(q): Query<FileQuery>) -> ApiResult<Json<Vec<AudioStreamInfo>>> {
    let streams = service::list_audio_streams(Path::new(&q.file))
        .await
        .map_err(crate::error::ApiError::Internal)?;
    Ok(Json(streams))
}

#[derive(Deserialize)]
pub struct ExtractRequest {
    pub video: String,
    pub stream: u32,
    pub format: String,
}

async fn extract_audio(Json(req): Json<ExtractRequest>) -> ApiResult<Json<String>> {
    let out = service::extract_audio(Path::new(&req.video), req.stream, &req.format)
        .await
        .map_err(crate::error::ApiError::Internal)?;
    Ok(Json(out))
}
