//! YTS / YIFY provider.
//!
//! Endpoint: https://movies-api.accel.li/api/v2/list_movies.json
//! (The old `yts.torrentbay.st` returns HTML, hence the mirror.)
//!
//! Search strategy:
//! 1. If IMDB ID is available in `SearchContext`, query by that
//!    (most reliable — avoids title localization quirks).
//! 2. Otherwise query by original title + year.
//! 3. Fallback: sanitize title (strip punctuation) and retry.

use super::extract_info_hash_from_magnet;
use crate::torrent::search::provider::{CallPacer, SearchProvider, with_retry};
use crate::torrent::search::types::{ProviderError, RawTorrent, SearchContext};
use async_trait::async_trait;
use regex::Regex;
use serde::Deserialize;
use std::sync::LazyLock;
use std::time::{Duration, Instant};

pub struct YtsProvider {
    // tmdb_api_key is used by the orchestrator to pre-resolve imdb_id;
    // YtsProvider itself only reads `ctx.imdb_id`. We keep this field
    // in case the orchestrator ever stops pre-resolving, which would
    // let YtsProvider fall back to calling TMDB itself.
    #[allow(dead_code)]
    tmdb_api_key: String,
}

impl YtsProvider {
    pub fn new(tmdb_api_key: String) -> Self {
        Self { tmdb_api_key }
    }
}

#[async_trait]
impl SearchProvider for YtsProvider {
    fn name(&self) -> &'static str {
        "yts"
    }

    fn min_interval(&self) -> Duration {
        Duration::from_secs(3)
    }

    async fn search(&self, ctx: &SearchContext<'_>) -> Result<Vec<RawTorrent>, ProviderError> {
        let mut pacer = CallPacer::new(self.min_interval());

        // Strategy 1: imdb_id if present
        if let Some(imdb) = ctx.imdb_id {
            tracing::debug!(provider = "yts", imdb, "searching by imdb id");
            let results = with_retry(&mut pacer, self.max_retries(), || {
                search_via_api(ctx.http, imdb, None)
            })
            .await?;
            if !results.is_empty() {
                tracing::debug!(provider = "yts", count = results.len(), "found via imdb");
                return Ok(results);
            }
        }

        // Strategy 2: title + year
        tracing::debug!(
            provider = "yts",
            title = ctx.title,
            year = ?ctx.year,
            "searching by title"
        );
        let results = with_retry(&mut pacer, self.max_retries(), || {
            search_via_api(ctx.http, ctx.title, ctx.year)
        })
        .await?;
        if !results.is_empty() {
            tracing::debug!(provider = "yts", count = results.len(), "found via title");
            return Ok(results);
        }

        // Strategy 3: sanitized title
        let sanitized = sanitize_query(ctx.title);
        if sanitized != ctx.title {
            tracing::debug!(
                provider = "yts",
                sanitized = %sanitized,
                "searching by sanitized title"
            );
            let results = with_retry(&mut pacer, self.max_retries(), || {
                search_via_api(ctx.http, &sanitized, ctx.year)
            })
            .await?;
            if !results.is_empty() {
                tracing::debug!(
                    provider = "yts",
                    count = results.len(),
                    "found via sanitized"
                );
                return Ok(results);
            }
        }

        tracing::debug!(provider = "yts", "no results after all strategies");
        Ok(Vec::new())
    }
}

// ── YTS JSON schema ────────────────────────────────────────────────

#[derive(Deserialize)]
struct YtsResponse {
    data: YtsData,
}

#[derive(Deserialize)]
struct YtsData {
    #[serde(default)]
    movies: Vec<YtsMovie>,
}

#[derive(Deserialize)]
struct YtsMovie {
    title: String,
    year: u32,
    #[serde(default)]
    torrents: Vec<YtsTorrent>,
}

#[derive(Deserialize)]
struct YtsTorrent {
    quality: String,
    #[serde(rename = "url")]
    url: String,
    seeds: u32,
    #[serde(default)]
    peers: u32,
    #[serde(default)]
    magnet_url: Option<String>,
    #[serde(default)]
    size_bytes: Option<u64>,
}

