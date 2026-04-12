use axum::extract::{Path, Query, State};
use axum::{Json, Router, routing::get, routing::post};
use blowup_core::tmdb::model::{
    MovieCreditsEnriched, MovieListItem, SearchFilters, TmdbGenre, TmdbMovieCredits,
};
use blowup_core::tmdb::service;
use serde::Deserialize;

use crate::error::ApiResult;
use crate::state::AppState;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/tmdb/search", post(search_movies))
        .route("/tmdb/discover", post(discover_movies))
        .route("/tmdb/genres", get(list_genres))
        .route("/tmdb/credits/{id}", get(get_credits))
        .route("/tmdb/credits/enrich", post(enrich_credits))
}

#[derive(Deserialize)]
pub struct SearchRequest {
    pub api_key: String,
    pub query: String,
    pub filters: SearchFilters,
}

async fn search_movies(
    State(state): State<AppState>,
    Json(req): Json<SearchRequest>,
) -> ApiResult<Json<Vec<MovieListItem>>> {
    let results = service::search_movies(&state.http, &req.api_key, &req.query, &req.filters)
        .await
        .map_err(crate::error::ApiError::Internal)?;
    Ok(Json(results))
}

#[derive(Deserialize)]
pub struct DiscoverRequest {
    pub api_key: String,
    pub filters: SearchFilters,
}

async fn discover_movies(
    State(state): State<AppState>,
    Json(req): Json<DiscoverRequest>,
) -> ApiResult<Json<Vec<MovieListItem>>> {
    let results = service::discover_movies(&state.http, &req.api_key, &req.filters)
        .await
        .map_err(crate::error::ApiError::Internal)?;
    Ok(Json(results))
}

#[derive(Deserialize)]
pub struct ApiKeyQuery {
    pub api_key: String,
}

async fn list_genres(
    State(state): State<AppState>,
    Query(q): Query<ApiKeyQuery>,
) -> ApiResult<Json<Vec<TmdbGenre>>> {
    let genres = service::list_genres(&state.http, &q.api_key)
        .await
        .map_err(crate::error::ApiError::Internal)?;
    Ok(Json(genres))
}

async fn get_credits(
    State(state): State<AppState>,
    Path(id): Path<i64>,
    Query(q): Query<ApiKeyQuery>,
) -> ApiResult<Json<TmdbMovieCredits>> {
    let credits = service::get_tmdb_movie_credits(&state.http, &q.api_key, id)
        .await
        .map_err(crate::error::ApiError::Internal)?;
    Ok(Json(credits))
}

#[derive(Deserialize)]
pub struct EnrichRequest {
    pub api_key: String,
    pub ids: Vec<u64>,
}

async fn enrich_credits(
    State(state): State<AppState>,
    Json(req): Json<EnrichRequest>,
) -> ApiResult<Json<Vec<MovieCreditsEnriched>>> {
    let results = service::enrich_movie_credits(&state.http, &req.api_key, req.ids).await;
    Ok(Json(results))
}
