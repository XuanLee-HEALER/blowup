//! YTS / yify torrent search — stateless external API client.
//! Uses the `movies-api.accel.li` mirror because the original
//! `yts.torrentbay.st` now returns HTML instead of JSON.
//!
//! No DB, no file IO, no state. The only runtime dependency besides
//! `reqwest::Client` is the TMDB API key (optional fallback for
//! resolving an IMDB id from a TMDB id).

pub mod dedup;
pub mod parser;
pub mod provider;
pub mod scorer;
pub mod types;

use crate::error::SearchError;
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::sync::LazyLock;

#[derive(Debug, Clone, Serialize)]
pub struct MovieResult {
    pub title: String,
    pub year: u32,
    pub quality: String,
    pub magnet: Option<String>,
    pub torrent_url: Option<String>,
    pub seeds: u32,
}

pub async fn search_yify(
    client: &reqwest::Client,
    tmdb_api_key: &str,
    query: &str,
    year: Option<u32>,
    tmdb_id: Option<u64>,
) -> Result<Vec<MovieResult>, SearchError> {
    tracing::info!(query, ?year, ?tmdb_id, "yify search started");

    // 1. Try IMDB ID from TMDB (most reliable)
    if let Some(id) = tmdb_id {
        tracing::debug!(tmdb_id = id, "fetching IMDB ID from TMDB");
        if let Some(imdb_id) = fetch_imdb_id(client, tmdb_api_key, id).await {
            tracing::debug!(imdb_id = %imdb_id, "got IMDB ID, searching YTS");
            if let Ok(results) = search_via_api(client, &imdb_id, None).await
                && !results.is_empty()
            {
                tracing::info!(count = results.len(), "found via IMDB ID");
                return Ok(results);
            }
            tracing::debug!("IMDB ID search returned no results");
        } else {
            tracing::debug!("could not resolve IMDB ID");
        }
    }

    // 2. Try original title
    tracing::debug!(query, "searching by original title");
    if let Ok(results) = search_via_api(client, query, year).await
        && !results.is_empty()
    {
        tracing::info!(count = results.len(), "found via original title");
        return Ok(results);
    }

    // 3. Fallback: strip special characters and retry
    let sanitized = sanitize_query(query);
    if sanitized != query {
        tracing::debug!(sanitized, "retrying with sanitized title");
        if let Ok(results) = search_via_api(client, &sanitized, year).await
            && !results.is_empty()
        {
            tracing::info!(count = results.len(), "found via sanitized title");
            return Ok(results);
        }
    }

    tracing::warn!(query, "no results after all fallbacks");
    Err(SearchError::NoResults(query.to_string()))
}

/// Fetch IMDB ID for a TMDB movie via the TMDB external_ids endpoint.
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

static SANITIZE_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"[^\w\s]").expect("valid sanitize regex"));

fn sanitize_query(query: &str) -> String {
    let cleaned = SANITIZE_RE.replace_all(query, " ");
    cleaned.split_whitespace().collect::<Vec<_>>().join(" ")
}

async fn search_via_api(
    client: &reqwest::Client,
    query: &str,
    year: Option<u32>,
) -> Result<Vec<MovieResult>, SearchError> {
    tracing::debug!(query, ?year, "YTS API request");
    let mut params = vec![
        ("query_term", query.to_string()),
        ("sort_by", "seeds".to_string()),
        ("order_by", "desc".to_string()),
    ];
    if let Some(y) = year {
        params.push(("year", y.to_string()));
    }

    let resp = client
        .get("https://movies-api.accel.li/api/v2/list_movies.json")
        .query(&params)
        .header("User-Agent", "blowup/0.1")
        .send()
        .await?;

    let body: YtsResponse = resp.json().await?;
    parse_yts_response(body)
}

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
    torrents: Vec<YtsTorrent>,
}

#[derive(Deserialize)]
struct YtsTorrent {
    quality: String,
    #[serde(rename = "url")]
    url: String,
    seeds: u32,
    #[serde(default)]
    magnet_url: Option<String>,
}

fn parse_yts_response(resp: YtsResponse) -> Result<Vec<MovieResult>, SearchError> {
    let mut results: Vec<MovieResult> = resp
        .data
        .movies
        .into_iter()
        .flat_map(|movie| {
            let title = movie.title.clone();
            let year = movie.year;
            movie.torrents.into_iter().map(move |t| MovieResult {
                title: title.clone(),
                year,
                quality: t.quality,
                magnet: t.magnet_url,
                torrent_url: Some(t.url),
                seeds: t.seeds,
            })
        })
        .collect();

    results.sort_by(|a, b| {
        quality_rank(&b.quality)
            .cmp(&quality_rank(&a.quality))
            .then(b.seeds.cmp(&a.seeds))
    });

    Ok(results)
}

fn quality_rank(q: &str) -> u8 {
    match q {
        "2160p" => 4,
        "1080p" => 3,
        "720p" => 2,
        _ => 1,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn make_yts_response(movies: serde_json::Value) -> YtsResponse {
        serde_json::from_value(json!({"data": {"movies": movies}})).unwrap()
    }

    #[test]
    fn parse_single_movie() {
        let resp = make_yts_response(json!([{
            "title": "Blow-Up",
            "year": 1966,
            "torrents": [
                {"quality": "1080p", "url": "http://x.com/a.torrent", "seeds": 100},
                {"quality": "720p",  "url": "http://x.com/b.torrent", "seeds": 200}
            ]
        }]));
        let results = parse_yts_response(resp).unwrap();
        assert_eq!(results.len(), 2);
        assert_eq!(results[0].quality, "1080p");
    }

    #[test]
    fn quality_rank_order() {
        assert!(quality_rank("1080p") > quality_rank("720p"));
        assert!(quality_rank("2160p") > quality_rank("1080p"));
    }

    #[test]
    fn empty_movies_returns_empty_vec() {
        let resp = make_yts_response(json!([]));
        let results = parse_yts_response(resp).unwrap();
        assert!(results.is_empty());
    }
}
