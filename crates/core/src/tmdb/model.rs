//! Public DTOs exposed by `core::tmdb::service`. The wrappers in
//! `blowup-tauri` use these types as command input/output, and the
//! frontend deserializes them across the Tauri bridge.

use serde::{Deserialize, Serialize};

// ── Search / discover ──────────────────────────────────────────

/// Lightweight result row shown in the search list.
#[derive(Debug, Clone, Serialize)]
pub struct MovieListItem {
    pub id: u64,
    pub title: String,
    pub original_title: String,
    pub year: String, // empty string if unknown
    pub overview: String,
    pub vote_average: f64,
    pub poster_path: Option<String>,
    pub genre_ids: Vec<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub director: Option<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub cast: Vec<String>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct SearchFilters {
    pub year_from: Option<u32>,
    pub year_to: Option<u32>,
    pub genre_ids: Vec<u64>,
    pub min_rating: Option<f32>,
    pub sort_by: Option<String>,
    pub page: Option<u32>,
}

#[derive(Debug, Serialize)]
pub struct TmdbGenre {
    pub id: u64,
    pub name: String,
}

// ── Query / print ─────────────────────────────────────────────

pub struct TmdbMovie {
    pub title: String,
    pub year: String,
    pub genres: String,
    pub director: String,
    pub actors: String,
    pub rating: String,
    pub overview: String,
}

// ── Credits ────────────────────────────────────────────────────

#[derive(Serialize)]
pub struct TmdbCrewMember {
    pub id: i64,
    pub name: String,
    pub job: String,
    pub department: String,
}

#[derive(Serialize)]
pub struct TmdbCastMember {
    pub id: i64,
    pub name: String,
    pub character: String,
}

#[derive(Serialize)]
pub struct TmdbMovieCredits {
    pub tmdb_id: i64,
    pub title: String,
    pub original_title: Option<String>,
    pub year: Option<i64>,
    pub overview: Option<String>,
    pub vote_average: Option<f64>,
    pub poster_path: Option<String>,
    pub crew: Vec<TmdbCrewMember>,
    pub cast: Vec<TmdbCastMember>,
}

#[derive(Debug, Serialize)]
pub struct MovieCreditsEnriched {
    pub id: u64,
    pub director: Option<String>,
    pub cast: Vec<String>,
}
