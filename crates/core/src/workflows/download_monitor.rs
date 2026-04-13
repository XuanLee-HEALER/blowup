//! Background task that polls a librqbit torrent handle and transitions
//! the DB record + library index when it finishes.
//!
//! This is a pure workflow (it glues the `torrent`, `subtitle`, and
//! `library` domains together) so it lives here rather than in any one
//! of them. Both the Tauri and server adapters call `spawn(...)` with
//! the same parameters — no more Tauri-specific `app.try_state::<...>`
//! pattern that used to keep this function locked inside the tauri
//! crate.

use crate::infra::common::normalize_director_name;
use crate::infra::events::{DomainEvent, EventBus};
use crate::library::index::{IndexEntry, LibraryIndex};
use crate::subtitle::service::auto_extract_all_subtitles;
use crate::torrent::download as download_svc;
use crate::torrent::manager::TorrentHandle;
use sqlx::SqlitePool;
use std::path::PathBuf;
use std::sync::Arc;

pub struct DownloadMonitorParams {
    pub pool: SqlitePool,
    pub events: EventBus,
    pub library_index: Arc<LibraryIndex>,
    pub handle: TorrentHandle,
    pub download_id: i64,
    pub output_folder: PathBuf,
    pub director: String,
    pub title: String,
    pub tmdb_id: u64,
    pub year: Option<u32>,
    pub genres: Vec<String>,
}

/// Spawn a background tokio task that polls `handle.stats()` every
/// two seconds, writes progress to the `downloads` row, and — on
/// completion — extracts embedded subtitles, scans files, and
/// registers the result in the library index.
///
/// The spawned task owns its parameters, so the caller can move on
/// immediately. Publishing goes through the provided `EventBus`, so
/// both the Tauri frontend (via the event forwarder in the tauri
/// crate) and LAN clients (via the server's SSE endpoint) see the
/// same notifications.
pub fn spawn(p: DownloadMonitorParams) {
    let director_normalized = normalize_director_name(&p.director);
    let director_display = p.director;

    tokio::spawn(async move {
        loop {
            tokio::time::sleep(std::time::Duration::from_secs(2)).await;
            let stats = p.handle.stats();

            download_svc::update_progress(
                &p.pool,
                p.download_id,
                stats.progress_bytes as i64,
                stats.total_bytes as i64,
            )
            .await;

            p.events.publish(DomainEvent::DownloadsChanged);

            if stats.finished {
                tracing::info!(p.download_id, "download completed");
                download_svc::mark_completed(&p.pool, p.download_id, stats.total_bytes as i64)
                    .await;

                let files = crate::library::index::scan_dir_files(&p.output_folder);

                // Auto-extract embedded subtitles before adding to index
                for file in &files {
                    let ext = std::path::Path::new(file)
                        .extension()
                        .and_then(|e| e.to_str())
                        .unwrap_or("")
                        .to_lowercase();
                    if crate::library::index::VIDEO_EXTENSIONS.contains(&ext.as_str()) {
                        let video_path = p.output_folder.join(file);
                        auto_extract_all_subtitles(&video_path).await;
                    }
                }

                // Rescan after extraction to pick up new SRTs
                let files = crate::library::index::scan_dir_files(&p.output_folder);
                let entry = IndexEntry {
                    tmdb_id: p.tmdb_id,
                    title: p.title,
                    director: director_normalized.clone(),
                    director_display,
                    year: p.year,
                    genres: p.genres,
                    path: format!("{}/{}", director_normalized, p.tmdb_id),
                    files,
                    added_at: chrono::Utc::now().to_rfc3339(),
                    ..Default::default()
                };
                if let Err(e) = p.library_index.add_entry(entry) {
                    tracing::warn!(error = %e, "failed to add entry to library index");
                }

                p.events.publish(DomainEvent::LibraryChanged);
                break;
            }

            if let Some(err) = &stats.error {
                tracing::error!(p.download_id, error = %err, "download failed");
                download_svc::mark_failed(&p.pool, p.download_id, err).await;
                break;
            }
        }
    });
}
