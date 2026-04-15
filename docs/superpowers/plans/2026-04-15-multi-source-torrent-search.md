# Multi-Source Torrent Search Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Refactor `crates/core/src/torrent/search.rs` from a single-source YTS client into a pluggable multi-source pipeline (YTS + Nyaa + 1337x) with per-provider rate-limited retries, `info_hash` dedup, and weighted 7-dimension scoring.

**Architecture:** The old single file becomes a module directory under `crates/core/src/torrent/search/`. A `SearchProvider` trait abstracts each source; stateless providers with per-call `CallPacer`s enforce rate limits within retries and between sub-requests. The orchestrator fans out providers concurrently via `futures::future::join_all`, silently logs per-provider failures, dedups by `info_hash`, parses release titles for resolution/source/codec/group, scores, and returns a sorted `Vec<ScoredTorrent>`. Failures never bubble up — the frontend sees either results or an empty list. Callers pull the shared `reqwest::Client` and `TrackerManager` from `AppContext`, replacing the old per-call `Client::new()`.

**Tech Stack:** Rust (blowup-core / blowup-tauri / blowup-server), new deps `async-trait` + `quick-xml` + `scraper` + `futures` + `urlencoding`; React 19 + Mantine on the frontend. No database changes.

**Reference spec:** `docs/superpowers/specs/2026-04-15-multi-source-torrent-search-design.md` — consult for any question not covered in a step.

**Baseline:** branch `main` at commit `1a9ed1e` (the spec commit). The engineer may work directly on `main` since each task is tested + committed independently. A worktree is optional.

**Verification gate:** every task ends with `just check` green before the commit. If any step fails, fix in place before moving on — no amending prior commits.

---

## File Structure (target end state)

```
crates/core/src/torrent/search/             # replaces single search.rs file
├── mod.rs              # Public API: search_movie() orchestrator
├── types.rs            # SearchQuery / SearchContext / RawTorrent / ScoredTorrent / enums / ProviderError
├── provider.rs         # SearchProvider trait + CallPacer + with_retry helper
├── parser.rs           # parse_release_title() — regex extraction
├── scorer.rs           # score() + ScoreBreakdown weights
├── dedup.rs            # merge() — group by info_hash
└── providers/
    ├── mod.rs          # re-exports + build_default_providers()
    ├── yts.rs          # YTS API client
    ├── nyaa.rs         # Nyaa.si RSS parser
    ├── onethreeseven.rs# 1337x HTML scraper
    └── fixtures/       # test fixtures — committed as files next to the tests
        ├── nyaa_search.xml
        ├── 1337x_search.html
        └── 1337x_detail.html
```

**Deleted end state**:
- `crates/core/src/torrent/search.rs` (old monolithic file)
- `SearchConfig` struct in `crates/core/src/config/mod.rs`
- `SearchError` enum in `crates/core/src/error.rs` (no longer needed — providers use `ProviderError`)
- `search_yify_cmd` in `crates/tauri/src/commands/search.rs` → renamed to `search_movie_cmd`
- `yts` wrapper in `src/lib/tauri.ts` → renamed to `search.movie`
- `MovieResult` TS interface → replaced by `ScoredTorrent`
- Settings UI block for `rate_limit_secs`

---

## Task 1: Scaffold the module directory (no behavior change)

**Goal:** Convert `search.rs` single file into `search/mod.rs` under a new directory, with no code change. Everything still compiles and the old `search_yify` still works exactly as before. This is a pure file-move so subsequent tasks have a place to land new files.

**Files:**
- Delete: `crates/core/src/torrent/search.rs`
- Create: `crates/core/src/torrent/search/mod.rs` (copied verbatim from old `search.rs`)

- [ ] **Step 1: Move the file**

```bash
mkdir -p crates/core/src/torrent/search
git mv crates/core/src/torrent/search.rs crates/core/src/torrent/search/mod.rs
```

- [ ] **Step 2: Verify the workspace still compiles**

Run: `just check`
Expected: all green (`Finished ... test profile` + all tests pass). `blowup-core` / `blowup-tauri` / `blowup-server` all unchanged.

- [ ] **Step 3: Commit**

```bash
git commit -m "$(cat <<'EOF'
refactor(torrent/search): move search.rs into search/ module dir

Pure file-move — no behavior change. Makes room for the multi-source
search pipeline per docs/superpowers/specs/2026-04-15-multi-source-
torrent-search-design.md.

Co-Authored-By: Claude Opus 4.6 (1M context) <noreply@anthropic.com>
EOF
)"
```

---

## Task 2: Add new Rust dependencies

**Goal:** Add `async-trait` + `quick-xml` + `scraper` + `futures` + `urlencoding` to `blowup-core`, verify they compile. No Rust code changes yet.

**Files:**
- Modify: `crates/core/Cargo.toml`

- [ ] **Step 1: Add deps to `[dependencies]`**

Open `crates/core/Cargo.toml`. After the `librqbit = "8.1"` line, add:

```toml
async-trait           = "0.1"
quick-xml             = { version = "0.36", features = ["serialize"] }
scraper               = "0.20"
futures               = "0.3"
urlencoding           = "2.1"
```

- [ ] **Step 2: Build to download + compile the deps**

Run: `cargo build -p blowup-core`
Expected: green build. Downloads `async-trait`, `quick-xml`, `scraper`, `futures`, `urlencoding` and their transitive deps. No warnings new from our code (dep warnings are fine).

- [ ] **Step 3: Run full check**

Run: `just check`
Expected: green.

- [ ] **Step 4: Commit**

```bash
git add crates/core/Cargo.toml Cargo.lock
git commit -m "$(cat <<'EOF'
chore(core): add async-trait, quick-xml, scraper, futures, urlencoding

Dependencies for the upcoming multi-source torrent search pipeline.

Co-Authored-By: Claude Opus 4.6 (1M context) <noreply@anthropic.com>
EOF
)"
```

---

## Task 3: Core types and enums (`types.rs`)

**Goal:** Create the type vocabulary: `Resolution` / `SourceKind` / `Codec` enums, `SearchQuery` / `SearchContext` / `RawTorrent` / `ParsedTitle` / `ScoreBreakdown` / `ScoredTorrent` / `ProviderError` structs. No logic — just data shapes.

**Files:**
- Create: `crates/core/src/torrent/search/types.rs`
- Modify: `crates/core/src/torrent/search/mod.rs` (add `pub mod types;`)

- [ ] **Step 1: Create `types.rs`**

Full file contents:

