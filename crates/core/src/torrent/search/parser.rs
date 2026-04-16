//! Release title parsing via regex.
//!
//! The provider returns `raw_title` verbatim from whatever the source
//! gave us (scene format, Nyaa fansub format, YTS bracket format, ...).
//! This module extracts structured fields so the scorer can rank them.
//!
//! First-match wins in regex priority order:
//!   resolution: 2160 → 1080 → 720 → 480
//!   source:     remux → bluray → webdl → webrip → hdtv → ts → cam
//!   codec:      x265/hevc → av1 → x264/avc

use crate::torrent::search::types::{Codec, ParsedTitle, Resolution, SourceKind};
use regex::Regex;
use std::sync::LazyLock;

// ── Regex constants ────────────────────────────────────────────────

static RES_2160: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)\b(?:2160p|4k)\b").unwrap());
static RES_1080: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)\b1080p\b").unwrap());
static RES_720: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)\b720p\b").unwrap());
static RES_480: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)\b480p\b").unwrap());

static SRC_REMUX: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)\bremux\b").unwrap());
static SRC_BLURAY: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)\b(?:bluray|blu-?ray|bdrip|brrip)\b").unwrap());
static SRC_WEBDL: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)\bweb-?dl\b").unwrap());
static SRC_WEBRIP: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)\bwebrip\b").unwrap());
static SRC_HDTV: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)\bhdtv\b").unwrap());
static SRC_TS: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)\b(?:ts|telesync)\b").unwrap());
static SRC_CAM: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)\b(?:cam|camrip|hdcam)\b").unwrap());

static CODEC_X265: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)\b(?:x265|h\.?265|hevc)\b").unwrap());
static CODEC_X264: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)\b(?:x264|h\.?264|avc)\b").unwrap());
static CODEC_AV1: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)\bav1\b").unwrap());

static HDR: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)\b(?:hdr|hdr10\+?|dolby.?vision)\b").unwrap());

/// Release group: trailing `-GROUP` (letters + digits, length ≥ 2),
/// optionally followed by a file extension we strip. Case-sensitive on
/// the capture so mixed-case names like "NTb" survive.
static RELEASE_GROUP: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"-([A-Za-z0-9]{2,})$").unwrap());

/// Matches known file extensions at end of title so we can chop them
/// off before running the release-group regex.
static TRAILING_EXT: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)\.(?:mp4|mkv|avi|mov|m4v|ts|torrent)$").unwrap());

// ── Public API ─────────────────────────────────────────────────────

pub fn parse_release_title(title: &str) -> ParsedTitle {
    ParsedTitle {
        resolution: parse_resolution(title),
        source_kind: parse_source_kind(title),
        codec: parse_codec(title),
        hdr: HDR.is_match(title),
        release_group: parse_release_group(title),
    }
}

fn parse_resolution(t: &str) -> Resolution {
    if RES_2160.is_match(t) {
        Resolution::P2160
    } else if RES_1080.is_match(t) {
        Resolution::P1080
    } else if RES_720.is_match(t) {
        Resolution::P720
    } else if RES_480.is_match(t) {
        Resolution::P480
    } else {
        Resolution::Unknown
    }
}

fn parse_source_kind(t: &str) -> SourceKind {
    if SRC_REMUX.is_match(t) {
        SourceKind::Remux
    } else if SRC_BLURAY.is_match(t) {
        SourceKind::Bluray
    } else if SRC_WEBDL.is_match(t) {
        SourceKind::WebDl
    } else if SRC_WEBRIP.is_match(t) {
        SourceKind::WebRip
    } else if SRC_HDTV.is_match(t) {
        SourceKind::Hdtv
    } else if SRC_TS.is_match(t) {
        SourceKind::Ts
    } else if SRC_CAM.is_match(t) {
        SourceKind::Cam
    } else {
        SourceKind::Unknown
    }
}

fn parse_codec(t: &str) -> Codec {
    if CODEC_X265.is_match(t) {
        Codec::X265
    } else if CODEC_AV1.is_match(t) {
        Codec::Av1
    } else if CODEC_X264.is_match(t) {
        Codec::X264
    } else {
        Codec::Unknown
    }
}

