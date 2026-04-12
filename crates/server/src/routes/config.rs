use axum::{Json, Router, routing::get};
use blowup_core::config::{self, Config};

use crate::error::ApiResult;
use crate::state::AppState;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/config", get(get_config).post(save_config))
        .route("/config/cache-path", get(get_cache_path))
}

async fn get_config() -> ApiResult<Json<Config>> {
    Ok(Json(config::load_config()))
}

async fn save_config(Json(new_config): Json<Config>) -> ApiResult<()> {
    config::save_config(&new_config)?;
    Ok(())
}

async fn get_cache_path() -> Json<String> {
    Json(
        config::app_data_dir()
            .join("credits_cache.json")
            .to_string_lossy()
            .into_owned(),
    )
}
