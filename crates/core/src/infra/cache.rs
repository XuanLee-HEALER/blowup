use crate::config::{app_data_dir, load_config};
use parking_lot::Mutex;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

static CACHE: Mutex<Option<CreditsCache>> = parking_lot::const_mutex(None);

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreditsCacheEntry {
    pub id: u64,
    pub director: Option<String>,
    pub cast: Vec<String>,
    pub ts: i64,
}

#[derive(Debug, Serialize, Deserialize)]
struct CreditsCache {
    max_entries: usize,
    entries: Vec<CreditsCacheEntry>,
}

impl Default for CreditsCache {
    fn default() -> Self {
        Self {
            max_entries: 200,
            entries: Vec::new(),
        }
    }
}

fn cache_path() -> PathBuf {
    app_data_dir().join("credits_cache.json")
}

/// Load cache from disk into the global singleton.
/// Creates the file if it doesn't exist; rebuilds if corrupted.
pub fn init_cache() {
    let cache = load_from_disk();
    save_to_disk(&cache);
    let mut guard = CACHE.lock();
    *guard = Some(cache);
    tracing::info!("credits cache initialized");
}

/// Flush the in-memory cache to disk. Call before app exit.
pub fn flush_cache() {
    let guard = CACHE.lock();
    if let Some(cache) = guard.as_ref() {
        save_to_disk(cache);
        tracing::info!(
            entries = cache.entries.len(),
            "credits cache flushed to disk"
        );
    }
}

fn load_from_disk() -> CreditsCache {
    let path = cache_path();
    let max = load_config().cache.max_entries;

    let empty = || CreditsCache {
        max_entries: max,
        ..CreditsCache::default()
    };

    if !path.exists() {
        tracing::debug!("cache file not found, creating new");
        return empty();
    }

    let content = match std::fs::read_to_string(&path) {
        Ok(c) => c,
        Err(e) => {
            tracing::warn!(error = %e, "failed to read cache file, rebuilding");
            std::fs::remove_file(&path).ok();
            return empty();
        }
    };

    match serde_json::from_str::<CreditsCache>(&content) {
        Ok(mut cache) => {
            cache.max_entries = max;
            // Trim if config reduced max_entries
            while cache.entries.len() > cache.max_entries {
                cache.entries.remove(0);
            }
            tracing::debug!(entries = cache.entries.len(), "cache loaded from disk");
            cache
        }
        Err(e) => {
            tracing::warn!(error = %e, "cache file corrupted, rebuilding");
            std::fs::remove_file(&path).ok();
            empty()
        }
    }
}

fn save_to_disk(cache: &CreditsCache) {
    let path = cache_path();
    if let Some(parent) = path.parent()
        && let Err(e) = std::fs::create_dir_all(parent)
    {
        tracing::error!(error = %e, "failed to create cache directory");
        return;
    }
    match serde_json::to_string(cache) {
        Ok(content) => {
            if let Err(e) = std::fs::write(&path, content) {
                tracing::error!(error = %e, "failed to write cache file");
            }
        }
        Err(e) => tracing::error!(error = %e, "failed to serialize cache"),
    }
}

/// Look up a cached entry by TMDB ID. Moves the entry to the end (LRU touch).
pub fn credits_get(id: u64) -> Option<CreditsCacheEntry> {
    let mut guard = CACHE.lock();
    let cache = guard.as_mut()?;
    let pos = cache.entries.iter().position(|e| e.id == id)?;
    let entry = cache.entries.remove(pos);
    cache.entries.push(entry.clone());
    tracing::debug!(id, "credits cache hit");
    Some(entry)
}

/// Insert or update a cache entry. Evicts oldest if over limit. Writes to disk.
pub fn credits_put(id: u64, director: Option<String>, cast: Vec<String>) {
    let mut guard = CACHE.lock();
    let Some(cache) = guard.as_mut() else {
        tracing::warn!(id, "credits_put called before init_cache — dropping write");
        return;
    };

    // Remove existing entry for this id
    cache.entries.retain(|e| e.id != id);

    let entry = CreditsCacheEntry {
        id,
        director,
        cast,
        ts: chrono::Utc::now().timestamp(),
    };
    cache.entries.push(entry);

    // Evict oldest if over limit
    while cache.entries.len() > cache.max_entries {
        cache.entries.remove(0);
    }

    tracing::debug!(id, total = cache.entries.len(), "credits cache put");
    save_to_disk(cache);
}

#[cfg(test)]
mod tests {
    use super::*;
    use serial_test::serial;

    fn setup_test_cache() {
        let mut guard = CACHE.lock();
        *guard = Some(CreditsCache {
            max_entries: 3,
            entries: Vec::new(),
        });
    }

    #[test]
    #[serial]
    fn put_and_get() {
        setup_test_cache();
        credits_put(1, Some("Director A".into()), vec!["Actor 1".into()]);
        let entry = credits_get(1).unwrap();
        assert_eq!(entry.director.as_deref(), Some("Director A"));
        assert_eq!(entry.cast, vec!["Actor 1"]);
    }

    #[test]
    #[serial]
    fn lru_eviction() {
        setup_test_cache();
        credits_put(1, None, vec![]);
        credits_put(2, None, vec![]);
        credits_put(3, None, vec![]);
        // Cache is full (max 3), adding one more should evict id=1
        credits_put(4, None, vec![]);
        assert!(credits_get(1).is_none());
        assert!(credits_get(4).is_some());
    }

    #[test]
    #[serial]
    fn lru_touch_on_get() {
        setup_test_cache();
        credits_put(1, None, vec![]);
        credits_put(2, None, vec![]);
        credits_put(3, None, vec![]);
        // Touch id=1, making id=2 the oldest
        credits_get(1);
        credits_put(4, None, vec![]);
        // id=2 should be evicted, id=1 should survive
        assert!(credits_get(2).is_none());
        assert!(credits_get(1).is_some());
    }

    #[test]
    #[serial]
    fn update_existing_entry() {
        setup_test_cache();
        credits_put(1, Some("Old".into()), vec![]);
        credits_put(1, Some("New".into()), vec!["Actor".into()]);
        let entry = credits_get(1).unwrap();
        assert_eq!(entry.director.as_deref(), Some("New"));
        assert_eq!(entry.cast, vec!["Actor"]);
        // Should not have duplicates
        let guard = CACHE.lock();
        let cache = guard.as_ref().unwrap();
        assert_eq!(cache.entries.iter().filter(|e| e.id == 1).count(), 1);
    }
}
