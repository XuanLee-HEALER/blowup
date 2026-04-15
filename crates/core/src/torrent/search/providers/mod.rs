//! Concrete `SearchProvider` implementations.

pub mod nyaa;
pub mod onethreeseven;
pub mod yts;

use crate::torrent::search::provider::SearchProvider;
use std::sync::Arc;

/// Build the default provider set. Called once per search — providers
/// are stateless so construction is cheap.
pub fn build_default_providers(tmdb_api_key: String) -> Vec<Arc<dyn SearchProvider>> {
    vec![
        Arc::new(yts::YtsProvider::new(tmdb_api_key)),
        Arc::new(nyaa::NyaaProvider::new()),
        Arc::new(onethreeseven::OnethreesevenProvider::new()),
    ]
}
