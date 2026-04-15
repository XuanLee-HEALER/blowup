//! Multi-source torrent search orchestrator.
//!
//! See docs/superpowers/specs/2026-04-15-multi-source-torrent-search-design.md

pub mod dedup;
pub mod parser;
pub mod provider;
pub mod providers;
pub mod scorer;
pub mod types;

use crate::torrent::tracker::TrackerManager;
use serde::Deserialize;
use std::sync::Arc;
use std::time::Instant;
use types::{RawTorrent, ScoredTorrent, SearchContext, SearchQuery};

/// Main entry point. Runs all enabled providers concurrently, merges
/// their results by info_hash, parses + scores each, and returns a
/// list sorted by total score (descending).
///
/// Individual provider failures are logged via `tracing::warn!` and
/// never bubble up; the caller sees only successful results. All
/// providers failing returns an empty Vec.
pub async fn search_movie(
    http: &reqwest::Client,
    tracker: &Arc<TrackerManager>,
    query: SearchQuery,
) -> Vec<ScoredTorrent> {
    tracing::info!(
        title = %query.title,
        year = ?query.year,
        tmdb_id = ?query.tmdb_id,
        "search_movie started"
    );
    let total_t = Instant::now();

    // Resolve IMDB id up-front (optional).
    let imdb_id = if let Some(tmdb_id) = query.tmdb_id {
        fetch_imdb_id(http, &query.tmdb_api_key, tmdb_id).await
    } else {
        None
    };
    tracing::debug!(imdb_id = ?imdb_id, "imdb id resolved");

    // Snapshot tracker list.
    let trackers = tracker.hot_trackers().await;

    let ctx = SearchContext {
        http,
        title: &query.title,
        year: query.year,
        imdb_id: imdb_id.as_deref(),
        tmdb_api_key: &query.tmdb_api_key,
        trackers: &trackers,
    };

    let providers_list = providers::build_default_providers(query.tmdb_api_key.clone());

    // Fan out. Each future resolves to (name, Result<Vec<RawTorrent>>).
    let futs: Vec<_> = providers_list
        .iter()
        .map(|p| {
            let p = p.clone();
            async move {
                let t = Instant::now();
                let res = p.search(&ctx).await;
                tracing::debug!(
                    provider = p.name(),
                    elapsed_ms = t.elapsed().as_millis() as u64,
                    ok = res.is_ok(),
                    "provider finished"
                );
                (p.name(), res)
            }
        })
        .collect();
    let results = futures::future::join_all(futs).await;

    // Collect successes; log failures.
    let mut all_raw: Vec<RawTorrent> = Vec::new();
    for (name, res) in results {
        match res {
            Ok(items) => {
                tracing::debug!(provider = name, count = items.len(), "provider returned");
                all_raw.extend(items);
            }
            Err(e) => {
                tracing::warn!(provider = name, error = %e, "provider failed");
            }
        }
    }

    // Drop entries with no downloadable entrypoint.
    let usable: Vec<RawTorrent> = all_raw
        .into_iter()
        .filter(|r| r.magnet.is_some() || r.torrent_url.is_some())
        .collect();

    // Dedup, parse, score, sort.
    let deduped = dedup::merge(usable);
    let mut scored: Vec<ScoredTorrent> = deduped.into_iter().map(assemble_scored).collect();
    scored.sort_by(|a, b| b.score.cmp(&a.score));

    tracing::info!(
        count = scored.len(),
        total_ms = total_t.elapsed().as_millis() as u64,
        "search_movie complete"
    );
    scored
}

fn assemble_scored(raw: RawTorrent) -> ScoredTorrent {
    let parsed = parser::parse_release_title(&raw.raw_title);
    let breakdown = scorer::score(&raw, &parsed);
    let total = breakdown.total();
    ScoredTorrent {
        source: raw.source,
        raw_title: raw.raw_title,
        info_hash: raw.info_hash,
        magnet: raw.magnet,
        torrent_url: raw.torrent_url,
        size_bytes: raw.size_bytes,
        seeders: raw.seeders,
        leechers: raw.leechers,
        resolution: parsed.resolution,
        source_kind: parsed.source_kind,
        codec: parsed.codec,
        release_group: parsed.release_group,
        hdr: parsed.hdr,
        score: total,
        breakdown,
    }
}

/// Fetch IMDB ID for a TMDB movie via the TMDB external_ids endpoint.
/// Best-effort — returns None on any failure.
async fn fetch_imdb_id(client: &reqwest::Client, api_key: &str, tmdb_id: u64) -> Option<String> {
    if api_key.is_empty() {
        return None;
    }

    #[derive(Deserialize)]
    struct ExternalIds {
        imdb_id: Option<String>,
    }

    let resp = client
        .get(format!(
            "https://api.themoviedb.org/3/movie/{tmdb_id}/external_ids"
        ))
        .query(&[("api_key", api_key)])
        .header("User-Agent", "blowup/0.1")
        .send()
        .await
        .ok()?;

    let ids: ExternalIds = resp.json().await.ok()?;
    ids.imdb_id.filter(|s| !s.is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn raw(source: &'static str, title: &str, hash: &str, seeders: u32) -> RawTorrent {
        RawTorrent {
            source,
            raw_title: title.to_string(),
            info_hash: Some(hash.to_string()),
            magnet: Some(format!("magnet:?xt=urn:btih:{hash}")),
            torrent_url: None,
            size_bytes: Some(5_000_000_000),
            seeders,
            leechers: 0,
        }
    }

    /// Replicate the post-fetch pipeline (dedup + parse + score + sort)
    /// so we can test it without reaching for network. Intentionally
    /// duplicates the orchestrator steps verbatim — DRY would make the
    /// test meaningless.
    fn pipeline(all_raw: Vec<RawTorrent>) -> Vec<ScoredTorrent> {
        let usable: Vec<_> = all_raw
            .into_iter()
            .filter(|r| r.magnet.is_some() || r.torrent_url.is_some())
            .collect();
        let deduped = dedup::merge(usable);
        let mut scored: Vec<_> = deduped.into_iter().map(assemble_scored).collect();
        scored.sort_by(|a, b| b.score.cmp(&a.score));
        scored
    }

    #[test]
    fn pipeline_dedups_same_hash_across_sources() {
        let a = raw("yts", "Blow.Up.1966.1080p.BluRay.x265-FraMeSToR", "abc", 50);
        let b = raw(
            "nyaa",
            "Blow.Up.1966.1080p.BluRay.x265-FraMeSToR",
            "abc",
            80,
        );
        let out = pipeline(vec![a, b]);
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].seeders, 80);
    }

    #[test]
    fn pipeline_sorts_by_total_score_desc() {
        let high = raw(
            "yts",
            "Hero.2002.2160p.BluRay.REMUX.x265.HDR-GECKOS",
            "aaa",
            200,
        );
        let low = raw("nyaa", "Hero.2002.HDTV.XviD-X", "bbb", 3);
        let out = pipeline(vec![low, high]);
        assert_eq!(out[0].info_hash.as_deref(), Some("aaa"));
        assert_eq!(out[1].info_hash.as_deref(), Some("bbb"));
    }

    #[test]
    fn pipeline_drops_unusable_entries() {
        let usable = raw("yts", "Blow.Up.1966.1080p.BluRay.x265-FraMeSToR", "abc", 50);
        let unusable = RawTorrent {
            magnet: None,
            torrent_url: None,
            ..raw("nyaa", "Useless", "def", 50)
        };
        let out = pipeline(vec![usable, unusable]);
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].info_hash.as_deref(), Some("abc"));
    }
}