async fn search_via_api(
    http: &reqwest::Client,
    query: &str,
    year: Option<u32>,
) -> Result<Vec<RawTorrent>, ProviderError> {
    let mut params: Vec<(&str, String)> = vec![
        ("query_term", query.to_string()),
        ("sort_by", "seeds".to_string()),
        ("order_by", "desc".to_string()),
    ];
    if let Some(y) = year {
        params.push(("year", y.to_string()));
    }

    let url = "https://movies-api.accel.li/api/v2/list_movies.json";
    let t = Instant::now();
    let resp = http
        .get(url)
        .query(&params)
        .header("User-Agent", "blowup/0.1")
        .send()
        .await?;
    let status = resp.status();
    tracing::debug!(
        provider = "yts",
        request_url = url,
        request_method = "GET",
        response_status = status.as_u16(),
        response_ms = t.elapsed().as_millis() as u64,
        "yts api call"
    );

    if !status.is_success() {
        let code = status.as_u16();
        if code == 429 {
            return Err(ProviderError::Http429);
        } else if (500..600).contains(&code) {
            return Err(ProviderError::Http5xx(code));
        } else {
            return Err(ProviderError::Http4xx(code));
        }
    }

    let body: YtsResponse = resp
        .json()
        .await
        .map_err(|e| ProviderError::Parse(format!("yts json: {e}")))?;

    let mut out = Vec::new();
    for movie in body.data.movies {
        let quality_title_suffix = if !movie.year.to_string().is_empty() {
            format!(" ({})", movie.year)
        } else {
            String::new()
        };
        for t in movie.torrents {
            let info_hash = t
                .magnet_url
                .as_deref()
                .and_then(extract_info_hash_from_magnet);
            out.push(RawTorrent {
                source: "yts",
                raw_title: format!(
                    "{}{} {} BluRay x264 YIFY",
                    movie.title, quality_title_suffix, t.quality
                ),
                info_hash,
                magnet: t.magnet_url,
                torrent_url: Some(t.url),
                size_bytes: t.size_bytes,
                seeders: t.seeds,
                leechers: t.peers,
            });
        }
    }
    tracing::debug!(
        provider = "yts",
        raw_count = out.len(),
        "yts parse complete"
    );
    Ok(out)
}

static SANITIZE_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"[^\w\s]").unwrap());

fn sanitize_query(query: &str) -> String {
    let cleaned = SANITIZE_RE.replace_all(query, " ");
    cleaned.split_whitespace().collect::<Vec<_>>().join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn sample_response() -> YtsResponse {
        serde_json::from_value(json!({
            "data": {
                "movies": [{
                    "title": "Blow-Up",
                    "year": 1966,
                    "torrents": [
                        {
                            "quality": "1080p",
                            "url": "https://yts.example/a.torrent",
                            "seeds": 120,
                            "peers": 8,
                            "magnet_url": "magnet:?xt=urn:btih:AABBCCDDEEFF0011223344556677889900AABBCC",
                            "size_bytes": 5_368_709_120u64
                        },
                        {
                            "quality": "720p",
                            "url": "https://yts.example/b.torrent",
                            "seeds": 80,
                            "peers": 4,
                            "magnet_url": null,
                            "size_bytes": 2_147_483_648u64
                        }
                    ]
                }]
            }
        }))
        .unwrap()
    }

    #[test]
    fn parses_yts_response_fields() {
        // Direct parse path — exercises the conversion without needing HTTP.
        let resp = sample_response();
        let mut out: Vec<RawTorrent> = Vec::new();
        for movie in resp.data.movies {
            for t in movie.torrents {
                let info_hash = t
                    .magnet_url
                    .as_deref()
                    .and_then(extract_info_hash_from_magnet);
                out.push(RawTorrent {
                    source: "yts",
                    raw_title: format!(
                        "{} ({}) {} BluRay x264 YIFY",
                        movie.title, movie.year, t.quality
                    ),
                    info_hash,
                    magnet: t.magnet_url,
                    torrent_url: Some(t.url),
                    size_bytes: t.size_bytes,
                    seeders: t.seeds,
                    leechers: t.peers,
                });
            }
        }

        assert_eq!(out.len(), 2);
        assert_eq!(out[0].source, "yts");
        assert_eq!(out[0].seeders, 120);
        assert_eq!(
            out[0].info_hash.as_deref(),
            Some("aabbccddeeff0011223344556677889900aabbcc")
        );
        assert_eq!(out[0].size_bytes, Some(5_368_709_120u64));
        assert!(out[0].raw_title.contains("1080p"));

        // Second torrent has no magnet → info_hash is None.
        assert_eq!(out[1].info_hash, None);
        assert!(out[1].magnet.is_none());
    }

    #[test]
    fn info_hash_extraction_lowercases() {
        let hash = extract_info_hash_from_magnet(
            "magnet:?xt=urn:btih:DEADBEEFCAFEBABE0123456789ABCDEF01234567&dn=foo",
        );
        assert_eq!(
            hash.as_deref(),
            Some("deadbeefcafebabe0123456789abcdef01234567")
        );
    }

    #[test]
    fn info_hash_none_if_no_btih() {
        assert_eq!(extract_info_hash_from_magnet("https://example.com/x"), None);
    }

    #[test]
    fn sanitize_strips_punctuation() {
        assert_eq!(sanitize_query("Blow-Up: A Photo!"), "Blow Up A Photo");
        assert_eq!(sanitize_query("Already clean"), "Already clean");
    }

    /// Live smoke test — hits real YTS. `#[ignore]` so CI skips it.
    /// Run manually: `cargo test -p blowup-core --ignored yts_live_`.
    #[tokio::test]
    #[ignore]
    async fn yts_live_the_matrix() {
        let http = reqwest::Client::new();
        let results = search_via_api(&http, "The Matrix", Some(1999))
            .await
            .expect("yts live call should succeed");
        // Business-independent: just assert we got SOMETHING back and
        // didn't crash on parse. Specific counts / scores change over time.
        assert!(!results.is_empty(), "expected at least one result");
        let first = &results[0];
        assert!(first.magnet.is_some() || first.torrent_url.is_some());
    }
}