```rust
//! Type vocabulary for the multi-source torrent search pipeline.
//!
//! No logic lives here — just the shared data shapes passed between
//! the orchestrator, providers, parser, scorer, and dedup layers.

use serde::{Deserialize, Serialize};
use thiserror::Error;

// ── Query & context ────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct SearchQuery {
    pub title: String,
    pub year: Option<u32>,
    pub tmdb_id: Option<u64>,
    pub tmdb_api_key: String,
}

/// Passed to every `SearchProvider::search` call. Borrows everything.
#[derive(Debug, Clone, Copy)]
pub struct SearchContext<'a> {
    pub http: &'a reqwest::Client,
    pub title: &'a str,
    pub year: Option<u32>,
    /// Pre-resolved IMDB id (e.g., "tt0061791"). Orchestrator fetches
    /// this once from TMDB and hands it to every provider.
    pub imdb_id: Option<&'a str>,
    pub tmdb_api_key: &'a str,
    /// Snapshot of `TrackerManager::hot_trackers()` taken once per
    /// search. Nyaa uses this to synthesize magnet URIs.
    pub trackers: &'a [String],
}

// ── Raw provider output ────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct RawTorrent {
    pub source: &'static str,
    pub raw_title: String,
    /// Lowercase hex, 40 chars. None if the provider couldn't supply one.
    pub info_hash: Option<String>,
    pub magnet: Option<String>,
    pub torrent_url: Option<String>,
    pub size_bytes: Option<u64>,
    pub seeders: u32,
    pub leechers: u32,
}

// ── Parsed title fields ────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Resolution {
    Unknown,
    Sd,
    P480,
    P720,
    P1080,
    P2160,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SourceKind {
    Unknown,
    Cam,
    Ts,
    Hdtv,
    WebRip,
    WebDl,
    Bluray,
    Remux,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Codec {
    Unknown,
    X264,
    X265,
    Av1,
}

#[derive(Debug, Clone)]
pub struct ParsedTitle {
    pub resolution: Resolution,
    pub source_kind: SourceKind,
    pub codec: Codec,
    pub release_group: Option<String>,
    pub hdr: bool,
}

// ── Scoring ────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct ScoreBreakdown {
    pub seeders: i32,
    pub resolution: i32,
    pub source: i32,
    pub codec: i32,
    pub size: i32,
    pub group: i32,
    pub hdr: i32,
}

impl ScoreBreakdown {
    pub fn total(&self) -> i32 {
        self.seeders
            + self.resolution
            + self.source
            + self.codec
            + self.size
            + self.group
            + self.hdr
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct ScoredTorrent {
    pub source: &'static str,
    pub raw_title: String,
    pub info_hash: Option<String>,
    pub magnet: Option<String>,
    pub torrent_url: Option<String>,
    pub size_bytes: Option<u64>,
    pub seeders: u32,
    pub leechers: u32,
    pub resolution: Resolution,
    pub source_kind: SourceKind,
    pub codec: Codec,
    pub release_group: Option<String>,
    pub hdr: bool,
    pub score: i32,
    pub breakdown: ScoreBreakdown,
}

// ── Provider error ─────────────────────────────────────────────────

#[derive(Debug, Error)]
pub enum ProviderError {
    #[error("network timeout")]
    Timeout,
    #[error("connect failed: {0}")]
    Connect(String),
    #[error("http 5xx: {0}")]
    Http5xx(u16),
    #[error("http 429 rate limited")]
    Http429,
    #[error("http 4xx: {0}")]
    Http4xx(u16),
    #[error("parse error: {0}")]
    Parse(String),
}

impl ProviderError {
    pub fn is_retryable(&self) -> bool {
        matches!(
            self,
            Self::Timeout | Self::Connect(_) | Self::Http5xx(_) | Self::Http429
        )
    }
}

/// Classify a `reqwest::Error` into the matching `ProviderError`.
/// Kept here so every provider uses the same rules.
impl From<reqwest::Error> for ProviderError {
    fn from(e: reqwest::Error) -> Self {
        if e.is_timeout() {
            Self::Timeout
        } else if e.is_connect() {
            Self::Connect(e.to_string())
        } else if let Some(status) = e.status() {
            let code = status.as_u16();
            match code {
                429 => Self::Http429,
                500..=599 => Self::Http5xx(code),
                400..=499 => Self::Http4xx(code),
                _ => Self::Connect(e.to_string()),
            }
        } else {
            Self::Connect(e.to_string())
        }
    }
}
```

- [ ] **Step 2: Register the module in `search/mod.rs`**

Open `crates/core/src/torrent/search/mod.rs`. At the very top (before the existing `//! YTS ...` doc comment), add:

```rust
pub mod types;
```

- [ ] **Step 3: Build to verify types compile**

Run: `cargo build -p blowup-core`
Expected: green. No warnings from `types.rs`. Some `dead_code` warnings are acceptable and will go away as later tasks use the types.

- [ ] **Step 4: Run full check**

Run: `just check`
Expected: green.

- [ ] **Step 5: Commit**

```bash
git add crates/core/src/torrent/search/types.rs crates/core/src/torrent/search/mod.rs
git commit -m "$(cat <<'EOF'
feat(search): add types module for multi-source pipeline

Data vocabulary for the new search orchestrator: SearchQuery,
SearchContext, RawTorrent, ScoredTorrent, Resolution/SourceKind/Codec
enums, ScoreBreakdown with total(), ProviderError with classify.

Co-Authored-By: Claude Opus 4.6 (1M context) <noreply@anthropic.com>
EOF
)"
```

---

## Task 4: Release title parser (`parser.rs`) — TDD

**Goal:** Implement `parse_release_title(title: &str) -> ParsedTitle` that extracts `resolution` / `source_kind` / `codec` / `release_group` / `hdr` via regex. Driven by 20 golden fixtures.

**Files:**
- Create: `crates/core/src/torrent/search/parser.rs`
- Modify: `crates/core/src/torrent/search/mod.rs` (add `pub mod parser;`)

- [ ] **Step 1: Register the module**

Open `crates/core/src/torrent/search/mod.rs`. After `pub mod types;`, add:

```rust
pub mod parser;
```

- [ ] **Step 2: Write the failing test file**

Create `crates/core/src/torrent/search/parser.rs` with a stub function and the golden test cases. This is the "red" phase of TDD — the stub returns defaults, every assertion will fail.

```rust
//! Release title parsing via regex.
//!
//! The provider returns `raw_title` verbatim from whatever the source
//! gave us (scene format, Nyaa fansub format, YTS bracket format, ...).
//! This module extracts structured fields so the scorer can rank them.
//!
//! First-match wins in regex priority order:
//!   resolution: 2160 → 1080 → 720 → 480
//!   source:     remux → bluray → webdl → webrip → hdtv → ts → cam
//!   codec:      x265/hevc → x264/avc → av1

use crate::torrent::search::types::{Codec, ParsedTitle, Resolution, SourceKind};
use regex::Regex;
use std::sync::LazyLock;

// ── Regex constants ────────────────────────────────────────────────

static RES_2160: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)\b(?:2160p|4k|uhd)\b").unwrap());
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
    LazyLock::new(|| Regex::new(r"(?i)\b(?:hdr|hdr10|dv|dolby.?vision)\b").unwrap());

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
    RELEASE_GROUP
        .captures(&stripped)
        .map(|c| c[1].to_string())
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
```

- [ ] **Step 3: Run the tests to verify they PASS on first write**

Note: because the implementation is in the same file and already correct, the "failing" phase is already past. TDD purists can comment out each regex body and see them fail, but that's busywork. The golden fixtures serve as regression tests from here on.

Run: `cargo test -p blowup-core torrent::search::parser --lib`
Expected: all 22 tests pass.

- [ ] **Step 4: Run full check**

Run: `just check`
Expected: green.

- [ ] **Step 5: Commit**

```bash
git add crates/core/src/torrent/search/parser.rs crates/core/src/torrent/search/mod.rs
git commit -m "$(cat <<'EOF'
feat(search): add release title parser with 22 golden fixtures

Regex-based extraction of resolution/source/codec/release_group/HDR.
First-match-wins priority order per spec §4. Mixed-case release group
names like "NTb" are preserved via [A-Za-z0-9] character class.
Trailing .mp4/.mkv/etc extensions are stripped before group matching.

Co-Authored-By: Claude Opus 4.6 (1M context) <noreply@anthropic.com>
EOF
)"
```

---

## Task 5: Scorer (`scorer.rs`) — TDD

**Goal:** Implement `score(raw, parsed) -> ScoreBreakdown` with the weights from spec §5.

**Files:**
- Create: `crates/core/src/torrent/search/scorer.rs`
- Modify: `crates/core/src/torrent/search/mod.rs` (add `pub mod scorer;`)

- [ ] **Step 1: Register the module**

Append to `crates/core/src/torrent/search/mod.rs`:

```rust
pub mod scorer;
```

- [ ] **Step 2: Write `scorer.rs` with implementation AND tests**

```rust
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
    if WHITELIST.iter().any(|w| *w == lower.as_str()) {
        50
    } else if BLACKLIST.iter().any(|w| *w == lower.as_str()) {
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
        assert_eq!(
            size_score(Some(10 * GB), Resolution::P1080, Codec::X264),
            0
        );
        // 1080p x264 expected 10 GB; 15 GB is ratio 1.5 → 0
        assert_eq!(
            size_score(Some(15 * GB), Resolution::P1080, Codec::X264),
            0
        );
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
        assert_eq!(
            size_score(Some(4 * GB), Resolution::P1080, Codec::X265),
            0
        );
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
```

- [ ] **Step 3: Run the scorer tests**

Run: `cargo test -p blowup-core torrent::search::scorer --lib`
Expected: all ~25 tests pass.

- [ ] **Step 4: Full check**

Run: `just check`
Expected: green.

- [ ] **Step 5: Commit**

```bash
git add crates/core/src/torrent/search/scorer.rs crates/core/src/torrent/search/mod.rs
git commit -m "$(cat <<'EOF'
feat(search): add weighted scoring with per-dimension breakdown

7 dimensions: seeders / resolution / source / codec / size / group /
hdr. Weights per spec §5. Hard floor at seeders<3 (-1000). Size is
scored against expected ranges per (resolution, codec). 23 unit tests
cover boundaries in each dimension + three end-to-end totals.

Co-Authored-By: Claude Opus 4.6 (1M context) <noreply@anthropic.com>
EOF
)"
```

---

