//! Concrete `SearchProvider` implementations.

pub mod nyaa;
pub mod onethreeseven;
pub mod yts;

use crate::torrent::search::provider::SearchProvider;
use regex::Regex;
use std::sync::{Arc, LazyLock};

/// Build the default provider set. Called once per search — providers
/// are stateless so construction is cheap.
pub fn build_default_providers(tmdb_api_key: String) -> Vec<Arc<dyn SearchProvider>> {
    vec![
        Arc::new(yts::YtsProvider::new(tmdb_api_key)),
        Arc::new(nyaa::NyaaProvider::new()),
        Arc::new(onethreeseven::OnethreesevenProvider::new()),
    ]
}

// ── Shared magnet utilities ────────────────────────────────────────

static MAGNET_HASH_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)urn:btih:([a-f0-9]+)").unwrap());

pub(crate) fn extract_info_hash_from_magnet(magnet: &str) -> Option<String> {
    MAGNET_HASH_RE.captures(magnet).map(|c| c[1].to_lowercase())
}
