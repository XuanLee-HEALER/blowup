use blowup_core::torrent::search::{self, MovieResult};

#[tauri::command]
pub async fn search_yify_cmd(
    query: String,
    year: Option<u32>,
    tmdb_id: Option<u64>,
) -> Result<Vec<MovieResult>, String> {
    let client = reqwest::Client::new();
    let cfg = blowup_core::config::load_config();
    search::search_yify(&client, &cfg.tmdb.api_key, &query, year, tmdb_id)
        .await
        .map_err(|e| e.to_string())
}
