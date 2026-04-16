//! 1337x HTML-scrape provider.
//!
//! Two-stage:
//! 1. GET https://1337x.to/search/<query>/1/ → parse results table
//!    for title + detail URL + seeders + leechers + size.
//! 2. For the top 5 by seeders, concurrently fetch detail pages and
//!    extract the magnet link (+ info_hash from the magnet).
//!
//! Pacing: CallPacer::wait between Stage 1 and Stage 2. Within Stage 2
//! the 5 detail requests run concurrently (buffer_unordered) — a small
//! burst to one host is acceptable and keeps total latency ~5s.
//!
//! Selector brittleness: HTML can change. On selector miss we return
//! `ProviderError::Parse` with a clear message so the maintainer
//! (us) can locate the problem fast. We do NOT attempt Cloudflare
//! bypass — if 1337x moves behind a challenge, we surface 403/503
//! and rely on the orchestrator to silently drop the source.

use super::extract_info_hash_from_magnet;
use crate::torrent::search::provider::{CallPacer, SearchProvider, with_retry};
use crate::torrent::search::types::{ProviderError, RawTorrent, SearchContext};
use async_trait::async_trait;
use futures::stream::{self, StreamExt};
use scraper::{Html, Selector};
use std::sync::LazyLock;
use std::time::{Duration, Instant};

const DETAIL_CONCURRENCY: usize = 5;
const TOP_N_FOR_DETAIL: usize = 5;
const USER_AGENT: &str = "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 \
     (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36";

// ── Selectors ──────────────────────────────────────────────────────

// Row in the search results table.
const ROW_SEL: &str = "table.table-list tbody tr";
// Name cell contains two <a> — the last one holds the title text + detail URL.
const NAME_ANCHOR_SEL: &str = "td.coll-1.name a:not(.icon)";
const SEEDS_SEL: &str = "td.coll-2.seeds";
const LEECHES_SEL: &str = "td.coll-3.leeches";
const SIZE_SEL: &str = "td.coll-4.size";
// On the detail page, magnet is the <a href="magnet:..."> inside the
// download links list.
const MAGNET_SEL: &str = "ul.download-links-dontblock a[href^='magnet:']";

pub struct OnethreesevenProvider;

impl OnethreesevenProvider {
    pub fn new() -> Self {
        Self
    }
}

impl Default for OnethreesevenProvider {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl SearchProvider for OnethreesevenProvider {
    fn name(&self) -> &'static str {
        "1337x"
    }

    fn min_interval(&self) -> Duration {
        Duration::from_secs(5)
    }

    async fn search(&self, ctx: &SearchContext<'_>) -> Result<Vec<RawTorrent>, ProviderError> {
        let mut pacer = CallPacer::new(self.min_interval());
        let query = ctx.title.to_string();

        // Stage 1: search listing
        let mut stage1 = with_retry(&mut pacer, self.max_retries(), || {
            fetch_search(ctx.http, &query)
        })
        .await?;
        tracing::debug!(
            provider = "1337x",
            stage1_count = stage1.len(),
            "1337x stage1 complete"
        );
        if stage1.is_empty() {
            return Ok(Vec::new());
        }

        // Sort by seeders desc and take top N.
        stage1.sort_by(|a, b| b.seeders.cmp(&a.seeders));
        stage1.truncate(TOP_N_FOR_DETAIL);

        // Stage 2: detail pages in parallel
        pacer.wait().await;
        let http = ctx.http.clone();
        let enriched: Vec<RawTorrent> = stream::iter(stage1.into_iter())
            .map(move |mut row| {
                let http = http.clone();
                async move {
                    let detail_url = row.detail_url.clone();
                    match fetch_detail(&http, &detail_url).await {
                        Ok(magnet) => {
                            row.info_hash = extract_info_hash_from_magnet(&magnet);
                            row.magnet = Some(magnet);
                        }
                        Err(e) => {
                            tracing::warn!(
                                provider = "1337x",
                                detail_url,
                                error = %e,
                                "1337x detail fetch failed; keeping row with torrent_url"
                            );
                        }
                    }
                    to_raw(row)
                }
            })
            .buffer_unordered(DETAIL_CONCURRENCY)
            .collect()
            .await;

        tracing::debug!(
            provider = "1337x",
            raw_count = enriched.len(),
            "1337x stage2 complete"
        );
        Ok(enriched)
    }
}

// ── Internal row shape (pre-magnet) ─────────────────────────────────

