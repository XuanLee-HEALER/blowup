use blowup_core::tmdb::model::{MovieListItem, SearchFilters, TmdbGenre};
use blowup_core::tmdb::service;

#[tauri::command]
pub async fn search_movies(
    api_key: String,
    query: String,
    filters: SearchFilters,
) -> Result<Vec<MovieListItem>, String> {
    let client = reqwest::Client::new();
    service::search_movies(&client, &api_key, &query, &filters).await
}

#[tauri::command]
pub async fn discover_movies(
    api_key: String,
    filters: SearchFilters,
) -> Result<Vec<MovieListItem>, String> {
    let client = reqwest::Client::new();
    service::discover_movies(&client, &api_key, &filters).await
}

#[tauri::command]
pub async fn list_genres(api_key: String) -> Result<Vec<TmdbGenre>, String> {
    let client = reqwest::Client::new();
    service::list_genres(&client, &api_key).await
}
