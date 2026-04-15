//! Concrete `SearchProvider` implementations.

pub mod yts;

use crate::torrent::search::provider::SearchProvider;
use std::sync::Arc;

/// Build the default provider set. Called once per search — providers
/// are stateless so construction is cheap.
pub fn build_default_providers(tmdb_api_key: String) -> Vec<Arc<dyn SearchProvider>> {
    vec![
        Arc::new(yts::YtsProvider::new(tmdb_api_key)),
        // nyaa and 1337x added in later tasks
    ]
}