#[derive(Debug, Clone)]
pub(crate) struct SearchRow {
    raw_title: String,
    detail_url: String, // absolute URL
    torrent_url: Option<String>,
    info_hash: Option<String>,
    magnet: Option<String>,
    size_bytes: Option<u64>,
    seeders: u32,
    leechers: u32,
}

fn to_raw(r: SearchRow) -> RawTorrent {
    RawTorrent {
        source: "1337x",
        raw_title: r.raw_title,
        info_hash: r.info_hash,
        magnet: r.magnet,
        torrent_url: r.torrent_url.or(Some(r.detail_url)),
        size_bytes: r.size_bytes,
        seeders: r.seeders,
        leechers: r.leechers,
    }
}

// ── HTTP fetches ───────────────────────────────────────────────────

async fn fetch_search(
    http: &reqwest::Client,
    query: &str,
) -> Result<Vec<SearchRow>, ProviderError> {
    let url = format!("https://1337x.to/search/{}/1/", urlencoding::encode(query));
    let t = Instant::now();
    let resp = http
        .get(&url)
        .header("User-Agent", USER_AGENT)
        .send()
        .await?;
    let status = resp.status();
    tracing::debug!(
        provider = "1337x",
        request_url = %url,
        request_method = "GET",
        response_status = status.as_u16(),
        response_ms = t.elapsed().as_millis() as u64,
        "1337x stage1 call"
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
    let body = resp.text().await?;
    parse_search_html(&body).map_err(|e| ProviderError::Parse(format!("1337x search: {e}")))
}

async fn fetch_detail(http: &reqwest::Client, url: &str) -> Result<String, ProviderError> {
    let t = Instant::now();
    let resp = http
        .get(url)
        .header("User-Agent", USER_AGENT)
        .send()
        .await?;
    let status = resp.status();
    tracing::debug!(
        provider = "1337x",
        request_url = url,
        request_method = "GET",
        response_status = status.as_u16(),
        response_ms = t.elapsed().as_millis() as u64,
        "1337x stage2 call"
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
    let body = resp.text().await?;
    parse_detail_html(&body).map_err(|e| ProviderError::Parse(format!("1337x detail: {e}")))
}

// ── HTML parsing ───────────────────────────────────────────────────

static ROW: LazyLock<Selector> = LazyLock::new(|| Selector::parse(ROW_SEL).unwrap());
static NAME_ANCHOR: LazyLock<Selector> =
    LazyLock::new(|| Selector::parse(NAME_ANCHOR_SEL).unwrap());
static SEEDS: LazyLock<Selector> = LazyLock::new(|| Selector::parse(SEEDS_SEL).unwrap());
static LEECHES: LazyLock<Selector> = LazyLock::new(|| Selector::parse(LEECHES_SEL).unwrap());
static SIZE: LazyLock<Selector> = LazyLock::new(|| Selector::parse(SIZE_SEL).unwrap());
static MAGNET: LazyLock<Selector> = LazyLock::new(|| Selector::parse(MAGNET_SEL).unwrap());

pub(crate) fn parse_search_html(body: &str) -> Result<Vec<SearchRow>, String> {
    let doc = Html::parse_document(body);
    let rows: Vec<_> = doc.select(&ROW).collect();
    if rows.is_empty() {
        return Err("selector table.table-list tbody tr matched 0 rows".to_string());
    }
    let mut out = Vec::new();
    for row in rows {
        let Some(anchor) = row.select(&NAME_ANCHOR).next() else {
            continue; // row without a usable name cell; skip
        };
        let raw_title = anchor.text().collect::<String>().trim().to_string();
        let href = anchor.value().attr("href").unwrap_or("").to_string();
        let detail_url = if href.starts_with("http") {
            href
        } else {
            format!("https://1337x.to{href}")
        };

        let seeders = row
            .select(&SEEDS)
            .next()
            .and_then(|e| e.text().collect::<String>().trim().parse::<u32>().ok())
            .unwrap_or(0);
        let leechers = row
            .select(&LEECHES)
            .next()
            .and_then(|e| e.text().collect::<String>().trim().parse::<u32>().ok())
            .unwrap_or(0);
        let size_text = row
            .select(&SIZE)
            .next()
            .map(|e| {
                // The size cell contains "12.3 GB" followed by a nested
                // <span class="seeds"> that duplicates the seeder count.
                // Take only the direct text nodes.
                e.children()
                    .filter_map(|c| c.value().as_text().map(|t| t.to_string()))
                    .collect::<String>()
                    .trim()
                    .to_string()
            })
            .unwrap_or_default();
        let size_bytes = parse_size_human(&size_text);

        out.push(SearchRow {
            raw_title,
            detail_url,
            torrent_url: None,
            info_hash: None,
            magnet: None,
            size_bytes,
            seeders,
            leechers,
        });
    }
    Ok(out)
}

pub(crate) fn parse_detail_html(body: &str) -> Result<String, String> {
    let doc = Html::parse_document(body);
    let Some(anchor) = doc.select(&MAGNET).next() else {
        return Err(
            "selector ul.download-links-dontblock a[href^='magnet:'] matched 0".to_string(),
        );
    };
    let href = anchor
        .value()
        .attr("href")
        .ok_or("magnet anchor has no href")?;
    Ok(href.to_string())
}

/// Same helper as nyaa but supporting space-less formats "12.3GB".
/// 1337x sometimes emits space-separated, sometimes not.
fn parse_size_human(s: &str) -> Option<u64> {
    let s = s.trim();
    if s.is_empty() {
        return None;
    }
    // Split at the first letter.
    let split = s
        .char_indices()
        .find(|(_, c)| c.is_alphabetic())
        .map(|(i, _)| i)?;
    let (num, unit) = s.split_at(split);
    let num = num.trim();
    let unit = unit.trim();
    let val: f64 = num.parse().ok()?;
    let mult: u64 = match unit {
        "B" => 1,
        "KiB" => 1024,
        "MiB" => 1024 * 1024,
        "GiB" => 1024 * 1024 * 1024,
        "TiB" => 1024u64.pow(4),
        "KB" => 1000,
        "MB" => 1_000_000,
        "GB" => 1_000_000_000,
        "TB" => 1_000_000_000_000,
        _ => return None,
    };
    Some((val * mult as f64) as u64)
}

#[cfg(test)]
mod tests {
    use super::*;

    const SEARCH_FIXTURE: &str = include_str!("fixtures/1337x_search.html");
    const DETAIL_FIXTURE: &str = include_str!("fixtures/1337x_detail.html");

    #[test]
    fn parses_search_listing() {
        let rows = parse_search_html(SEARCH_FIXTURE).unwrap();
        assert_eq!(rows.len(), 3);
        let top = &rows[0];
        assert_eq!(top.raw_title, "Blow.Up.1966.1080p.BluRay.x265-FraMeSToR");
        assert_eq!(top.seeders, 214);
        assert_eq!(top.leechers, 18);
        assert_eq!(top.size_bytes, Some(12_300_000_000));
        assert!(
            top.detail_url
                .starts_with("https://1337x.to/torrent/1000001/")
        );
    }

    #[test]
    fn parses_detail_magnet() {
        let magnet = parse_detail_html(DETAIL_FIXTURE).unwrap();
        assert!(magnet.starts_with("magnet:?xt=urn:btih:AABBCCDDEEFF"));
        let hash = extract_info_hash_from_magnet(&magnet);
        assert_eq!(
            hash.as_deref(),
            Some("aabbccddeeff0011223344556677889900aabbcc")
        );
    }

    #[test]
    fn parse_size_handles_spaced_and_non_spaced() {
        assert_eq!(parse_size_human("12.3 GB"), Some(12_300_000_000));
        assert_eq!(parse_size_human("12.3GB"), Some(12_300_000_000));
        assert_eq!(parse_size_human("4.5 GB"), Some(4_500_000_000));
        assert_eq!(parse_size_human("500 MB"), Some(500_000_000));
        assert_eq!(parse_size_human("garbage"), None);
        assert_eq!(parse_size_human(""), None);
    }

    #[test]
    fn parse_search_fails_on_empty_body() {
        let err = parse_search_html("<html></html>").unwrap_err();
        assert!(err.contains("matched 0 rows"));
    }

    #[tokio::test]
    #[ignore]
    async fn onethreeseven_live_the_matrix() {
        let http = reqwest::Client::new();
        let rows = fetch_search(&http, "The Matrix")
            .await
            .expect("1337x live call should succeed");
        assert!(!rows.is_empty());
        // Fetch detail for the top row.
        let top = &rows[0];
        let magnet = fetch_detail(&http, &top.detail_url)
            .await
            .expect("1337x detail live call should succeed");
        assert!(magnet.starts_with("magnet:"));
    }
}
