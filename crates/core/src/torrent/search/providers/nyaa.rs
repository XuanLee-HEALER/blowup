//! Nyaa.si RSS provider.
//!
//! Endpoint: https://nyaa.si/?page=rss&q=<query>&c=0_0&f=0&s=seeders&o=desc
//!
//! Nyaa has no official JSON API — the RSS feed is the supported
//! interface. Each `<item>` has standard RSS fields plus a `nyaa:`
//! namespace that provides `seeders`, `leechers`, `infoHash`, `size`.
//! Magnets are NOT provided; we synthesize them from infoHash +
//! the tracker slice passed in via SearchContext.

use crate::torrent::search::provider::{CallPacer, SearchProvider, with_retry};
use crate::torrent::search::types::{ProviderError, RawTorrent, SearchContext};
use async_trait::async_trait;
use quick_xml::events::Event;
use quick_xml::reader::Reader;
use std::time::{Duration, Instant};

pub struct NyaaProvider;

impl NyaaProvider {
    pub fn new() -> Self {
        Self
    }
}

impl Default for NyaaProvider {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl SearchProvider for NyaaProvider {
    fn name(&self) -> &'static str {
        "nyaa"
    }

    fn min_interval(&self) -> Duration {
        Duration::from_secs(2)
    }

    async fn search(&self, ctx: &SearchContext<'_>) -> Result<Vec<RawTorrent>, ProviderError> {
        let mut pacer = CallPacer::new(self.min_interval());
        let query = ctx.title.to_string();

        let raws = with_retry(&mut pacer, self.max_retries(), || {
            fetch_and_parse(ctx.http, &query)
        })
        .await?;

        // Synthesize magnets from info_hash + trackers slice.
        let with_magnets: Vec<RawTorrent> = raws
            .into_iter()
            .map(|mut r| {
                if r.magnet.is_none()
                    && let Some(h) = &r.info_hash
                {
                    r.magnet = Some(make_magnet(h, &r.raw_title, ctx.trackers));
                }
                r
            })
            .collect();

        tracing::debug!(
            provider = "nyaa",
            raw_count = with_magnets.len(),
            "nyaa parse complete"
        );
        Ok(with_magnets)
    }
}

