use axum::extract::State;
use axum::{Json, Router, routing::get, routing::post};
use blowup_core::torrent::tracker::TrackerStatus;
use serde::Deserialize;

use crate::error::ApiResult;
use crate::state::AppState;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/tracker/status", get(get_status))
        .route("/tracker/refresh", post(refresh))
        .route("/tracker/user", post(add_user))
}

async fn get_status(State(state): State<AppState>) -> Json<TrackerStatus> {
    Json(state.tracker.get_status().await)
}

async fn refresh(State(state): State<AppState>) -> ApiResult<Json<TrackerStatus>> {
    let status = state
        .tracker
        .refresh_auto()
        .await
        .map_err(crate::error::ApiError::Internal)?;
    Ok(Json(status))
}

#[derive(Deserialize)]
pub struct UserTrackersRequest {
    pub raw: String,
}

async fn add_user(
    State(state): State<AppState>,
    Json(req): Json<UserTrackersRequest>,
) -> ApiResult<Json<TrackerStatus>> {
    let status = state
        .tracker
        .add_user_trackers(req.raw)
        .await
        .map_err(crate::error::ApiError::from)?;
    Ok(Json(status))
}
