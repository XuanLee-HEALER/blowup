use axum::{Json, Router, routing::post};
use blowup_core::torrent::search::{
    search_movie,
    types::{ScoredTorrent, SearchQuery},
};
use serde::Deserialize;

use crate::error::ApiResult;
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
) -> ApiResult<Json<Vec<ScoredTorrent>>> {
    let cfg = blowup_core::config::load_config();
    let q = SearchQuery {
        title: req.query,
        year: req.year,
        tmdb_id: req.tmdb_id,
        tmdb_api_key: cfg.tmdb.api_key,
    };
    Ok(Json(search_movie(&state.http, &state.tracker, q).await))
}
