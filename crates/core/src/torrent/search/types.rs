//! Type vocabulary for the multi-source torrent search pipeline.
//!
//! No logic lives here — just the shared data shapes passed between
//! the orchestrator, providers, parser, scorer, and dedup layers.

use serde::{Deserialize, Serialize};
use thiserror::Error;

// ── Query & context ────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct SearchQuery {
    pub title: String,
    pub year: Option<u32>,
    pub tmdb_id: Option<u64>,
    pub tmdb_api_key: String,
}

/// Passed to every `SearchProvider::search` call. Borrows everything.
#[derive(Debug, Clone, Copy)]
pub struct SearchContext<'a> {
    pub http: &'a reqwest::Client,
    pub title: &'a str,
    pub year: Option<u32>,
    /// Pre-resolved IMDB id (e.g., "tt0061791"). Orchestrator fetches
    /// this once from TMDB and hands it to every provider.
    pub imdb_id: Option<&'a str>,
    pub tmdb_api_key: &'a str,
    /// Snapshot of `TrackerManager::hot_trackers()` taken once per
    /// search. Nyaa uses this to synthesize magnet URIs.
    pub trackers: &'a [String],
}

// ── Raw provider output ────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct RawTorrent {
    pub source: &'static str,
    pub raw_title: String,
    /// Lowercase hex, 40 chars. None if the provider couldn't supply one.
    pub info_hash: Option<String>,
    pub magnet: Option<String>,
    pub torrent_url: Option<String>,
    pub size_bytes: Option<u64>,
    pub seeders: u32,
    pub leechers: u32,
}

// ── Parsed title fields ────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Resolution {
    Unknown,
    Sd,
    P480,
    P720,
    P1080,
    P2160,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SourceKind {
    Unknown,
    Cam,
    Ts,
    Hdtv,
    WebRip,
    WebDl,
    Bluray,
    Remux,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Codec {
    Unknown,
    X264,
    X265,
    Av1,
}

#[derive(Debug, Clone)]
pub struct ParsedTitle {
    pub resolution: Resolution,
    pub source_kind: SourceKind,
    pub codec: Codec,
    pub release_group: Option<String>,
    pub hdr: bool,
}

// ── Scoring ────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct ScoreBreakdown {
    pub seeders: i32,
    pub resolution: i32,
    pub source: i32,
    pub codec: i32,
    pub size: i32,
    pub group: i32,
    pub hdr: i32,
}

impl ScoreBreakdown {
    pub fn total(&self) -> i32 {
        self.seeders
            + self.resolution
            + self.source
            + self.codec
            + self.size
            + self.group
            + self.hdr
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct ScoredTorrent {
    pub source: &'static str,
    pub raw_title: String,
    pub info_hash: Option<String>,
    pub magnet: Option<String>,
    pub torrent_url: Option<String>,
    pub size_bytes: Option<u64>,
    pub seeders: u32,
    pub leechers: u32,
    pub resolution: Resolution,
    pub source_kind: SourceKind,
    pub codec: Codec,
    pub release_group: Option<String>,
    pub hdr: bool,
    pub score: i32,
    pub breakdown: ScoreBreakdown,
}

// ── Provider error ─────────────────────────────────────────────────

#[derive(Debug, Error)]
pub enum ProviderError {
    #[error("network timeout")]
    Timeout,
    #[error("connect failed: {0}")]
    Connect(String),
    #[error("http 5xx: {0}")]
    Http5xx(u16),
    #[error("http 429 rate limited")]
    Http429,
    #[error("http 4xx: {0}")]
    Http4xx(u16),
    #[error("parse error: {0}")]
    Parse(String),
}

impl ProviderError {
    pub fn is_retryable(&self) -> bool {
        matches!(
            self,
            Self::Timeout | Self::Connect(_) | Self::Http5xx(_) | Self::Http429
        )
    }
}

/// Classify a `reqwest::Error` into the matching `ProviderError`.
/// Kept here so every provider uses the same rules.
impl From<reqwest::Error> for ProviderError {
    fn from(e: reqwest::Error) -> Self {
        if e.is_timeout() {
            Self::Timeout
        } else if e.is_connect() {
            Self::Connect(e.to_string())
        } else if let Some(status) = e.status() {
            let code = status.as_u16();
            match code {
                429 => Self::Http429,
                500..=599 => Self::Http5xx(code),
                400..=499 => Self::Http4xx(code),
                _ => Self::Connect(e.to_string()),
            }
        } else {
            Self::Connect(e.to_string())
        }
    }
}
