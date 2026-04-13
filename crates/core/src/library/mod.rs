//! Film library — on-disk film directories + SQLite items table.
//!
//! ## A note on cross-domain imports
//!
//! The `LibraryIndex` and its associated types (`IndexEntry`,
//! `FileMediaInfo`, `SubtitleDisplayConfig`, ...) are used by several
//! other domains (`tmdb::service::enrich_index_entry` writes posters
//! into it, `media::service::probe_and_cache` caches probes in it,
//! `torrent::download` looks up target directories, `library::items`
//! itself calls into `subtitle::parser::cleanup_stale_overlays` when
//! an SRT is deleted). This violates a strict reading of
//! "domains do not cross-import" from `docs/REFACTOR.md`.
//!
//! The pragmatic resolution: `LibraryIndex` is treated as an
//! infra-level type rather than a pure domain type. It's the
//! authoritative index of what's on disk, and anything that touches
//! the filesystem tree needs to read or update it. Long-term, the
//! intention is to move the truly cross-domain *workflows* (enrich,
//! probe-and-cache, download→library handoff) into
//! `crates/core/src/workflows/`; `subtitle_align` already lives
//! there as the first example. This migration is incremental and
//! tracked in `docs/REFACTOR.md`.

pub mod index;
pub mod items;

pub use index::{IndexEntry, LibraryIndex, VIDEO_EXTENSIONS};
pub use items::{
    LibraryAssetEntry, LibraryItemDetail, LibraryItemSummary, LibraryStats, ScanResult, StatEntry,
};
