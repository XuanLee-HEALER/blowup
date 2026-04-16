//! Cross-domain orchestration.
//!
//! Most of core is organised by single-domain service modules
//! (`tmdb::service`, `library::items`, `subtitle::service`, ...).
//! A few operations legitimately span multiple domains:
//!
//!   - starting a subtitle-alignment task reads from `subtitle` but
//!     also mutates the `tasks` registry and publishes a
//!     `DomainEvent::TasksChanged` on the shared `EventBus`.
//!   - enriching a library index entry talks to TMDB *and* writes
//!     poster bytes into the library filesystem tree.
//!   - the download completion path moves files into the library,
//!     extracts embedded subtitles, and bumps the index.
//!
//! Those orchestration functions live here so the individual domain
//! modules stay single-purpose. Domain modules are allowed to depend
//! on `infra::*` and on their own types, and `workflows::*` is
//! allowed to depend on multiple domains — but a single domain
//! module should not reach into another.
//!
//! See `docs/REFACTOR.md` for the longer story. This module is a
//! work-in-progress landing zone: functions move in here as the
//! underlying code gets detangled. At the moment it contains the
//! fire-and-forget subtitle-alignment helpers; more will follow.

pub mod download_monitor;
pub mod subtitle_align;
pub mod wiki_linker;

pub use subtitle_align::{run_subtitle_align_to_audio, run_subtitle_align_to_video};