fn parse_release_group(t: &str) -> Option<String> {
    // Strip trailing file extension first so "...GROUP.mkv" still matches.
    let stripped = TRAILING_EXT.replace(t, "");
    RELEASE_GROUP.captures(&stripped).map(|c| c[1].to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(t: &str) -> ParsedTitle {
        parse_release_title(t)
    }

    #[test]
    fn yts_style_bracketed() {
        let p = parse("2046 (2004) [1080p] [BluRay] [5.1] [YTS.MX]");
        assert_eq!(p.resolution, Resolution::P1080);
        assert_eq!(p.source_kind, SourceKind::Bluray);
        assert_eq!(p.codec, Codec::Unknown);
        assert!(!p.hdr);
        assert_eq!(p.release_group, None);
    }

    #[test]
    fn scene_1080p_bluray_x265() {
        let p = parse("Blow.Up.1966.1080p.BluRay.x265-FraMeSToR");
        assert_eq!(p.resolution, Resolution::P1080);
        assert_eq!(p.source_kind, SourceKind::Bluray);
        assert_eq!(p.codec, Codec::X265);
        assert!(!p.hdr);
        assert_eq!(p.release_group.as_deref(), Some("FraMeSToR"));
    }

    #[test]
    fn uhd_remux_hdr() {
        let p = parse("The.Matrix.1999.2160p.UHD.BluRay.REMUX.HEVC.HDR.Atmos-FraMeSToR");
        assert_eq!(p.resolution, Resolution::P2160);
        assert_eq!(p.source_kind, SourceKind::Remux);
        assert_eq!(p.codec, Codec::X265);
        assert!(p.hdr);
        assert_eq!(p.release_group.as_deref(), Some("FraMeSToR"));
    }

    #[test]
    fn webdl_x264_sparks() {
        let p = parse("Parasite.2019.1080p.WEB-DL.DD5.1.x264-SPARKS");
        assert_eq!(p.resolution, Resolution::P1080);
        assert_eq!(p.source_kind, SourceKind::WebDl);
        assert_eq!(p.codec, Codec::X264);
        assert_eq!(p.release_group.as_deref(), Some("SPARKS"));
    }

    #[test]
    fn bluray_720p_amiable() {
        let p = parse("Oldboy.2003.720p.BluRay.x264-AMIABLE");
        assert_eq!(p.resolution, Resolution::P720);
        assert_eq!(p.source_kind, SourceKind::Bluray);
        assert_eq!(p.codec, Codec::X264);
        assert_eq!(p.release_group.as_deref(), Some("AMIABLE"));
    }

    #[test]
    fn mixed_case_group_ntb() {
        // "NTb" is mixed case — requires [A-Za-z0-9] in the group regex.
        let p = parse("In.the.Mood.for.Love.2000.1080p.BluRay.x265.HEVC.10bit-NTb");
        assert_eq!(p.release_group.as_deref(), Some("NTb"));
        assert_eq!(p.codec, Codec::X265);
    }

    #[test]
    fn dolby_vision_uhd() {
        let p = parse("Hero.2002.2160p.UHD.BluRay.REMUX.HEVC.Dolby.Vision-GECKOS");
        assert_eq!(p.resolution, Resolution::P2160);
        assert_eq!(p.source_kind, SourceKind::Remux);
        assert!(p.hdr);
        assert_eq!(p.release_group.as_deref(), Some("GECKOS"));
    }

    #[test]
    fn webdl_h265_hdr() {
        let p = parse("Raise.the.Red.Lantern.1991.HDR.2160p.WEB-DL.H265-Anon");
        assert_eq!(p.resolution, Resolution::P2160);
        assert_eq!(p.source_kind, SourceKind::WebDl);
        assert_eq!(p.codec, Codec::X265);
        assert!(p.hdr);
        assert_eq!(p.release_group.as_deref(), Some("Anon"));
    }

    #[test]
    fn bdrip_480p() {
        let p = parse("Wings.of.Desire.1987.480p.BDRip.x264-CG");
        assert_eq!(p.resolution, Resolution::P480);
        assert_eq!(p.source_kind, SourceKind::Bluray);
        assert_eq!(p.codec, Codec::X264);
        assert_eq!(p.release_group.as_deref(), Some("CG"));
    }

    #[test]
    fn bluray_720p_ncmt() {
        let p = parse("Ashes.of.Time.Redux.2008.CRITERION.720p.BluRay.DTS.x264-NCmt");
        assert_eq!(p.resolution, Resolution::P720);
        assert_eq!(p.release_group.as_deref(), Some("NCmt"));
    }

    #[test]
    fn hdtv_no_resolution() {
        let p = parse("Still.Life.2006.HDTV.XviD-SomeGrp");
        assert_eq!(p.resolution, Resolution::Unknown);
        assert_eq!(p.source_kind, SourceKind::Hdtv);
        assert_eq!(p.codec, Codec::Unknown);
        assert_eq!(p.release_group.as_deref(), Some("SomeGrp"));
    }

    #[test]
    fn scene_1080p_bluray_geckos() {
        let p = parse("The.Grandmaster.2013.LIMITED.1080p.BluRay.x264-GECKOS");
        assert_eq!(p.resolution, Resolution::P1080);
        assert_eq!(p.source_kind, SourceKind::Bluray);
        assert_eq!(p.release_group.as_deref(), Some("GECKOS"));
    }

    #[test]
    fn yify_no_dash_group() {
        // YIFY uses a trailing space/period separator without dash — no group match.
        let p = parse("Crouching.Tiger.Hidden.Dragon.2000.720p.BrRip.x264.YIFY");
        assert_eq!(p.resolution, Resolution::P720);
        assert_eq!(p.source_kind, SourceKind::Bluray); // brrip
        assert_eq!(p.release_group, None);
    }

    #[test]
    fn av1_codec() {
        let p = parse("Chungking.Express.1994.1080p.BluRay.DTS.AV1-AnimeGroup");
        assert_eq!(p.codec, Codec::Av1);
    }

    #[test]
    fn webrip_psa() {
        let p = parse("Happy.Together.1997.720p.WEBRip.x265-PSA");
        assert_eq!(p.source_kind, SourceKind::WebRip);
        assert_eq!(p.release_group.as_deref(), Some("PSA"));
    }

    #[test]
    fn four_k_keyword() {
        // "4K" should map to 2160p.
        let p = parse("Spring.Summer.Fall.Winter.and.Spring.2003.4K.UHD.BluRay-SPARKS");
        assert_eq!(p.resolution, Resolution::P2160);
        assert_eq!(p.source_kind, SourceKind::Bluray);
    }

    #[test]
    fn webrip_evo() {
        let p = parse("Farewell.My.Concubine.1993.1080p.WEBRip.AAC.x264-EVO");
        assert_eq!(p.source_kind, SourceKind::WebRip);
        assert_eq!(p.release_group.as_deref(), Some("EVO"));
    }

    #[test]
    fn cam_with_extension() {
        let p = parse("A.Touch.of.Sin.2013.CAM.avi");
        assert_eq!(p.source_kind, SourceKind::Cam);
        assert_eq!(p.release_group, None); // .avi stripped, no trailing -GROUP
    }

    #[test]
    fn two_char_group_dd() {
        let p = parse("Yi.Yi.2000.LIMITED.1080p.BluRay.x264-DD");
        assert_eq!(p.release_group.as_deref(), Some("DD"));
    }

    #[test]
    fn hdtv_480p_lol() {
        let p = parse("Suzhou.River.2000.480p.HDTV.h264-LOL");
        assert_eq!(p.resolution, Resolution::P480);
        assert_eq!(p.source_kind, SourceKind::Hdtv);
        assert_eq!(p.codec, Codec::X264); // h264 matches
        assert_eq!(p.release_group.as_deref(), Some("LOL"));
    }

    #[test]
    fn unknown_everything() {
        let p = parse("some random title that has no markers");
        assert_eq!(p.resolution, Resolution::Unknown);
        assert_eq!(p.source_kind, SourceKind::Unknown);
        assert_eq!(p.codec, Codec::Unknown);
        assert!(!p.hdr);
        assert_eq!(p.release_group, None);
    }

    #[test]
    fn extension_stripped_mkv() {
        // Trailing .mkv should be stripped so the group is still detected.
        let p = parse("Some.Film.1080p.BluRay.x264-EVO.mkv");
        assert_eq!(p.release_group.as_deref(), Some("EVO"));
    }
}
