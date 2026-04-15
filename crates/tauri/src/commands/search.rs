use blowup_core::AppContext;
use blowup_core::torrent::search::{
    search_movie,
    types::{ScoredTorrent, SearchQuery},
};
use std::sync::Arc;
use tauri::State;

#[tauri::command]
pub async fn search_yify_cmd(
    query: String,
    year: Option<u32>,
    tmdb_id: Option<u64>,
    ctx: State<'_, Arc<AppContext>>,
) -> Result<Vec<ScoredTorrent>, String> {
    let cfg = blowup_core::config::load_config();
    let q = SearchQuery {
        title: query,
        year,
        tmdb_id,
        tmdb_api_key: cfg.tmdb.api_key,
    };
    Ok(search_movie(&ctx.http, &ctx.tracker, q).await)
}
