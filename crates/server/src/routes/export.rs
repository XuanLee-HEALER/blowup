use axum::extract::State;
use axum::{Json, Router, routing::get, routing::post};
use blowup_core::export::service;
use serde::Deserialize;
use std::path::Path;

use crate::error::{ApiError, ApiResult};
use crate::state::AppState;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/export/knowledge-base", post(export_kb))
        .route("/import/knowledge-base", post(import_kb))
        .route("/export/config", post(export_config))
        .route("/import/config", post(import_config))
        .route("/export/knowledge-base/s3", post(export_kb_s3))
        .route("/import/knowledge-base/s3", post(import_kb_s3))
        .route("/export/config/s3", post(export_config_s3))
        .route("/import/config/s3", post(import_config_s3))
        .route("/s3/test", get(test_s3))
}

#[derive(Deserialize)]
pub struct PathBody {
    pub path: String,
}

async fn export_kb(State(state): State<AppState>, Json(req): Json<PathBody>) -> ApiResult<()> {
    service::export_knowledge_base_to_file(&state.db, Path::new(&req.path))
        .await
        .map_err(ApiError::Internal)?;
    Ok(())
}

async fn import_kb(
    State(state): State<AppState>,
    Json(req): Json<PathBody>,
) -> ApiResult<Json<String>> {
    let result = service::import_knowledge_base_from_file(&state.db, Path::new(&req.path))
        .await
        .map_err(ApiError::Internal)?;
    Ok(Json(result))
}

async fn export_config(Json(req): Json<PathBody>) -> ApiResult<()> {
    let cfg = blowup_core::config::load_config();
    service::export_config_to_file(&cfg, Path::new(&req.path)).map_err(ApiError::Internal)?;
    Ok(())
}

async fn import_config(Json(req): Json<PathBody>) -> ApiResult<()> {
    let dst = blowup_core::config::config_path();
    service::import_config_from_file(Path::new(&req.path), &dst).map_err(ApiError::Internal)?;
    Ok(())
}

async fn export_kb_s3(State(state): State<AppState>) -> ApiResult<()> {
    let cfg = blowup_core::config::load_config();
    service::export_knowledge_base_s3(&state.db, &cfg.sync)
        .await
        .map_err(ApiError::Internal)?;
    Ok(())
}

async fn import_kb_s3(State(state): State<AppState>) -> ApiResult<Json<String>> {
    let cfg = blowup_core::config::load_config();
    let result = service::import_knowledge_base_s3(&state.db, &cfg.sync)
        .await
        .map_err(ApiError::Internal)?;
    Ok(Json(result))
}

async fn export_config_s3() -> ApiResult<()> {
    let cfg = blowup_core::config::load_config();
    let sync = cfg.sync.clone();
    service::export_config_s3(&cfg, &sync)
        .await
        .map_err(ApiError::Internal)?;
    Ok(())
}

async fn import_config_s3() -> ApiResult<()> {
    let cfg = blowup_core::config::load_config();
    let dst = blowup_core::config::config_path();
    service::import_config_s3(&cfg.sync, &dst)
        .await
        .map_err(ApiError::Internal)?;
    Ok(())
}

async fn test_s3() -> ApiResult<Json<String>> {
    let cfg = blowup_core::config::load_config();
    let msg = service::test_s3_connection(&cfg.sync)
        .await
        .map_err(ApiError::Internal)?;
    Ok(Json(msg))
}
