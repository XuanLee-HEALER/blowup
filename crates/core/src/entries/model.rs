//! Public DTOs for the knowledge-base entries domain + its graph view.

use serde::Serialize;

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

// ── Internal row type for tag aggregation ────────────────────────

#[derive(sqlx::FromRow)]
pub struct EntryRow {
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

// ── Graph view (derived from entries + relations) ────────────────

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
