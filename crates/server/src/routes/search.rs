use axum::{Json, Router, routing::post};
use blowup_core::torrent::search::{self, MovieResult};
use serde::Deserialize;

use crate::error::{ApiError, ApiResult};
use crate::state::AppState;

pub fn router() -> Router<AppState> {
    Router::new().route("/search/yify", post(search_yify))
}

#[derive(Deserialize)]
pub struct YifySearchRequest {
    pub query: String,
    pub year: Option<u32>,
    pub tmdb_id: Option<u64>,
}

async fn search_yify(
    axum::extract::State(state): axum::extract::State<AppState>,
    Json(req): Json<YifySearchRequest>,
) -> ApiResult<Json<Vec<MovieResult>>> {
    let cfg = blowup_core::config::load_config();
    let results = search::search_yify(
        &state.http,
        &cfg.tmdb.api_key,
        &req.query,
        req.year,
        req.tmdb_id,
    )
    .await
    .map_err(|e| ApiError::Internal(e.to_string()))?;
    Ok(Json(results))
}