async fn fetch_and_parse(
    http: &reqwest::Client,
    query: &str,
) -> Result<Vec<RawTorrent>, ProviderError> {
    let url = format!(
        "https://nyaa.si/?page=rss&q={}&c=0_0&f=0&s=seeders&o=desc",
        urlencoding::encode(query)
    );
    let t = Instant::now();
    let resp = http
        .get(&url)
        .header(
            "User-Agent",
            "Mozilla/5.0 (compatible; blowup/1.0; +https://github.com/XuanLee-HEALER/blowup)",
        )
        .send()
        .await?;
    let status = resp.status();
    tracing::debug!(
        provider = "nyaa",
        request_url = %url,
        request_method = "GET",
        response_status = status.as_u16(),
        response_ms = t.elapsed().as_millis() as u64,
        "nyaa rss call"
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
    parse_rss(&body).map_err(|e| ProviderError::Parse(format!("nyaa rss: {e}")))
}

/// Parse a Nyaa RSS document into `RawTorrent` entries.
/// Exposed at module scope so tests can call it directly.
pub(crate) fn parse_rss(body: &str) -> Result<Vec<RawTorrent>, String> {
    let mut reader = Reader::from_str(body);
    reader.config_mut().trim_text(true);

    let mut out = Vec::new();
    let mut in_item = false;
    let mut cur = ItemBuilder::default();
    let mut current_tag: Option<String> = None;
    let mut buf = Vec::new();

    loop {
        match reader.read_event_into(&mut buf) {
            Err(e) => return Err(format!("xml parse error: {e}")),
            Ok(Event::Eof) => break,
            Ok(Event::Start(e)) => {
                let name = String::from_utf8_lossy(e.name().as_ref()).to_string();
                if name == "item" {
                    in_item = true;
                    cur = ItemBuilder::default();
                } else if in_item {
                    current_tag = Some(name);
                }
            }
            Ok(Event::End(e)) => {
                let name = String::from_utf8_lossy(e.name().as_ref()).to_string();
                if name == "item" {
                    if let Some(raw) = cur.build() {
                        out.push(raw);
                    }
                    in_item = false;
                    cur = ItemBuilder::default();
                } else if in_item {
                    current_tag = None;
                }
            }
            Ok(Event::Text(e)) => {
                if let Some(tag) = &current_tag {
                    let text = e.unescape().map_err(|x| x.to_string())?.into_owned();
                    cur.absorb(tag, &text);
                }
            }
            _ => {}
        }
        buf.clear();
    }
    Ok(out)
}

#[derive(Default)]
struct ItemBuilder {
    title: Option<String>,
    link: Option<String>,
    seeders: Option<u32>,
    leechers: Option<u32>,
    info_hash: Option<String>,
    size: Option<u64>,
}

impl ItemBuilder {
    fn absorb(&mut self, tag: &str, text: &str) {
        match tag {
            "title" => self.title = Some(text.to_string()),
            "link" => self.link = Some(text.to_string()),
            "nyaa:seeders" => self.seeders = text.parse().ok(),
            "nyaa:leechers" => self.leechers = text.parse().ok(),
            "nyaa:infoHash" => self.info_hash = Some(text.to_lowercase()),
            "nyaa:size" => self.size = parse_size_human(text),
            _ => {}
        }
    }

    fn build(self) -> Option<RawTorrent> {
        let title = self.title?;
        Some(RawTorrent {
            source: "nyaa",
            raw_title: title,
            info_hash: self.info_hash,
            magnet: None, // filled in by caller from trackers
            torrent_url: self.link,
            size_bytes: self.size,
            seeders: self.seeders.unwrap_or(0),
            leechers: self.leechers.unwrap_or(0),
        })
    }
}

/// Parse human size strings like "4.2 GiB" / "850 MiB" / "1.3 TiB"
/// into bytes. Returns None on malformed input.
fn parse_size_human(s: &str) -> Option<u64> {
    let s = s.trim();
    let (num, unit) = s.rsplit_once(' ')?;
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

/// Build a magnet URI from info_hash, display name, and tracker list.
pub(crate) fn make_magnet(info_hash: &str, title: &str, trackers: &[String]) -> String {
    let tr_params: String = trackers
        .iter()
        .map(|t| format!("&tr={}", urlencoding::encode(t)))
        .collect();
    format!(
        "magnet:?xt=urn:btih:{}&dn={}{}",
        info_hash,
        urlencoding::encode(title),
        tr_params,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    const FIXTURE: &str = include_str!("fixtures/nyaa_search.xml");

    #[test]
    fn parses_fixture_items() {
        let out = parse_rss(FIXTURE).unwrap();
        assert_eq!(out.len(), 2);

        let first = &out[0];
        assert_eq!(first.source, "nyaa");
        assert_eq!(first.raw_title, "Blow.Up.1966.1080p.BluRay.x265-FraMeSToR");
        assert_eq!(
            first.info_hash.as_deref(),
            Some("aabbccddeeff0011223344556677889900aabbcc")
        );
        assert_eq!(first.seeders, 127);
        assert_eq!(first.leechers, 8);
        assert_eq!(
            first.size_bytes,
            Some((4.2 * 1024.0 * 1024.0 * 1024.0) as u64)
        );
        assert!(first.torrent_url.is_some());
        // Magnet is filled by SearchProvider::search after parsing; parse_rss
        // leaves it None.
        assert!(first.magnet.is_none());

        let second = &out[1];
        assert_eq!(second.seeders, 43);
        assert_eq!(second.size_bytes, Some((850.0 * 1024.0 * 1024.0) as u64));
    }

    #[test]
    fn parse_size_human_handles_units() {
        assert_eq!(
            parse_size_human("4.2 GiB"),
            Some((4.2 * 1024.0f64.powi(3)) as u64)
        );
        assert_eq!(parse_size_human("850 MiB"), Some(850 * 1024 * 1024));
        assert_eq!(
            parse_size_human("1.3 TiB"),
            Some((1.3 * 1024.0f64.powi(4)) as u64)
        );
        assert_eq!(parse_size_human("500 KB"), Some(500_000));
        assert_eq!(parse_size_human("garbage"), None);
    }

    #[test]
    fn make_magnet_embeds_trackers() {
        let trackers = vec![
            "udp://tracker.opentrackr.org:1337/announce".to_string(),
            "udp://open.tracker.cl:1337/announce".to_string(),
        ];
        let m = make_magnet(
            "aabbccddeeff0011223344556677889900aabbcc",
            "Blow Up 1966",
            &trackers,
        );
        assert!(m.starts_with("magnet:?xt=urn:btih:aabbccddeeff0011223344556677889900aabbcc"));
        assert!(m.contains("&dn=Blow%20Up%201966"));
        assert!(m.contains("&tr=udp%3A%2F%2Ftracker.opentrackr.org%3A1337%2Fannounce"));
        assert!(m.contains("&tr=udp%3A%2F%2Fopen.tracker.cl%3A1337%2Fannounce"));
    }

    #[test]
    fn make_magnet_no_trackers_still_valid() {
        let m = make_magnet("aabb", "T", &[]);
        assert_eq!(m, "magnet:?xt=urn:btih:aabb&dn=T");
    }

    #[tokio::test]
    #[ignore]
    async fn nyaa_live_the_matrix() {
        let http = reqwest::Client::new();
        let raws = fetch_and_parse(&http, "The Matrix")
            .await
            .expect("nyaa live call should succeed");
        assert!(!raws.is_empty(), "expected at least one result");
        // Structural-only assertions.
        let first = &raws[0];
        assert!(first.info_hash.is_some());
        assert!(first.torrent_url.is_some());
    }
}