## Task 6: Dedup (`dedup.rs`) — TDD

**Goal:** `merge(raws: Vec<RawTorrent>) -> Vec<RawTorrent>` groups by `info_hash`. Entries without `info_hash` pass through as independent results.

**Files:**
- Create: `crates/core/src/torrent/search/dedup.rs`
- Modify: `crates/core/src/torrent/search/mod.rs` (add `pub mod dedup;`)

- [ ] **Step 1: Register module**

Append to `crates/core/src/torrent/search/mod.rs`:

```rust
pub mod dedup;
```

- [ ] **Step 2: Implement + test**

```rust
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
```

- [ ] **Step 3: Run tests**

Run: `cargo test -p blowup-core torrent::search::dedup --lib`
Expected: 7 tests pass.

- [ ] **Step 4: Check + commit**

Run: `just check`
Expected: green.

```bash
git add crates/core/src/torrent/search/dedup.rs crates/core/src/torrent/search/mod.rs
git commit -m "$(cat <<'EOF'
feat(search): add info_hash-based dedup for multi-source results

merge() groups raws by info_hash, taking max seeders/leechers and
filling in missing magnet/torrent_url/size from later entries. No
title fuzzy matching — only info_hash. Entries without hash pass
through as independent results.

Co-Authored-By: Claude Opus 4.6 (1M context) <noreply@anthropic.com>
EOF
)"
```

---

## Task 7: Provider trait + `CallPacer` + `with_retry` helper

**Goal:** Define the `SearchProvider` trait and the shared `CallPacer` / `with_retry` utilities used inside every provider impl.

**Files:**
- Create: `crates/core/src/torrent/search/provider.rs`
- Modify: `crates/core/src/torrent/search/mod.rs` (add `pub mod provider;`)

- [ ] **Step 1: Register module**

Append to `crates/core/src/torrent/search/mod.rs`:

```rust
pub mod provider;
```

- [ ] **Step 2: Write `provider.rs`**

```rust
//! Shared provider trait and per-task rate-limit / retry helpers.
//!
//! See spec §2.4-2.5.

use crate::torrent::search::types::{ProviderError, RawTorrent, SearchContext};
use async_trait::async_trait;
use std::future::Future;
use std::time::{Duration, Instant};

#[async_trait]
pub trait SearchProvider: Send + Sync {
    /// Stable short name used in logs and as `RawTorrent::source`.
    fn name(&self) -> &'static str;

    /// Minimum time between outgoing requests to this provider.
    /// Enforced within a single search task (see `CallPacer`).
    fn min_interval(&self) -> Duration;

    /// How many retry attempts on retryable errors (Timeout / Connect /
    /// 5xx / 429). Default 2.
    fn max_retries(&self) -> u32 {
        2
    }

    async fn search(&self, ctx: &SearchContext<'_>) -> Result<Vec<RawTorrent>, ProviderError>;
}

// ── CallPacer ──────────────────────────────────────────────────────

/// Per-task rate limiter. One instance per `search()` call — not
/// shared across tasks. Tracks the time of the last outgoing request
/// and, on the next call, sleeps just long enough to maintain
/// `min_interval` spacing.
///
/// Safe because it lives inside a single async task and is accessed
/// via `&mut self`; no Mutex needed.
pub struct CallPacer {
    min_interval: Duration,
    last: Option<Instant>,
}

impl CallPacer {
    pub fn new(min_interval: Duration) -> Self {
        Self {
            min_interval,
            last: None,
        }
    }

    pub async fn wait(&mut self) {
        if let Some(prev) = self.last {
            let elapsed = prev.elapsed();
            if elapsed < self.min_interval {
                tokio::time::sleep(self.min_interval - elapsed).await;
            }
        }
        self.last = Some(Instant::now());
    }
}

// ── with_retry helper ──────────────────────────────────────────────

/// Run `op` up to `max_retries + 1` times, with per-attempt pacing and
/// exponential backoff between retries. Only retryable errors trigger
/// another attempt; non-retryable (parse / 4xx) short-circuit out.
pub async fn with_retry<F, Fut, T>(
    pacer: &mut CallPacer,
    max_retries: u32,
    mut op: F,
) -> Result<T, ProviderError>
where
    F: FnMut() -> Fut,
    Fut: Future<Output = Result<T, ProviderError>>,
{
    let mut attempt: u32 = 0;
    loop {
        pacer.wait().await;
        match op().await {
            Ok(v) => return Ok(v),
            Err(e) if !e.is_retryable() => return Err(e),
            Err(e) if attempt >= max_retries => return Err(e),
            Err(e) => {
                tracing::warn!(attempt, error = %e, "provider call failed, retrying");
                let backoff = Duration::from_secs(2u64.pow(attempt).min(30));
                tokio::time::sleep(backoff).await;
                attempt += 1;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn pacer_first_call_no_wait() {
        let mut p = CallPacer::new(Duration::from_millis(100));
        let t = Instant::now();
        p.wait().await;
        assert!(t.elapsed() < Duration::from_millis(20));
    }

    #[tokio::test]
    async fn pacer_enforces_min_interval() {
        let mut p = CallPacer::new(Duration::from_millis(100));
        p.wait().await;
        let t = Instant::now();
        p.wait().await;
        assert!(t.elapsed() >= Duration::from_millis(95));
        assert!(t.elapsed() < Duration::from_millis(250));
    }

    #[tokio::test]
    async fn retry_returns_ok_on_first_success() {
        let mut pacer = CallPacer::new(Duration::from_millis(1));
        let result: Result<i32, _> = with_retry(&mut pacer, 2, || async { Ok(42) }).await;
        assert_eq!(result.unwrap(), 42);
    }

    #[tokio::test]
    async fn retry_short_circuits_on_non_retryable() {
        let mut pacer = CallPacer::new(Duration::from_millis(1));
        let mut calls = 0u32;
        let result: Result<i32, _> = with_retry(&mut pacer, 5, || {
            calls += 1;
            async { Err::<i32, _>(ProviderError::Http4xx(400)) }
        })
        .await;
        assert!(result.is_err());
        assert_eq!(calls, 1, "should not retry on 4xx");
    }

    #[tokio::test]
    async fn retry_exhausts_budget_on_retryable() {
        let mut pacer = CallPacer::new(Duration::from_millis(1));
        let mut calls = 0u32;
        let result: Result<i32, _> = with_retry(&mut pacer, 2, || {
            calls += 1;
            async { Err::<i32, _>(ProviderError::Timeout) }
        })
        .await;
        assert!(result.is_err());
        assert_eq!(calls, 3, "1 initial + 2 retries = 3 total attempts");
    }

    #[tokio::test]
    async fn retry_recovers_after_failure() {
        let mut pacer = CallPacer::new(Duration::from_millis(1));
        let mut calls = 0u32;
        let result: Result<i32, _> = with_retry(&mut pacer, 3, || {
            calls += 1;
            let should_succeed = calls >= 2;
            async move {
                if should_succeed {
                    Ok(99)
                } else {
                    Err(ProviderError::Timeout)
                }
            }
        })
        .await;
        assert_eq!(result.unwrap(), 99);
        assert_eq!(calls, 2);
    }
}
```

- [ ] **Step 3: Run tests**

Run: `cargo test -p blowup-core torrent::search::provider --lib`
Expected: 6 tests pass. Some tests have timing assertions — if the CI is extremely slow they may flake; relax the upper bound before narrowing the lower bound.

- [ ] **Step 4: Full check + commit**

```bash
just check
```

```bash
git add crates/core/src/torrent/search/provider.rs crates/core/src/torrent/search/mod.rs
git commit -m "$(cat <<'EOF'
feat(search): add SearchProvider trait, CallPacer, with_retry helper

Stateless trait for search sources. CallPacer is per-task (not shared
across searches), enforcing min_interval between outgoing requests.
with_retry exponential-backs-off retryable errors (Timeout / Connect /
5xx / 429) up to max_retries, short-circuits non-retryable (parse /
4xx) immediately. Tests exercise success, non-retryable short-circuit,
exhausted budget, and delayed recovery.

Co-Authored-By: Claude Opus 4.6 (1M context) <noreply@anthropic.com>
EOF
)"
```

---

## Task 8: Providers module scaffold + YtsProvider

**Goal:** Create `providers/` subdirectory, add the `YtsProvider` that reuses the existing YTS logic (from the old `search_yify`) but behind the new trait. At the end of this task, `YtsProvider::search` returns real results for real movies.

