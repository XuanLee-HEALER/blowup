// crates/tauri/src/commands/library/mod.rs

pub mod entries;
pub mod graph;
pub mod items;

use serde::Serialize;

// ── Library Items (types still local for now; will move in batch J) ─────

#[derive(Serialize, sqlx::FromRow)]
pub struct LibraryItemSummary {
    pub id: i64,
    pub file_path: String,
    pub file_size: Option<i64>,
    pub duration_secs: Option<i64>,
    pub video_codec: Option<String>,
    pub audio_codec: Option<String>,
    pub resolution: Option<String>,
    pub added_at: String,
}

#[derive(Serialize)]
pub struct LibraryItemDetail {
    pub id: i64,
    pub file_path: String,
    pub file_size: Option<i64>,
    pub duration_secs: Option<i64>,
    pub video_codec: Option<String>,
    pub audio_codec: Option<String>,
    pub resolution: Option<String>,
    pub added_at: String,
    pub assets: Vec<LibraryAssetEntry>,
}

#[derive(Serialize, sqlx::FromRow)]
pub struct LibraryAssetEntry {
    pub id: i64,
    pub asset_type: String,
    pub file_path: String,
    pub lang: Option<String>,
    pub created_at: String,
}

#[derive(Serialize)]
pub struct LibraryStats {
    pub total_items: i64,
    pub total_file_size: i64,
    pub by_resolution: Vec<StatEntry>,
}

#[derive(Serialize, sqlx::FromRow)]
pub struct StatEntry {
    pub label: String,
    pub count: i64,
}

#[derive(Serialize)]
pub struct ScanResult {
    pub added: i64,
    pub skipped: i64,
    pub errors: Vec<String>,
}
