// src-tauri/src/commands/library/mod.rs

pub mod films;
pub mod genres;
pub mod graph;
pub mod people;
pub mod reviews;

use serde::{Deserialize, Serialize};

// ── Person ────────────────────────────────────────────────────────

#[derive(Serialize, sqlx::FromRow)]
pub struct PersonSummary {
    pub id: i64,
    pub name: String,
    pub primary_role: String,
    pub film_count: i64,
}

#[derive(Serialize)]
pub struct PersonDetail {
    pub id: i64,
    pub tmdb_id: Option<i64>,
    pub name: String,
    pub primary_role: String,
    pub born_date: Option<String>,
    pub nationality: Option<String>,
    pub biography: Option<String>,
    pub wiki_content: String,
    pub films: Vec<PersonFilmEntry>,
    pub relations: Vec<PersonRelation>,
}

#[derive(Serialize, sqlx::FromRow)]
pub struct PersonFilmEntry {
    pub film_id: i64,
    pub title: String,
    pub year: Option<i64>,
    pub role: String,
    pub poster_cache_path: Option<String>,
}

#[derive(Serialize, sqlx::FromRow)]
pub struct PersonRelation {
    pub target_id: i64,
    pub target_name: String,
    pub direction: String,
    pub relation_type: String,
}

// ── Film ─────────────────────────────────────────────────────────

#[derive(Serialize, sqlx::FromRow)]
pub struct FilmSummary {
    pub id: i64,
    pub title: String,
    pub year: Option<i64>,
    pub tmdb_rating: Option<f64>,
    pub poster_cache_path: Option<String>,
}

#[derive(Serialize)]
pub struct FilmDetail {
    pub id: i64,
    pub tmdb_id: Option<i64>,
    pub title: String,
    pub original_title: Option<String>,
    pub year: Option<i64>,
    pub overview: Option<String>,
    pub tmdb_rating: Option<f64>,
    pub poster_cache_path: Option<String>,
    pub wiki_content: String,
    pub people: Vec<FilmPersonEntry>,
    pub genres: Vec<GenreSummary>,
    pub reviews: Vec<ReviewEntry>,
}

#[derive(Serialize, sqlx::FromRow)]
pub struct FilmPersonEntry {
    pub person_id: i64,
    pub name: String,
    pub role: String,
}

#[derive(Serialize, sqlx::FromRow)]
pub struct ReviewEntry {
    pub id: i64,
    pub is_personal: bool,
    pub author: Option<String>,
    pub content: String,
    pub rating: Option<f64>,
    pub created_at: String,
}

// ── Genre ────────────────────────────────────────────────────────

#[derive(Serialize, sqlx::FromRow)]
pub struct GenreSummary {
    pub id: i64,
    pub name: String,
    pub film_count: i64,
    pub child_count: i64,
}

#[derive(Serialize)]
pub struct GenreDetail {
    pub id: i64,
    pub name: String,
    pub description: Option<String>,
    pub parent_id: Option<i64>,
    pub period: Option<String>,
    pub wiki_content: String,
    pub children: Vec<GenreSummary>,
    pub people: Vec<PersonSummary>,
    pub films: Vec<FilmSummary>,
}

#[derive(Serialize)]
pub struct GenreTreeNode {
    pub id: i64,
    pub name: String,
    pub period: Option<String>,
    pub film_count: i64,
    pub children: Vec<GenreTreeNode>,
}

// ── TMDB input ───────────────────────────────────────────────────

#[derive(Deserialize)]
pub struct TmdbMovieInput {
    pub tmdb_id: i64,
    pub title: String,
    pub original_title: Option<String>,
    pub year: Option<i64>,
    pub overview: Option<String>,
    pub tmdb_rating: Option<f64>,
    pub people: Vec<TmdbPersonInput>,
}

#[derive(Deserialize)]
pub struct TmdbPersonInput {
    pub tmdb_id: Option<i64>,
    pub name: String,
    pub role: String,
    pub primary_role: String,
}

// ── Graph ────────────────────────────────────────────────────────

#[derive(Serialize)]
pub struct GraphData {
    pub nodes: Vec<GraphNode>,
    pub links: Vec<GraphLink>,
}

#[derive(Serialize)]
pub struct GraphNode {
    pub id: String,
    pub label: String,
    pub node_type: String,
    pub role: Option<String>,
    pub weight: f64,
}

#[derive(Serialize)]
pub struct GraphLink {
    pub source: String,
    pub target: String,
    pub role: String,
}
