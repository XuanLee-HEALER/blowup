// crates/tauri/src/commands/library/mod.rs

pub mod entries;
pub mod graph;
pub mod items;

use serde::Serialize;

// ── Knowledge Base: Entries ──────────────────────────────────────

#[derive(Serialize)]
pub struct EntrySummary {
    pub id: i64,
    pub name: String,
    pub tags: Vec<String>,
    pub updated_at: String,
}

#[derive(Serialize)]
pub struct EntryDetail {
    pub id: i64,
    pub name: String,
    pub wiki: String,
    pub tags: Vec<String>,
    pub relations: Vec<RelationEntry>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Serialize, sqlx::FromRow)]
pub struct RelationEntry {
    pub id: i64,
    pub target_id: i64,
    pub target_name: String,
    pub direction: String,
    pub relation_type: String,
}

// ── Knowledge Base: Graph ───────────────────────────────────────

#[derive(Serialize)]
pub struct GraphData {
    pub nodes: Vec<GraphNode>,
    pub links: Vec<GraphLink>,
}

#[derive(Serialize)]
pub struct GraphNode {
    pub id: String,
    pub label: String,
    pub weight: f64,
}

#[derive(Serialize)]
pub struct GraphLink {
    pub source: String,
    pub target: String,
    pub relation_type: String,
}

// ── Library Items ───────────────────────────────────────────────

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

// ── Internal helper for entry tag aggregation ───────────────────

#[derive(sqlx::FromRow)]
pub(crate) struct EntryRow {
    pub id: i64,
    pub name: String,
    pub wiki: String,
    pub tags_csv: String,
    pub created_at: String,
    pub updated_at: String,
}

impl EntryRow {
    pub fn tags(&self) -> Vec<String> {
        if self.tags_csv.is_empty() {
            Vec::new()
        } else {
            self.tags_csv.split(',').map(|s| s.to_string()).collect()
        }
    }
}
