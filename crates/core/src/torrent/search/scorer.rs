//! Weighted additive scoring per spec §5.
//!
//! Each dimension is an `i32` (signed — some produce negative values).
//! `ScoreBreakdown::total()` is the sum. Callers sort by `total()` desc.

use crate::torrent::search::types::{
    Codec, ParsedTitle, RawTorrent, Resolution, ScoreBreakdown, SourceKind,
};

pub fn score(raw: &RawTorrent, p: &ParsedTitle) -> ScoreBreakdown {
    ScoreBreakdown {
        seeders: seeders_score(raw.seeders),
        resolution: resolution_score(p.resolution),
        source: source_score(p.source_kind),
        codec: codec_score(p.codec),
        size: size_score(raw.size_bytes, p.resolution, p.codec),
        group: group_score(p.release_group.as_deref()),
        hdr: if p.hdr { 30 } else { 0 },
    }
}

fn seeders_score(s: u32) -> i32 {
    if s < 3 {
        -1000
    } else {
        (s.min(100) * 5) as i32
    }
}

fn resolution_score(r: Resolution) -> i32 {
    match r {
        Resolution::P2160 => 300,
        Resolution::P1080 => 200,
        Resolution::P720 => 100,
        Resolution::P480 => 30,
        Resolution::Sd => 10,
        Resolution::Unknown => 0,
    }
}

fn source_score(s: SourceKind) -> i32 {
    match s {
        SourceKind::Remux => 300,
        SourceKind::Bluray => 250,
        SourceKind::WebDl => 200,
        SourceKind::WebRip => 120,
        SourceKind::Hdtv => 80,
        SourceKind::Ts | SourceKind::Cam => -300,
        SourceKind::Unknown => 0,
    }
}

fn codec_score(c: Codec) -> i32 {
    match c {
        Codec::X265 | Codec::Av1 => 20,
        Codec::X264 => 10,
        Codec::Unknown => 0,
    }
}

/// Expected file size (bytes) for a given resolution × codec.
/// Returns 0 on unknown combinations.
fn expected_size(resolution: Resolution, codec: Codec) -> u64 {
    const GB: u64 = 1024 * 1024 * 1024;
    // Unknown codec → treat as x264 (larger).
    let is_efficient = matches!(codec, Codec::X265 | Codec::Av1);
    match (resolution, is_efficient) {
        (Resolution::P720, false) => 4 * GB,
        (Resolution::P720, true) => (3 * GB) / 2, // 1.5 GB
        (Resolution::P1080, false) => 10 * GB,
        (Resolution::P1080, true) => 4 * GB,
        (Resolution::P2160, false) => 40 * GB,
        (Resolution::P2160, true) => 25 * GB,
        _ => 0,
    }
}

fn size_score(size_bytes: Option<u64>, resolution: Resolution, codec: Codec) -> i32 {
    let Some(actual) = size_bytes else { return 0 };
    let expected = expected_size(resolution, codec);
    if expected == 0 {
        return 0;
    }
    // ratio = actual / expected
    let ratio = actual as f64 / expected as f64;
    if ratio < 0.3 {
        -150
    } else if ratio < 0.5 {
        -50
    } else if ratio <= 2.0 {
        0
    } else {
        -50
    }
}