**Files:**
- Create: `crates/core/src/torrent/search/providers/mod.rs`
- Create: `crates/core/src/torrent/search/providers/yts.rs`
- Modify: `crates/core/src/torrent/search/mod.rs` (add `pub mod providers;`)

- [ ] **Step 1: Register providers module**

Append to `crates/core/src/torrent/search/mod.rs`:

```rust
pub mod providers;
```

- [ ] **Step 2: Create `providers/mod.rs`**

```rust
//! Concrete `SearchProvider` implementations.

pub mod yts;

use crate::torrent::search::provider::SearchProvider;
use std::sync::Arc;

/// Build the default provider set. Called once per search — providers
/// are stateless so construction is cheap.
pub fn build_default_providers(tmdb_api_key: String) -> Vec<Arc<dyn SearchProvider>> {
    vec![
        Arc::new(yts::YtsProvider::new(tmdb_api_key)),
        // nyaa and 1337x added in later tasks
    ]
}
```

- [ ] **Step 3: Create `providers/yts.rs`**

```rust
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

static MAGNET_HASH_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)urn:btih:([a-f0-9]+)").unwrap());

pub(crate) fn extract_info_hash_from_magnet(magnet: &str) -> Option<String> {
    MAGNET_HASH_RE
        .captures(magnet)
        .map(|c| c[1].to_lowercase())
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
```

- [ ] **Step 4: Run unit tests**

Run: `cargo test -p blowup-core torrent::search::providers::yts --lib`
Expected: 4 tests pass (live smoke test skipped via `#[ignore]`).

- [ ] **Step 5: Run live smoke test manually (optional sanity check)**

Run: `cargo test -p blowup-core --lib --ignored torrent::search::providers::yts::tests::yts_live_ -- --nocapture`
Expected: passes if YTS is reachable. If it fails with "no results" or network error, that is fine at this stage — the smoke test is a manual inspection tool, not a gate. Confirm the reason (network blocked? API moved?) before moving on.

- [ ] **Step 6: Full check + commit**

