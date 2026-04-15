//! Deduplicate raw torrent results by `info_hash`.
//!
//! Rules per spec §6:
//! - Same info_hash from multiple sources → merge into one, taking the
//!   max seeders/leechers and the first non-null magnet/torrent_url.
//! - Entries without info_hash are kept as independent results.
//! - Title fuzzy matching is NOT performed (too error-prone: "X 1080p"
//!   vs "X 720p" must not be merged).

use crate::torrent::search::types::RawTorrent;
use std::collections::HashMap;

pub fn merge(raws: Vec<RawTorrent>) -> Vec<RawTorrent> {
    let mut by_hash: HashMap<String, RawTorrent> = HashMap::new();
    let mut without_hash: Vec<RawTorrent> = Vec::new();

    for r in raws {
        match r.info_hash.clone() {
            Some(h) => {
                by_hash
                    .entry(h)
                    .and_modify(|existing| merge_into(existing, &r))
                    .or_insert(r);
            }
            None => without_hash.push(r),
        }
    }

    let mut out: Vec<RawTorrent> = by_hash.into_values().collect();
    out.extend(without_hash);
    out
}

fn merge_into(existing: &mut RawTorrent, new: &RawTorrent) {
    existing.seeders = existing.seeders.max(new.seeders);
    existing.leechers = existing.leechers.max(new.leechers);
    if existing.magnet.is_none() && new.magnet.is_some() {
        existing.magnet = new.magnet.clone();
    }
    if existing.torrent_url.is_none() && new.torrent_url.is_some() {
        existing.torrent_url = new.torrent_url.clone();
    }
    // size_bytes: prefer first seen; different sources sometimes
    // disagree on exact size, not worth reconciling.
    if existing.size_bytes.is_none() && new.size_bytes.is_some() {
        existing.size_bytes = new.size_bytes;
    }
    // source tag kept as first-seen; multi-source origin only visible
    // in trace logs.
}

#[cfg(test)]
mod tests {
    use super::*;

    fn raw(
        source: &'static str,
        hash: Option<&str>,
        magnet: Option<&str>,
        seeders: u32,
    ) -> RawTorrent {
        RawTorrent {
            source,
            raw_title: format!("{source} {hash:?}"),
            info_hash: hash.map(String::from),
            magnet: magnet.map(String::from),
            torrent_url: None,
            size_bytes: None,
            seeders,
            leechers: 0,
        }
    }

    #[test]
    fn empty_input_returns_empty() {
        assert!(merge(vec![]).is_empty());
    }

    #[test]
    fn single_entry_passes_through() {
        let r = raw("yts", Some("abc"), Some("magnet:?xt=urn:btih:abc"), 10);
        let out = merge(vec![r]);
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].seeders, 10);
    }

    #[test]
    fn same_hash_merged_max_seeders() {
        let a = raw("yts", Some("abc"), Some("magnet:yts"), 50);
        let b = raw("nyaa", Some("abc"), Some("magnet:nyaa"), 80);
        let out = merge(vec![a, b]);
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].seeders, 80);
    }

    #[test]
    fn missing_magnet_filled_from_second() {
        let a = raw("yts", Some("abc"), None, 10);
        let b = raw("nyaa", Some("abc"), Some("magnet:nyaa"), 20);
        let out = merge(vec![a, b]);
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].magnet.as_deref(), Some("magnet:nyaa"));
    }

    #[test]
    fn different_hashes_kept_separate() {
        let a = raw("yts", Some("abc"), Some("m1"), 10);
        let b = raw("nyaa", Some("def"), Some("m2"), 20);
        let out = merge(vec![a, b]);
        assert_eq!(out.len(), 2);
    }

    #[test]
    fn no_hash_entries_kept_as_independent() {
        let a = raw("yts", None, Some("m1"), 5);
        let b = raw("nyaa", None, Some("m2"), 7);
        let out = merge(vec![a, b]);
        assert_eq!(out.len(), 2);
    }

    #[test]
    fn mix_of_hashed_and_unhashed() {
        let a = raw("yts", Some("abc"), Some("m1"), 10);
        let b = raw("nyaa", Some("abc"), Some("m2"), 20);
        let c = raw("1337x", None, Some("m3"), 5);
        let out = merge(vec![a, b, c]);
        // One merged (hash abc) + one unhashed = 2 total
        assert_eq!(out.len(), 2);
        // The merged one has seeders=20
        let merged = out.iter().find(|r| r.info_hash.is_some()).unwrap();
        assert_eq!(merged.seeders, 20);
    }
}
