use axum::extract::Query;
use axum::{Json, Router, routing::get, routing::post};
use blowup_core::subtitle::service::{
    self, AlignResult, SubEntry, SubtitleSearchResult, SubtitleStreamInfo,
};
use serde::Deserialize;
use std::path::Path;

use crate::error::ApiResult;
use crate::state::AppState;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/subtitle/streams", get(list_streams))
        .route("/subtitle/parse", get(parse_subtitle))
        .route("/subtitle/search", post(search_subtitles))
        .route("/subtitle/download", post(download_subtitle))
        .route("/subtitle/fetch", post(fetch_subtitle))
        .route("/subtitle/align", post(align_subtitle))
        .route("/subtitle/align-to-audio", post(align_to_audio))
        .route("/subtitle/extract", post(extract_subtitle))
        .route("/subtitle/shift", post(shift_subtitle))
}

#[derive(Deserialize)]
pub struct FileQuery {
    pub file: String,
}

async fn list_streams(Query(q): Query<FileQuery>) -> ApiResult<Json<Vec<SubtitleStreamInfo>>> {
    let streams = service::list_all_subtitle_stream(Path::new(&q.file))
        .await
        .map_err(|e| crate::error::ApiError::Internal(e.to_string()))?;
    Ok(Json(streams))
}

async fn parse_subtitle(Query(q): Query<FileQuery>) -> ApiResult<Json<Vec<SubEntry>>> {
    let entries = service::parse_subtitle_file(Path::new(&q.file))
        .map_err(crate::error::ApiError::Internal)?;
    Ok(Json(entries))
}

#[derive(Deserialize)]
pub struct SearchRequest {
    pub video: String,
    pub lang: String,
    pub title: Option<String>,
    pub year: Option<u32>,
    pub tmdb_id: Option<u64>,
}

async fn search_subtitles(
    Json(req): Json<SearchRequest>,
) -> ApiResult<Json<Vec<SubtitleSearchResult>>> {
    let cfg = blowup_core::config::load_config();
    let results = service::search_with_priority(
        Path::new(&req.video),
        &req.lang,
        req.title.as_deref(),
        req.year,
        req.tmdb_id,
        &cfg,
    )
    .await
    .map_err(crate::error::ApiError::Internal)?;
    Ok(Json(results))
}

#[derive(Deserialize)]
pub struct DownloadRequest {
    pub video: String,
    pub lang: String,
    pub download_id: String,
}

async fn download_subtitle(Json(req): Json<DownloadRequest>) -> ApiResult<()> {
    let cfg = blowup_core::config::load_config();
    service::download_by_id(Path::new(&req.video), &req.lang, &req.download_id, &cfg)
        .await
        .map_err(crate::error::ApiError::Internal)?;
    Ok(())
}

#[derive(Deserialize)]
pub struct FetchRequest {
    pub video: String,
    pub lang: String,
}

async fn fetch_subtitle(Json(req): Json<FetchRequest>) -> ApiResult<()> {
    let cfg = blowup_core::config::load_config();
    service::fetch_subtitle(Path::new(&req.video), &req.lang, &cfg)
        .await
        .map_err(|e| crate::error::ApiError::Internal(e.to_string()))?;
    Ok(())
}

#[derive(Deserialize)]
pub struct AlignRequest {
    pub video: String,
    pub srt: String,
}

async fn align_subtitle(Json(req): Json<AlignRequest>) -> ApiResult<()> {
    service::align_subtitle(Path::new(&req.video), Path::new(&req.srt))
        .await
        .map_err(|e| crate::error::ApiError::Internal(e.to_string()))?;
    Ok(())
}

#[derive(Deserialize)]
pub struct AlignToAudioRequest {
    pub srt: String,
    pub audio: String,
}

async fn align_to_audio(Json(req): Json<AlignToAudioRequest>) -> ApiResult<Json<AlignResult>> {
    let result = service::align_subtitle_to_audio(Path::new(&req.srt), Path::new(&req.audio))
        .await
        .map_err(|e| crate::error::ApiError::Internal(e.to_string()))?;
    Ok(Json(result))
}

#[derive(Deserialize)]
pub struct ExtractRequest {
    pub video: String,
    pub stream: Option<u32>,
}

async fn extract_subtitle(Json(req): Json<ExtractRequest>) -> ApiResult<()> {
    service::extract_sub_srt(Path::new(&req.video), req.stream)
        .await
        .map_err(|e| crate::error::ApiError::Internal(e.to_string()))?;
    Ok(())
}

#[derive(Deserialize)]
pub struct ShiftRequest {
    pub srt: String,
    pub offset_ms: i64,
}

async fn shift_subtitle(Json(req): Json<ShiftRequest>) -> ApiResult<()> {
    service::shift_srt(Path::new(&req.srt), req.offset_ms)
        .map_err(|e| crate::error::ApiError::Internal(e.to_string()))?;
    Ok(())
}
