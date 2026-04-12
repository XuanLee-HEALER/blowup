use axum::body::Bytes;
use axum::extract::Query;
use axum::http::{StatusCode, header};
use axum::response::IntoResponse;
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
        .route("/audio/peaks", post(get_audio_peaks))
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

#[derive(Deserialize)]
pub struct PeaksRequest {
    pub file: String,
}

/// Returns raw f32le mono @ 100Hz peaks as an `application/octet-stream`
/// body. iOS/LAN clients use this the same way the Tauri WebView does:
/// ArrayBuffer → Float32Array → WaveSurfer `peaks`.
async fn get_audio_peaks(Json(req): Json<PeaksRequest>) -> Response {
    match service::extract_audio_peaks(Path::new(&req.file)).await {
        Ok(bytes) => (
            StatusCode::OK,
            [(header::CONTENT_TYPE, "application/octet-stream")],
            Bytes::from(bytes),
        )
            .into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e).into_response(),
    }
}

type Response = axum::response::Response;