Run: `just check`
Expected: green. (Live smoke test is `#[ignore]` so it doesn't run.)

```bash
git add crates/core/src/torrent/search/providers/
git commit -m "$(cat <<'EOF'
feat(search): add YtsProvider under new SearchProvider trait

Ports the old search_yify strategy (imdb → title → sanitized-title)
into a stateless YtsProvider. Each strategy uses with_retry and the
per-task CallPacer. Now emits size_bytes (previously dropped),
lowercase info_hash extracted from magnet, and full debug tracing.
Old search_yify still lives in search/mod.rs — it will be deleted
after the orchestrator is wired up.

Co-Authored-By: Claude Opus 4.6 (1M context) <noreply@anthropic.com>
EOF
)"
```

---

## Task 9: NyaaProvider

**Goal:** Implement `NyaaProvider` — parses Nyaa.si RSS, synthesizes magnets from `info_hash` + the trackers slice in `SearchContext`.

**Files:**
- Create: `crates/core/src/torrent/search/providers/nyaa.rs`
- Create: `crates/core/src/torrent/search/providers/fixtures/nyaa_search.xml` (fixture)
- Modify: `crates/core/src/torrent/search/providers/mod.rs` (add module + include in builder)

- [ ] **Step 1: Register in `providers/mod.rs`**

Open `crates/core/src/torrent/search/providers/mod.rs`. Replace:

```rust
pub mod yts;
```

with:

```rust
pub mod nyaa;
pub mod yts;
```

And update `build_default_providers` to:

```rust
pub fn build_default_providers(tmdb_api_key: String) -> Vec<Arc<dyn SearchProvider>> {
    vec![
        Arc::new(yts::YtsProvider::new(tmdb_api_key)),
        Arc::new(nyaa::NyaaProvider::new()),
        // 1337x added in Task 10
    ]
}
```

- [ ] **Step 2: Create the fixture file**

Create `crates/core/src/torrent/search/providers/fixtures/nyaa_search.xml` with a minimal but realistic Nyaa RSS snippet:

```xml
<?xml version="1.0" encoding="UTF-8"?>
<rss version="2.0" xmlns:nyaa="https://nyaa.si/xmlns/nyaa">
  <channel>
    <title>Nyaa - "blow up" - Torrent File RSS</title>
    <description>RSS Feed for "blow up"</description>
    <link>https://nyaa.si/?page=rss&amp;q=blow+up&amp;c=0_0&amp;f=0</link>
    <item>
      <title>Blow.Up.1966.1080p.BluRay.x265-FraMeSToR</title>
      <link>https://nyaa.si/download/1234567.torrent</link>
      <guid isPermaLink="true">https://nyaa.si/view/1234567</guid>
      <pubDate>Sat, 12 Apr 2025 15:23:11 -0000</pubDate>
      <nyaa:seeders>127</nyaa:seeders>
      <nyaa:leechers>8</nyaa:leechers>
      <nyaa:downloads>842</nyaa:downloads>
      <nyaa:infoHash>aabbccddeeff0011223344556677889900aabbcc</nyaa:infoHash>
      <nyaa:categoryId>4_2</nyaa:categoryId>
      <nyaa:category>Live Action - English-translated</nyaa:category>
      <nyaa:size>4.2 GiB</nyaa:size>
      <nyaa:comments>3</nyaa:comments>
      <nyaa:trusted>No</nyaa:trusted>
      <nyaa:remake>No</nyaa:remake>
    </item>
    <item>
      <title>[FansubGroup] Blow Up (1966) [720p][x264][Chinese Sub]</title>
      <link>https://nyaa.si/download/2345678.torrent</link>
      <guid isPermaLink="true">https://nyaa.si/view/2345678</guid>
      <pubDate>Mon, 05 Mar 2024 08:00:00 -0000</pubDate>
      <nyaa:seeders>43</nyaa:seeders>
      <nyaa:leechers>2</nyaa:leechers>
      <nyaa:downloads>128</nyaa:downloads>
      <nyaa:infoHash>1122334455667788990011223344556677889900</nyaa:infoHash>
      <nyaa:categoryId>4_2</nyaa:categoryId>
      <nyaa:category>Live Action - English-translated</nyaa:category>
      <nyaa:size>850 MiB</nyaa:size>
      <nyaa:comments>0</nyaa:comments>
      <nyaa:trusted>No</nyaa:trusted>
      <nyaa:remake>No</nyaa:remake>
    </item>
  </channel>
</rss>
```

- [ ] **Step 3: Create `providers/nyaa.rs`**

```rust
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
                if r.magnet.is_none() {
                    if let Some(h) = &r.info_hash {
                        r.magnet = Some(make_magnet(h, &r.raw_title, ctx.trackers));
                    }
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
        assert_eq!(first.size_bytes, Some((4.2 * 1024.0 * 1024.0 * 1024.0) as u64));
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
        assert_eq!(parse_size_human("4.2 GiB"), Some((4.2 * 1024.0f64.powi(3)) as u64));
        assert_eq!(parse_size_human("850 MiB"), Some(850 * 1024 * 1024));
        assert_eq!(parse_size_human("1.3 TiB"), Some((1.3 * 1024.0f64.powi(4)) as u64));
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
```

- [ ] **Step 4: Run tests**

Run: `cargo test -p blowup-core torrent::search::providers::nyaa --lib`
Expected: 4 tests pass (live test skipped).

- [ ] **Step 5: Full check + commit**

```bash
just check
```

```bash
git add crates/core/src/torrent/search/providers/
git commit -m "$(cat <<'EOF'
feat(search): add NyaaProvider with fixture parse test

RSS-based (Nyaa has no JSON API). Parses <item> + nyaa:* extensions
via quick-xml streaming. Human size strings ("4.2 GiB") decoded to
bytes. Magnets synthesized from info_hash + TrackerManager trackers
at the provider layer (Nyaa does not ship magnets itself). Live smoke
test under #[ignore] flag.

Co-Authored-By: Claude Opus 4.6 (1M context) <noreply@anthropic.com>
EOF
)"
```

---

## Task 10: OnethreesevenProvider (1337x HTML scraper)

**Goal:** Implement the two-stage 1337x scraper (search listing → top-5 detail pages concurrently). Includes two fixture files.

**Files:**
- Create: `crates/core/src/torrent/search/providers/onethreeseven.rs`
- Create: `crates/core/src/torrent/search/providers/fixtures/1337x_search.html`
- Create: `crates/core/src/torrent/search/providers/fixtures/1337x_detail.html`
- Modify: `crates/core/src/torrent/search/providers/mod.rs`

- [ ] **Step 1: Register in `providers/mod.rs`**

Update `providers/mod.rs`:

```rust
pub mod nyaa;
pub mod onethreeseven;
pub mod yts;

use crate::torrent::search::provider::SearchProvider;
use std::sync::Arc;

pub fn build_default_providers(tmdb_api_key: String) -> Vec<Arc<dyn SearchProvider>> {
    vec![
        Arc::new(yts::YtsProvider::new(tmdb_api_key)),
        Arc::new(nyaa::NyaaProvider::new()),
        Arc::new(onethreeseven::OnethreesevenProvider::new()),
    ]
}
```

- [ ] **Step 2: Create `fixtures/1337x_search.html`**

This fixture is a minimal snippet of the 1337x search result table — just enough rows for the parser to exercise its selectors. Real 1337x pages have dozens of rows, nav chrome, and ads; we only keep the table structure.

```html
<!DOCTYPE html>
<html>
<body>
<div class="box-info-heading clearfix"><h1>Search For "blow up"</h1></div>
<div class="table-list-wrap">
  <table class="table-list table table-responsive table-striped">
    <thead>
      <tr>
        <th class="coll-1 name">Name</th>
        <th class="coll-2">Seeders</th>
        <th class="coll-3">Leechers</th>
        <th class="coll-date">Time</th>
        <th class="coll-4">Size</th>
        <th class="coll-5">Uploader</th>
      </tr>
    </thead>
    <tbody>
      <tr>
        <td class="coll-1 name">
          <a href="/sub/54/0/" class="icon"><i class="flaticon-movie"></i></a>
          <a href="/torrent/1000001/Blow-Up-1966-1080p-BluRay-x265-FraMeSToR/">Blow.Up.1966.1080p.BluRay.x265-FraMeSToR</a>
        </td>
        <td class="coll-2 seeds">214</td>
        <td class="coll-3 leeches">18</td>
        <td class="coll-date">Apr. 12th '25</td>
        <td class="coll-4 size mob-uploader">12.3 GB<span class="seeds">214</span></td>
        <td class="coll-5 uploader"><a href="/user/anon/">anon</a></td>
      </tr>
      <tr>
        <td class="coll-1 name">
          <a href="/sub/54/0/" class="icon"><i class="flaticon-movie"></i></a>
          <a href="/torrent/1000002/Blow-Up-1966-720p-BluRay-x264-AMIABLE/">Blow.Up.1966.720p.BluRay.x264-AMIABLE</a>
        </td>
        <td class="coll-2 seeds">87</td>
        <td class="coll-3 leeches">5</td>
        <td class="coll-date">Mar. 5th '24</td>
        <td class="coll-4 size mob-uploader">4.5 GB<span class="seeds">87</span></td>
        <td class="coll-5 uploader"><a href="/user/anon/">anon</a></td>
      </tr>
      <tr>
        <td class="coll-1 name">
          <a href="/sub/54/0/" class="icon"><i class="flaticon-movie"></i></a>
          <a href="/torrent/1000003/Blow-Up-1966-WEBRip-x265-PSA/">Blow.Up.1966.1080p.WEBRip.x265-PSA</a>
        </td>
        <td class="coll-2 seeds">32</td>
        <td class="coll-3 leeches">2</td>
        <td class="coll-date">Feb. 1st '24</td>
        <td class="coll-4 size mob-uploader">2.1 GB<span class="seeds">32</span></td>
        <td class="coll-5 uploader"><a href="/user/psa/">psa</a></td>
      </tr>
    </tbody>
  </table>
</div>
</body>
</html>
```

- [ ] **Step 3: Create `fixtures/1337x_detail.html`**

```html
<!DOCTYPE html>
<html>
<body>
<div class="box-info-heading clearfix"><h1>Blow.Up.1966.1080p.BluRay.x265-FraMeSToR</h1></div>
<div class="torrent-detail-page">
  <ul class="download-links-dontblock">
    <li><a href="/torrent/1000001/Blow-Up.torrent">TORRENT DOWNLOAD</a></li>
    <li><a href="magnet:?xt=urn:btih:AABBCCDDEEFF0011223344556677889900AABBCC&dn=Blow.Up.1966.1080p.BluRay.x265-FraMeSToR&tr=udp%3A%2F%2Ftracker.opentrackr.org%3A1337">MAGNET DOWNLOAD</a></li>
  </ul>
  <ul class="list">
    <li>Category: <span>Movies</span></li>
    <li>Size: <span>12.3 GB</span></li>
    <li>Seeders: <span class="seeds">214</span></li>
    <li>Leechers: <span class="leeches">18</span></li>
  </ul>
</div>
</body>
</html>
```

- [ ] **Step 4: Create `providers/onethreeseven.rs`**

```rust
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

use crate::torrent::search::provider::{CallPacer, SearchProvider, with_retry};
use crate::torrent::search::providers::yts::extract_info_hash_from_magnet;
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
        let mut stage1 =
            with_retry(&mut pacer, self.max_retries(), || fetch_search(ctx.http, &query)).await?;
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
                            row.info_hash =
                                extract_info_hash_from_magnet(&magnet);
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
struct SearchRow {
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
    let url = format!(
        "https://1337x.to/search/{}/1/",
        urlencoding::encode(query)
    );
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
    let resp = http.get(url).header("User-Agent", USER_AGENT).send().await?;
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
        return Err("selector ul.download-links-dontblock a[href^='magnet:'] matched 0".to_string());
    };
    let href = anchor.value().attr("href").ok_or("magnet anchor has no href")?;
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
        assert!(top.detail_url.starts_with("https://1337x.to/torrent/1000001/"));
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
```

- [ ] **Step 5: Run unit tests**

Run: `cargo test -p blowup-core torrent::search::providers::onethreeseven --lib`
Expected: 4 tests pass (live test skipped).

- [ ] **Step 6: Full check + commit**

```bash
just check
```

```bash
git add crates/core/src/torrent/search/providers/
git commit -m "$(cat <<'EOF'
feat(search): add 1337x two-stage HTML scraper provider

Stage 1 parses the search results table (3 rows in fixture) via
scraper crate selectors; Stage 2 concurrently fetches detail pages
for the top 5 seeders and extracts the magnet anchor. Parse failures
return ProviderError::Parse with the matching selector in the message
for fast diagnosis when 1337x changes its HTML. Realistic browser
User-Agent; no Cloudflare bypass. Live smoke test under #[ignore].

Co-Authored-By: Claude Opus 4.6 (1M context) <noreply@anthropic.com>
EOF
)"
```

---

## Task 11: Orchestrator — `search_movie()` in `search/mod.rs`

**Goal:** Replace the old `search_yify` in `search/mod.rs` with the new orchestrator that fans out to all providers, dedupes, scores, and sorts. Includes a test with mock providers.

**Files:**
- Modify: `crates/core/src/torrent/search/mod.rs` (delete old `search_yify` + `MovieResult`, add `search_movie` and the `assemble_scored` helper)
- Modify: `crates/core/src/torrent/mod.rs` (drop `pub use search::MovieResult;` — replaced by `ScoredTorrent`)

- [ ] **Step 1: Replace `search/mod.rs` contents**

Open `crates/core/src/torrent/search/mod.rs`. Current contents (from Task 1) are the old `search_yify` + `MovieResult` + helpers. Replace the ENTIRE file with:

```rust
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
            let ctx = ctx;
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
        let b = raw("nyaa", "Blow.Up.1966.1080p.BluRay.x265-FraMeSToR", "abc", 80);
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
        let usable = raw(
            "yts",
            "Blow.Up.1966.1080p.BluRay.x265-FraMeSToR",
            "abc",
            50,
        );
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
```

- [ ] **Step 2: Drop `MovieResult` from `torrent/mod.rs`**

Open `crates/core/src/torrent/mod.rs`. The current last section is:

```rust
pub use search::MovieResult;
```

Delete that line (`MovieResult` no longer exists in `search`). Leave the other `pub use` statements alone.

- [ ] **Step 3: Fix compilation of downstream callers that still reference `search_yify` / `MovieResult`**

At this point the build is broken. Two files need a quick shim:

`crates/tauri/src/commands/search.rs` — currently calls `search::search_yify` and returns `Vec<MovieResult>`. Update to the new function (rename will be finalized in Task 12; here we just make it compile):

```rust
use blowup_core::AppContext;
use blowup_core::torrent::search::{search_movie, types::{ScoredTorrent, SearchQuery}};
use std::sync::Arc;
use tauri::State;

#[tauri::command]
pub async fn search_yify_cmd(
    query: String,
    year: Option<u32>,
    tmdb_id: Option<u64>,
    ctx: State<'_, Arc<AppContext>>,
) -> Result<Vec<ScoredTorrent>, String> {
    let cfg = blowup_core::config::load_config();
    let q = SearchQuery {
        title: query,
        year,
        tmdb_id,
        tmdb_api_key: cfg.tmdb.api_key,
    };
    Ok(search_movie(&ctx.http, &ctx.tracker, q).await)
}
```

(The command name stays `search_yify_cmd` for one more task so the frontend keeps working; renaming + frontend edits happen in Task 12-13.)

`crates/server/src/routes/search.rs` — similarly port to the new function:

```rust
use axum::{Json, Router, routing::post};
use blowup_core::torrent::search::{search_movie, types::{ScoredTorrent, SearchQuery}};
use serde::Deserialize;

use crate::error::ApiResult;
use crate::state::AppState;

pub fn router() -> Router<AppState> {
    Router::new().route("/search/yify", post(search_yify))
}

#[derive(Deserialize)]
pub struct YifySearchRequest {
    pub query: String,
    pub year: Option<u32>,
    pub tmdb_id: Option<u64>,
}

async fn search_yify(
    axum::extract::State(state): axum::extract::State<AppState>,
    Json(req): Json<YifySearchRequest>,
) -> ApiResult<Json<Vec<ScoredTorrent>>> {
    let cfg = blowup_core::config::load_config();
    let q = SearchQuery {
        title: req.query,
        year: req.year,
        tmdb_id: req.tmdb_id,
        tmdb_api_key: cfg.tmdb.api_key,
    };
    Ok(Json(search_movie(&state.http, &state.tracker, q).await))
}
```

- [ ] **Step 4: Frontend `MovieResult` compile safety net**

Open `src/lib/tauri.ts`. The old `MovieResult` TS interface is still there and `FilmDetailPanel.tsx` uses it. The response shape changed (`ScoredTorrent` has more fields but also has `quality` as `null` now — wait, actually `ScoredTorrent` has NO `quality` field). To keep the frontend temporarily functional with the renamed command, add `quality` as a computed field on the frontend side in Task 13.

For Task 11 just keep `MovieResult` as-is in `tauri.ts`. TypeScript won't notice the underlying shape changed because the command returns unchecked JSON. The frontend will render missing fields as `undefined`, which is visible but not crashing. Actual frontend changes land in Task 13.

- [ ] **Step 5: Build + test**

Run: `cargo build -p blowup-core -p blowup-tauri -p blowup-server`
Expected: green build across all three crates.

Run: `cargo test -p blowup-core torrent::search --lib`
Expected: all search-module tests pass (parser / scorer / dedup / provider / providers/* / mod tests).

Run: `just check`
Expected: full workspace green.

- [ ] **Step 6: Commit**

```bash
git add crates/core/src/torrent/search/mod.rs \
        crates/core/src/torrent/mod.rs \
        crates/tauri/src/commands/search.rs \
        crates/server/src/routes/search.rs
git commit -m "$(cat <<'EOF'
feat(search): wire search_movie orchestrator, remove old search_yify

Replaces the old YTS-only search.rs contents with a fan-out
orchestrator that calls every registered provider concurrently,
silently drops failures, dedupes by info_hash, parses titles, scores,
and sorts. Tauri command and server route temporarily keep their
old path/name (search_yify_cmd / /search/yify) so the frontend keeps
working; Task 12 renames them and Task 13 updates the UI.

Co-Authored-By: Claude Opus 4.6 (1M context) <noreply@anthropic.com>
EOF
)"
```

---

## Task 12: Rename Tauri command and server route to `search_movie` / `/search/movie`

**Goal:** Final rename of the public surface. Delete the `search_yify_cmd` name + `/search/yify` path. Frontend migration starts in Task 13.

**Files:**
- Modify: `crates/tauri/src/commands/search.rs` (rename fn)
- Modify: `crates/tauri/src/lib.rs` (update command registration)
- Modify: `crates/server/src/routes/search.rs` (rename route)
- Modify: `crates/server/tests/smoke.rs` (update any references to `/search/yify` — grep first)

- [ ] **Step 1: Check for existing references in tests**

Run: `git grep -n "search_yify\|/search/yify" crates/`
Expected: references in:
- `crates/tauri/src/commands/search.rs`
- `crates/tauri/src/lib.rs`
- `crates/server/src/routes/search.rs`
- possibly `crates/server/tests/smoke.rs`

Note each match — every one needs updating.

- [ ] **Step 2: Rename Tauri command**

Open `crates/tauri/src/commands/search.rs`. Rename `search_yify_cmd` → `search_movie_cmd`:

```rust
use blowup_core::AppContext;
use blowup_core::torrent::search::{search_movie, types::{ScoredTorrent, SearchQuery}};
use std::sync::Arc;
use tauri::State;

#[tauri::command]
pub async fn search_movie_cmd(
    query: String,
    year: Option<u32>,
    tmdb_id: Option<u64>,
    ctx: State<'_, Arc<AppContext>>,
) -> Result<Vec<ScoredTorrent>, String> {
    let cfg = blowup_core::config::load_config();
    let q = SearchQuery {
        title: query,
        year,
        tmdb_id,
        tmdb_api_key: cfg.tmdb.api_key,
    };
    Ok(search_movie(&ctx.http, &ctx.tracker, q).await)
}
```

- [ ] **Step 3: Update the command registry**

Open `crates/tauri/src/lib.rs`. Find the line:

```rust
commands::search::search_yify_cmd,
```

Replace with:

```rust
commands::search::search_movie_cmd,
```

- [ ] **Step 4: Rename server route**

Open `crates/server/src/routes/search.rs`:

```rust
use axum::{Json, Router, routing::post};
use blowup_core::torrent::search::{search_movie, types::{ScoredTorrent, SearchQuery}};
use serde::Deserialize;

use crate::error::ApiResult;
use crate::state::AppState;

pub fn router() -> Router<AppState> {
    Router::new().route("/search/movie", post(search_movie_route))
}

#[derive(Deserialize)]
pub struct MovieSearchRequest {
    pub query: String,
    pub year: Option<u32>,
    pub tmdb_id: Option<u64>,
}

async fn search_movie_route(
    axum::extract::State(state): axum::extract::State<AppState>,
    Json(req): Json<MovieSearchRequest>,
) -> ApiResult<Json<Vec<ScoredTorrent>>> {
    let cfg = blowup_core::config::load_config();
    let q = SearchQuery {
        title: req.query,
        year: req.year,
        tmdb_id: req.tmdb_id,
        tmdb_api_key: cfg.tmdb.api_key,
    };
    Ok(Json(search_movie(&state.http, &state.tracker, q).await))
}
```

- [ ] **Step 5: Update smoke test references if needed**

If `git grep` in Step 1 showed anything in `crates/server/tests/smoke.rs` referencing `/search/yify`, update the path to `/search/movie` in the test. If no match, skip this step.

- [ ] **Step 6: Build + test**

Run: `just check`
Expected: green.

At this point the frontend still calls `search_yify_cmd` and will fail at runtime (`"unknown command"`). That's expected — the next task fixes the frontend.

- [ ] **Step 7: Commit**

```bash
git add crates/tauri/src/commands/search.rs \
        crates/tauri/src/lib.rs \
        crates/server/src/routes/search.rs \
        crates/server/tests/smoke.rs
git commit -m "$(cat <<'EOF'
refactor(search): rename command/route to search_movie / /search/movie

Finalizes the public surface rename. Frontend still calls the old
name — fixed in the next task.

Co-Authored-By: Claude Opus 4.6 (1M context) <noreply@anthropic.com>
EOF
)"
```

---

## Task 13: Frontend migration — new TS types + UI

**Goal:** Update `src/lib/tauri.ts` (types + invoke wrapper) and `FilmDetailPanel.tsx` (new result row layout with score badge + expandable breakdown).

**Files:**
- Modify: `src/lib/tauri.ts`
- Modify: `src/components/FilmDetailPanel.tsx`

- [ ] **Step 1: Update `src/lib/tauri.ts`**

Open `src/lib/tauri.ts`. Find the `MovieResult` interface (around line 214) and **replace** it with the new types. Also rename the `yts` wrapper.

Near the top (after `SubtitleStyle` or wherever the other shared types live), add:

```ts
// ── Torrent search (multi-source) ─────────────────────────────────
export type Resolution =
  | "unknown" | "sd" | "p480" | "p720" | "p1080" | "p2160";
export type SourceKind =
  | "unknown" | "cam" | "ts" | "hdtv" | "webrip" | "webdl" | "bluray" | "remux";
export type Codec = "unknown" | "x264" | "x265" | "av1";

export interface ScoreBreakdown {
  seeders: number;
  resolution: number;
  source: number;
  codec: number;
  size: number;
  group: number;
  hdr: number;
}

export interface ScoredTorrent {
  source: string;              // "yts" | "1337x" | "nyaa"
  raw_title: string;
  info_hash: string | null;
  magnet: string | null;
  torrent_url: string | null;
  size_bytes: number | null;
  seeders: number;
  leechers: number;
  resolution: Resolution;
  source_kind: SourceKind;
  codec: Codec;
  release_group: string | null;
  hdr: boolean;
  score: number;
  breakdown: ScoreBreakdown;
}
```

Delete the old `MovieResult` interface (the one with fields `title / year / quality / magnet / torrent_url / seeds`).

Replace the `yts` export block at the bottom:

```ts
// Old:
// export const yts = {
//   search: (query: string, year?: number, tmdbId?: number) =>
//     invoke<MovieResult[]>("search_yify_cmd", { query, year, tmdbId }),
// };

// New:
export const search = {
  movie: (query: string, year?: number, tmdbId?: number) =>
    invoke<ScoredTorrent[]>("search_movie_cmd", { query, year, tmdbId }),
};
```

- [ ] **Step 2: Remove `search.rate_limit_secs` from AppConfig**

In the same file, find the `AppConfig` interface and delete the `search` field line:

```ts
// DELETE:
// search: { rate_limit_secs: number };
```

No replacement needed — the config section is gone entirely.

- [ ] **Step 3: Rewrite `FilmDetailPanel.tsx` search modal**

Open `src/components/FilmDetailPanel.tsx`. The current imports from `../lib/tauri`:

```ts
import { yts, download } from "../lib/tauri";
import type { MovieListItem, MovieResult, TorrentFileInfo } from "../lib/tauri";
```

Change to:

```ts
import { search, download } from "../lib/tauri";
import type {
  MovieListItem,
  ScoredTorrent,
  TorrentFileInfo,
} from "../lib/tauri";
```

Replace all usages of `MovieResult` type and variable names with `ScoredTorrent`. Specifically:

```ts
const [results, setResults] = useState<ScoredTorrent[]>([]);
const [filePickResult, setFilePickResult] = useState<ScoredTorrent | null>(null);
```

Replace the `yts.search(...)` call with:

```ts
useEffect(() => {
  if (!opened) return;
  setLoading(true);
  setError("");
  search
    .movie(film.title, year, film.id)
    .then(setResults)
    .catch((e) => setError(String(e)))
    .finally(() => setLoading(false));
}, [opened, film.title, year, film.id]);
```

Update `handleFetchFiles` signature:

```ts
const handleFetchFiles = async (r: ScoredTorrent) => { ... };
```

Add label helpers near the top of the file (after imports):

```ts
const resolutionLabel = (r: ScoredTorrent["resolution"]): string =>
  ({
    p2160: "4K",
    p1080: "1080p",
    p720: "720p",
    p480: "480p",
    sd: "SD",
    unknown: "",
  }[r] ?? "");

const sourceLabel = (s: ScoredTorrent["source_kind"]): string =>
  ({
    remux: "Remux",
    bluray: "Bluray",
    webdl: "WEB-DL",
    webrip: "WEBRip",
    hdtv: "HDTV",
    ts: "TS",
    cam: "CAM",
    unknown: "",
  }[s] ?? "");

const codecLabel = (c: ScoredTorrent["codec"]): string =>
  ({ x265: "x265", x264: "x264", av1: "AV1", unknown: "" }[c] ?? "");

const qualityTags = (r: ScoredTorrent): string =>
  [
    resolutionLabel(r.resolution),
    sourceLabel(r.source_kind),
    codecLabel(r.codec),
    r.hdr ? "HDR" : "",
  ]
    .filter(Boolean)
    .join(" · ");
```

Replace the results rendering block in `TorrentSearchModal` (the `{results.map((r, i) => ...)}` section) with:

```tsx
<Stack gap={0}>
  {results.map((r, i) => {
    const target = r.magnet ?? r.torrent_url ?? "";
    const isStarted = started.has(target);
    const showDetail = detailIndex === i;
    return (
      <Box
        key={i}
        py="8px"
        style={{ borderBottom: "1px solid var(--color-separator)" }}
      >
        <Group justify="space-between" wrap="nowrap" gap="md">
          <Group gap="sm" wrap="nowrap" style={{ flex: 1, minWidth: 0 }}>
            <Text size="sm" fw={600} c="var(--color-accent)">
              ⭐ {r.score}
            </Text>
            <Text size="sm" truncate>
              {qualityTags(r)}
            </Text>
            {r.size_bytes != null && (
              <Text size="xs" c="var(--color-label-secondary)">
                {formatSize(r.size_bytes)}
              </Text>
            )}
            <Text size="xs" c="var(--color-label-secondary)">
              ▸ {r.seeders} seeds
            </Text>
          </Group>
          <Group gap="xs" wrap="nowrap" style={{ flexShrink: 0 }}>
            <Button
              size="compact-xs"
              variant="default"
              onClick={() => setDetailIndex(showDetail ? null : i)}
            >
              {showDetail ? "收起" : "详情"}
            </Button>
            {isStarted ? (
              <Text size="xs" c="var(--color-accent)">
                下载中
              </Text>
            ) : fetching.has(target) ? (
              <Loader size="xs" />
            ) : (
              <Button
                size="compact-xs"
                disabled={!target}
                onClick={() => handleFetchFiles(r)}
              >
                下载
              </Button>
            )}
          </Group>
        </Group>
        <Text size="xs" c="var(--color-label-tertiary)" truncate mt={4}>
          [{r.source}] {r.raw_title}
        </Text>
        {showDetail && (
          <Box
            mt="xs"
            p="xs"
            style={{
              background: "var(--color-surface-2)",
              border: "1px solid var(--color-separator)",
              fontFamily: "monospace",
              fontSize: 11,
            }}
          >
            {renderBreakdownLine("seeders", r.breakdown.seeders, `${r.seeders} peers`)}
            {renderBreakdownLine(
              "resolution",
              r.breakdown.resolution,
              resolutionLabel(r.resolution) || "unknown",
            )}
            {renderBreakdownLine(
              "source",
              r.breakdown.source,
              sourceLabel(r.source_kind) || "unknown",
            )}
            {renderBreakdownLine(
              "codec",
              r.breakdown.codec,
              codecLabel(r.codec) || "unknown",
            )}
            {renderBreakdownLine(
              "size",
              r.breakdown.size,
              r.size_bytes != null ? formatSize(r.size_bytes) : "—",
            )}
            {renderBreakdownLine(
              "group",
              r.breakdown.group,
              r.release_group ?? "—",
            )}
            {renderBreakdownLine("hdr", r.breakdown.hdr, r.hdr ? "yes" : "no")}
            <div
              style={{
                marginTop: 4,
                paddingTop: 4,
                borderTop: "1px solid var(--color-separator)",
              }}
            >
              {renderBreakdownLine("total", r.score, "")}
            </div>
          </Box>
        )}
      </Box>
    );
  })}
</Stack>
```

Add the state declaration near the top of `TorrentSearchModal`:

```ts
const [detailIndex, setDetailIndex] = useState<number | null>(null);
```

Add the helper function (above the component, or inside it before the return):

```ts
function renderBreakdownLine(label: string, value: number, note: string) {
  const sign = value >= 0 ? "+" : "";
  return (
    <div style={{ display: "flex", justifyContent: "space-between" }}>
      <span>{label}</span>
      <span>
        {sign}
        {value}
        {note && (
          <span style={{ marginLeft: 8, color: "var(--color-label-tertiary)" }}>
            ({note})
          </span>
        )}
      </span>
    </div>
  );
}
```

Also update the small file-picker modal preview text which currently references `filePickResult.quality`:

```tsx
<Text size="xs" c="var(--color-label-secondary)" mb="md">
  {filePickResult ? qualityTags(filePickResult) : ""} · 共 {fileList.length} 个文件
</Text>
```

And the `startDownload` call — `r.quality` is gone, pass a derived label instead:

```tsx
await download.startDownload({
  title: film.title,
  target,
  director: film.director,
  tmdbId: film.id,
  year: year,
  genres: [],
  quality: resolutionLabel(filePickResult.resolution), // was filePickResult.quality
  onlyFiles: [...selectedFiles],
});
```

- [ ] **Step 4: Update the loading text**

In the same file, change:

```tsx
<Text size="sm" c="var(--color-label-secondary)">
  搜索中...
</Text>
```

to:

```tsx
<Text size="sm" c="var(--color-label-secondary)">
  搜索中... (YTS · Nyaa · 1337x)
</Text>
```

- [ ] **Step 5: Type-check the frontend**

Run: `bun run typecheck`
(If `typecheck` is not a script, run `bunx tsc --noEmit`.)
Expected: zero errors.

- [ ] **Step 6: Lint**

Run: `bun run lint`
Expected: zero errors.

- [ ] **Step 7: Full workspace check**

Run: `just check`
Expected: green.

- [ ] **Step 8: Manual smoke test — dev run**

Run: `just dev`
Then in the app:
1. Open a film detail panel (Library / Search page).
2. Click the torrent search action.
3. Observe the modal — should show "搜索中... (YTS · Nyaa · 1337x)".
4. Wait ~5-15s for results.
5. Verify each row shows: ⭐ score, quality tags, size, seed count, `[source]` + raw_title, "详情" button.
6. Click "详情" on one row → accordion with 7 breakdown lines should appear.
7. Click "下载" on a real magnet → existing file picker should still work.
8. Watch `just dev`'s stderr for `tracing` lines — should see `provider finished` and `search_movie complete` with timings.

If any step fails, fix before committing. Don't commit broken UI.

- [ ] **Step 9: Commit**

```bash
git add src/lib/tauri.ts src/components/FilmDetailPanel.tsx
git commit -m "$(cat <<'EOF'
feat(frontend): multi-source torrent search UI with score breakdown

New ScoredTorrent type with 7-dimension score breakdown; expandable
per-row "详情" panel shows each component. Source tag + raw release
title under the main row. Loading state hints at the three sources
being queried. Wrapper renamed from yts.search to search.movie.

Co-Authored-By: Claude Opus 4.6 (1M context) <noreply@anthropic.com>
EOF
)"
```

---

## Task 14: Remove dead `SearchConfig` + Settings UI + SearchError

**Goal:** Delete the `SearchConfig` struct, the `search` section from `Config`, the Settings UI input for `rate_limit_secs`, and the now-unused `SearchError` enum.

**Files:**
- Modify: `crates/core/src/config/mod.rs`
- Modify: `crates/core/src/error.rs`
- Modify: `src/pages/Settings.tsx`

- [ ] **Step 1: Remove `SearchConfig`**

Open `crates/core/src/config/mod.rs`.

1. Delete the `pub search: SearchConfig,` line inside `pub struct Config`.
2. Delete the entire `SearchConfig` struct + its `Default` impl + `default_rate_limit()` function.
3. Delete any test that asserts `cfg.search.rate_limit_secs` (search the file; there are two — lines ~309 and ~328 per the earlier grep).

- [ ] **Step 2: Remove `SearchError`**

Open `crates/core/src/error.rs`. Delete the entire `pub enum SearchError` + its `impl` and the `search_error_display` test.

Run: `git grep -n "SearchError" crates/`
Expected: zero matches. If any remain, update them (probably in the old `search.rs` archive if we somehow missed it, or in `error.rs` itself).

- [ ] **Step 3: Verify core still builds**

Run: `cargo build -p blowup-core`
Expected: green.

- [ ] **Step 4: Remove the Settings UI block**

Open `src/pages/Settings.tsx`. Find the section around line 320 that renders an input bound to `c.search.rate_limit_secs`:

```tsx
defaultValue={cfg.search.rate_limit_secs}
// ...
c.search.rate_limit_secs = v;
```

Delete the entire input + its surrounding label/wrapper. If this was inside a "搜索" section that had only this one field, delete the section heading too.

- [ ] **Step 5: Verify frontend still type-checks**

Run: `bun run typecheck` (or `bunx tsc --noEmit`)
Expected: zero errors. The `AppConfig.search` field was already removed in Task 13, so this should be consistent.

- [ ] **Step 6: Full workspace check**

Run: `just check`
Expected: green.

- [ ] **Step 7: Commit**

```bash
git add crates/core/src/config/mod.rs \
        crates/core/src/error.rs \
        src/pages/Settings.tsx
git commit -m "$(cat <<'EOF'
chore(config): remove dead SearchConfig + SearchError + Settings UI

rate_limit_secs was a dead field — never consumed by any code.
Removed from config struct (serde ignores it on load so existing
config.toml files remain valid), from Settings UI, and from the
TS AppConfig interface. SearchError is likewise replaced by
ProviderError inside the new search pipeline.

Co-Authored-By: Claude Opus 4.6 (1M context) <noreply@anthropic.com>
EOF
)"
```

---

## Task 15: End-to-end verification

**Goal:** Final sanity pass across the whole refactor. No code changes unless a problem surfaces.

- [ ] **Step 1: Full workspace check**

Run: `just check`
Expected: all green — lint + typecheck + clippy + fmt + tests.

- [ ] **Step 2: Full workspace test including ignored smoke tests**

Run: `cargo test -p blowup-core --lib`
Expected: all regular tests pass.

(Optional) Run live smoke tests manually if network is available:
```bash
cargo test -p blowup-core --lib --ignored torrent::search::providers:: -- --nocapture
```
Expected: each provider returns ≥ 1 result for "The Matrix". If a provider fails (site changed, Cloudflare up), document it in the PR description — the design specifies that failures are silently dropped, so production behavior is still fine.

- [ ] **Step 3: Manual desktop run**

Run: `just dev`

Exercise the full flow:
1. Start the app.
2. Open a popular film detail panel (e.g., search TMDB for "The Matrix").
3. Click search resources.
4. Verify the modal opens with "搜索中... (YTS · Nyaa · 1337x)".
5. Wait for results (5-15s).
6. Verify the top result has a plausible score and quality tags.
7. Click "详情" on two different rows → breakdown panels open independently.
8. Click "下载" on a real magnet → the existing file-picker flow still works.
9. Cross-check the terminal output for `tracing::info` lines starting with `search_movie`: look for "search_movie started" → "search_movie complete" with an `elapsed_ms` under 20_000.

Also test:
- A Chinese film (e.g., "春夏秋冬又一春" / "In the Mood for Love") — verify Nyaa returns results where YTS returned nothing.
- A film that probably has NO results (e.g., some obscure short film) — verify "未找到资源" shows cleanly.

- [ ] **Step 4: Cleanup check**

Run: `git grep -n "search_yify\|MovieResult\|rate_limit_secs\|SearchError" -- crates/ src/`
Expected: zero matches. Any match is a leftover reference — investigate and fix before committing.

- [ ] **Step 5: Final commit (if anything changed) or log completion**

If Steps 3/4 surfaced fixes, commit them:

```bash
git add -A
git commit -m "$(cat <<'EOF'
chore(search): final cleanup from e2e verification

Co-Authored-By: Claude Opus 4.6 (1M context) <noreply@anthropic.com>
EOF
)"
```

Otherwise, log the verification in the PR description and end the task here.

---

## Self-review checklist (read before merging)

- [ ] Every spec section (§1 motivation → §14 out of scope) has a corresponding task or is explicitly deferred.
- [ ] `SearchContext.trackers` is sourced from `TrackerManager::hot_trackers()` exactly once per search (Task 11).
- [ ] Every provider uses `with_retry` + `CallPacer` with its own `min_interval`.
- [ ] No task leaves the workspace in a broken state — each commit builds and passes tests.
- [ ] The release group regex accepts mixed case (`NTb` test in Task 4 proves this).
- [ ] `seeders < 3` → `-1000` is tested at the unit level (Task 5) and via a total-score test.
- [ ] `search_movie` returns `Vec<ScoredTorrent>`, not `Result<...>` — simpler API per spec §2.6.
- [ ] `SearchConfig` and `SearchError` are fully removed (Task 14).
- [ ] Frontend still compiles after `AppConfig.search` removal (Task 13 fixes this first, Task 14 finishes).
- [ ] Live smoke tests are `#[ignore]` by default — CI never runs them.
- [ ] Provider failures are silent: logged via `tracing::warn!`, not surfaced to the UI.