fn group_score(group: Option<&str>) -> i32 {
    let Some(g) = group else { return 0 };
    // Case-insensitive compare.
    let lower = g.to_lowercase();
    const WHITELIST: &[&str] = &[
        "sparks",
        "geckos",
        "amiable",
        "framestor",
        "rarbg",
        "ntb",
        "cmrg",
        "kogi",
        "psa",
    ];
    const BLACKLIST: &[&str] = &["ganool", "etrg"];
    if WHITELIST.contains(&lower.as_str()) {
        50
    } else if BLACKLIST.contains(&lower.as_str()) {
        -100
    } else {
        0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn raw(seeders: u32, size: Option<u64>) -> RawTorrent {
        RawTorrent {
            source: "test",
            raw_title: "test".to_string(),
            info_hash: None,
            magnet: None,
            torrent_url: None,
            size_bytes: size,
            seeders,
            leechers: 0,
        }
    }

    fn parsed(
        resolution: Resolution,
        source_kind: SourceKind,
        codec: Codec,
        group: Option<&str>,
        hdr: bool,
    ) -> ParsedTitle {
        ParsedTitle {
            resolution,
            source_kind,
            codec,
            release_group: group.map(String::from),
            hdr,
        }
    }

    // ── seeders dimension ──

    #[test]
    fn seeders_below_floor_is_hard_negative() {
        assert_eq!(seeders_score(0), -1000);
        assert_eq!(seeders_score(1), -1000);
        assert_eq!(seeders_score(2), -1000);
    }

    #[test]
    fn seeders_at_three_crosses_floor() {
        assert_eq!(seeders_score(3), 15);
    }

    #[test]
    fn seeders_linear_in_range() {
        assert_eq!(seeders_score(50), 250);
        assert_eq!(seeders_score(100), 500);
    }

    #[test]
    fn seeders_capped_above_100() {
        assert_eq!(seeders_score(200), 500);
        assert_eq!(seeders_score(10_000), 500);
    }

    // ── resolution dimension ──

    #[test]
    fn resolution_weights() {
        assert_eq!(resolution_score(Resolution::P2160), 300);
        assert_eq!(resolution_score(Resolution::P1080), 200);
        assert_eq!(resolution_score(Resolution::P720), 100);
        assert_eq!(resolution_score(Resolution::P480), 30);
        assert_eq!(resolution_score(Resolution::Sd), 10);
        assert_eq!(resolution_score(Resolution::Unknown), 0);
    }

    // ── source dimension ──

    #[test]
    fn source_weights() {
        assert_eq!(source_score(SourceKind::Remux), 300);
        assert_eq!(source_score(SourceKind::Bluray), 250);
        assert_eq!(source_score(SourceKind::WebDl), 200);
        assert_eq!(source_score(SourceKind::WebRip), 120);
        assert_eq!(source_score(SourceKind::Hdtv), 80);
        assert_eq!(source_score(SourceKind::Ts), -300);
        assert_eq!(source_score(SourceKind::Cam), -300);
        assert_eq!(source_score(SourceKind::Unknown), 0);
    }

    // ── codec dimension ──

    #[test]
    fn codec_weights() {
        assert_eq!(codec_score(Codec::X265), 20);
        assert_eq!(codec_score(Codec::Av1), 20);
        assert_eq!(codec_score(Codec::X264), 10);
        assert_eq!(codec_score(Codec::Unknown), 0);
    }

    // ── size dimension ──

    const GB: u64 = 1024 * 1024 * 1024;

    #[test]
    fn size_none_returns_zero() {
        assert_eq!(size_score(None, Resolution::P1080, Codec::X264), 0);
    }

    #[test]
    fn size_unknown_resolution_returns_zero() {
        assert_eq!(
            size_score(Some(5 * GB), Resolution::Unknown, Codec::X264),
            0
        );
    }

    #[test]
    fn size_very_low_ratio_heavy_penalty() {
        // 1080p x264 expected 10 GB; 2 GB is ratio 0.2 → -150
        assert_eq!(
            size_score(Some(2 * GB), Resolution::P1080, Codec::X264),
            -150
        );
    }

    #[test]
    fn size_low_ratio_mild_penalty() {
        // 1080p x264 expected 10 GB; 4 GB is ratio 0.4 → -50
        assert_eq!(
            size_score(Some(4 * GB), Resolution::P1080, Codec::X264),
            -50
        );
    }

    #[test]
    fn size_in_healthy_range() {
        // 1080p x264 expected 10 GB; 10 GB is ratio 1.0 → 0
        assert_eq!(size_score(Some(10 * GB), Resolution::P1080, Codec::X264), 0);
        // 1080p x264 expected 10 GB; 15 GB is ratio 1.5 → 0
        assert_eq!(size_score(Some(15 * GB), Resolution::P1080, Codec::X264), 0);
    }

    #[test]
    fn size_too_large_penalty() {
        // 1080p x264 expected 10 GB; 25 GB is ratio 2.5 → -50
        assert_eq!(
            size_score(Some(25 * GB), Resolution::P1080, Codec::X264),
            -50
        );
    }

    #[test]
    fn size_x265_uses_smaller_expected() {
        // 1080p x265 expected 4 GB; 4 GB is ratio 1.0 → 0
        assert_eq!(size_score(Some(4 * GB), Resolution::P1080, Codec::X265), 0);
        // 1080p x265 expected 4 GB; 10 GB is ratio 2.5 → -50
        assert_eq!(
            size_score(Some(10 * GB), Resolution::P1080, Codec::X265),
            -50
        );
    }

    #[test]
    fn size_unknown_codec_treated_as_x264() {
        // 1080p unknown → expected 10 GB; 10 GB → 0
        assert_eq!(
            size_score(Some(10 * GB), Resolution::P1080, Codec::Unknown),
            0
        );
    }

    // ── group dimension ──

    #[test]
    fn group_whitelist_case_insensitive() {
        assert_eq!(group_score(Some("SPARKS")), 50);
        assert_eq!(group_score(Some("sparks")), 50);
        assert_eq!(group_score(Some("FraMeSToR")), 50);
        assert_eq!(group_score(Some("NTb")), 50);
    }

    #[test]
    fn group_blacklist() {
        assert_eq!(group_score(Some("Ganool")), -100);
        assert_eq!(group_score(Some("ETRG")), -100);
    }

    #[test]
    fn group_unknown_is_neutral() {
        assert_eq!(group_score(Some("YIFY")), 0);
        assert_eq!(group_score(Some("RandomGroup")), 0);
        assert_eq!(group_score(None), 0);
    }

    // ── total score ──

    #[test]
    fn total_high_quality_torrent() {
        let r = raw(200, Some(12_884_901_888)); // ~12 GB, 1080p x265 ratio 3.0 → -50
        let p = parsed(
            Resolution::P1080,
            SourceKind::Bluray,
            Codec::X265,
            Some("FraMeSToR"),
            true,
        );
        let b = score(&r, &p);
        assert_eq!(b.seeders, 500);
        assert_eq!(b.resolution, 200);
        assert_eq!(b.source, 250);
        assert_eq!(b.codec, 20);
        assert_eq!(b.size, -50); // bloat
        assert_eq!(b.group, 50);
        assert_eq!(b.hdr, 30);
        assert_eq!(b.total(), 1000);
    }

    #[test]
    fn total_trash_cam_is_heavily_negative() {
        let r = raw(5, Some(500_000_000)); // ~500 MB, CAM
        let p = parsed(Resolution::Sd, SourceKind::Cam, Codec::X264, None, false);
        let b = score(&r, &p);
        // seeders:25 res:10 source:-300 codec:10 size:0 group:0 hdr:0 = -255
        assert_eq!(b.total(), -255);
    }

    #[test]
    fn total_dead_torrent_hard_floor() {
        let r = raw(2, Some(4 * GB));
        let p = parsed(
            Resolution::P1080,
            SourceKind::Bluray,
            Codec::X265,
            Some("SPARKS"),
            false,
        );
        let b = score(&r, &p);
        // seeders:-1000 res:200 source:250 codec:20 size:0 group:50 hdr:0 = -480
        assert_eq!(b.seeders, -1000);
        assert_eq!(b.total(), -480);
    }
}
