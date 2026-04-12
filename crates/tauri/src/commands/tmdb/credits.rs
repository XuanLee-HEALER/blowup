use blowup_core::tmdb::model::{MovieCreditsEnriched, TmdbMovieCredits};
use blowup_core::tmdb::service;

#[tauri::command]
pub async fn get_tmdb_movie_credits(
    api_key: String,
    tmdb_id: i64,
) -> Result<TmdbMovieCredits, String> {
    let client = reqwest::Client::new();
    service::get_tmdb_movie_credits(&client, &api_key, tmdb_id).await
}

#[tauri::command]
pub async fn enrich_movie_credits(
    api_key: String,
    ids: Vec<u64>,
) -> Result<Vec<MovieCreditsEnriched>, String> {
    let client = reqwest::Client::new();
    Ok(service::enrich_movie_credits(&client, &api_key, ids).await)
}
