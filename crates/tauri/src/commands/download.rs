//! Tauri wrappers around `blowup_core::torrent::download::*`. The
//! background monitor task stays here because it uses the Tauri
//! state-manager to look up the LibraryIndex; it publishes to the
//! shared EventBus so both the desktop frontend (via the event
//! forwarder in lib.rs) and LAN-side iOS clients (via SSE) see the
//! same notifications.

use blowup_core::infra::events::{DomainEvent, EventBus};
use blowup_core::library::index::{IndexEntry, LibraryIndex};
use blowup_core::torrent::download::{
    self as svc, DownloadRecord, StartDownloadRequest, parse_genres_csv, validate_library_root,
};
use blowup_core::torrent::manager::{TorrentFileInfo, TorrentHandle, TorrentManager};
use blowup_core::torrent::tracker::TrackerManager;
use sqlx::SqlitePool;
use std::sync::Arc;
use tauri::Manager;

#[tauri::command]
pub async fn get_torrent_files(
    tm: tauri::State<'_, TorrentManager>,
    target: String,
) -> Result<Vec<TorrentFileInfo>, String> {
    tm.get_torrent_files(&target).await
}

#[tauri::command]
pub async fn start_download(
    app: tauri::AppHandle,
    events: tauri::State<'_, EventBus>,
    pool: tauri::State<'_, SqlitePool>,
    tm: tauri::State<'_, TorrentManager>,
    tracker_mgr: tauri::State<'_, TrackerManager>,
    index: tauri::State<'_, Arc<LibraryIndex>>,
    req: StartDownloadRequest,
) -> Result<i64, String> {
    validate_library_root(index.inner())?;
    let output_folder = index.compute_download_path(&req.director, req.tmdb_id);

    let download_id = svc::insert_download_record(pool.inner(), &req).await?;

    let trackers = tracker_mgr.hot_trackers().await;
    let (torrent_id, handle): (usize, TorrentHandle) = match tm
        .start_download(
            &req.target,
            output_folder.clone(),
            req.only_files,
            Some(trackers),
        )
        .await
    {
        Ok(r) => r,
        Err(e) => {
            svc::mark_failed(pool.inner(), download_id, &e).await;
            return Err(e);
        }
    };

    svc::set_torrent_id(pool.inner(), download_id, torrent_id as i64).await?;

    spawn_download_monitor(MonitorParams {
        app_handle: app,
        events: events.inner().clone(),
        pool: pool.inner().clone(),
        handle,
        download_id,
        output_folder,
        director: req.director,
        title: req.title,
        tmdb_id: req.tmdb_id,
        year: req.year,
        genres: req.genres.unwrap_or_default(),
    });

    events.publish(DomainEvent::DownloadsChanged);
    Ok(download_id)
}

struct MonitorParams {
    app_handle: tauri::AppHandle,
    events: EventBus,
    pool: SqlitePool,
    handle: TorrentHandle,
    download_id: i64,
    output_folder: std::path::PathBuf,
    director: String,
    title: String,
    tmdb_id: u64,
    year: Option<u32>,
    genres: Vec<String>,
}

fn spawn_download_monitor(p: MonitorParams) {
    let director_normalized = blowup_core::infra::common::normalize_director_name(&p.director);
    let director_display = p.director;

    tokio::spawn(async move {
        loop {
            tokio::time::sleep(std::time::Duration::from_secs(2)).await;
            let stats = p.handle.stats();

            svc::update_progress(
                &p.pool,
                p.download_id,
                stats.progress_bytes as i64,
                stats.total_bytes as i64,
            )
            .await;

            p.events.publish(DomainEvent::DownloadsChanged);

            if stats.finished {
                tracing::info!(p.download_id, "download completed");
                svc::mark_completed(&p.pool, p.download_id, stats.total_bytes as i64).await;

                let files = blowup_core::library::index::scan_dir_files(&p.output_folder);

                // Auto-extract embedded subtitles before adding to index
                for file in &files {
                    let ext = std::path::Path::new(file)
                        .extension()
                        .and_then(|e| e.to_str())
                        .unwrap_or("")
                        .to_lowercase();
                    if blowup_core::library::index::VIDEO_EXTENSIONS.contains(&ext.as_str()) {
                        let video_path = p.output_folder.join(file);
                        blowup_core::subtitle::service::auto_extract_all_subtitles(&video_path)
                            .await;
                    }
                }

                // Rescan after extraction to pick up new SRTs
                let files = blowup_core::library::index::scan_dir_files(&p.output_folder);
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
                if let Some(idx) = p.app_handle.try_state::<Arc<LibraryIndex>>()
                    && let Err(e) = idx.add_entry(entry)
                {
                    tracing::warn!(error = %e, "failed to add entry to library index");
                }

                p.events.publish(DomainEvent::LibraryChanged);
                break;
            }

            if let Some(err) = &stats.error {
                tracing::error!(p.download_id, error = %err, "download failed");
                svc::mark_failed(&p.pool, p.download_id, err).await;
                break;
            }
        }
    });
}

#[tauri::command]
pub async fn list_downloads(
    pool: tauri::State<'_, SqlitePool>,
) -> Result<Vec<DownloadRecord>, String> {
    svc::list_downloads(pool.inner()).await
}

#[tauri::command]
pub async fn pause_download(
    events: tauri::State<'_, EventBus>,
    pool: tauri::State<'_, SqlitePool>,
    tm: tauri::State<'_, TorrentManager>,
    id: i64,
) -> Result<(), String> {
    let record = svc::get_active_download(pool.inner(), id, "downloading").await?;

    if let Some(tid) = record.torrent_id {
        tm.pause(tid as usize).await.ok();
    }

    svc::mark_paused(pool.inner(), id).await?;
    events.publish(DomainEvent::DownloadsChanged);
    Ok(())
}

#[tauri::command]
pub async fn resume_download(
    app: tauri::AppHandle,
    events: tauri::State<'_, EventBus>,
    pool: tauri::State<'_, SqlitePool>,
    tm: tauri::State<'_, TorrentManager>,
    tracker_mgr: tauri::State<'_, TrackerManager>,
    index: tauri::State<'_, Arc<LibraryIndex>>,
    id: i64,
) -> Result<(), String> {
    let record = svc::get_active_download(pool.inner(), id, "paused").await?;

    let resumed = match record.torrent_id {
        Some(tid) => tm.unpause(tid as usize).await.is_ok(),
        None => false,
    };

    if resumed {
        svc::mark_resumed(pool.inner(), id).await?;
        events.publish(DomainEvent::DownloadsChanged);
        return Ok(());
    }

    // Torrent not in session (e.g. after app restart) — re-add from magnet link
    let tmdb_id = record.tmdb_id.unwrap_or(0) as u64;
    let director = record.director.unwrap_or_else(|| "Unknown".to_string());
    let output_folder = index.compute_download_path(&director, tmdb_id);

    let trackers = tracker_mgr.hot_trackers().await;
    let (torrent_id, handle) = tm
        .start_download(&record.target, output_folder.clone(), None, Some(trackers))
        .await?;

    svc::mark_resumed_with_new_torrent(pool.inner(), id, torrent_id as i64).await?;

    spawn_download_monitor(MonitorParams {
        app_handle: app,
        events: events.inner().clone(),
        pool: pool.inner().clone(),
        handle,
        download_id: id,
        output_folder,
        director,
        title: record.title,
        tmdb_id,
        year: record.year.map(|y| y as u32),
        genres: parse_genres_csv(record.genres.as_deref()),
    });

    events.publish(DomainEvent::DownloadsChanged);
    Ok(())
}

#[tauri::command]
pub async fn delete_download(
    events: tauri::State<'_, EventBus>,
    pool: tauri::State<'_, SqlitePool>,
    tm: tauri::State<'_, TorrentManager>,
    index: tauri::State<'_, Arc<LibraryIndex>>,
    id: i64,
) -> Result<(), String> {
    let record = svc::get_download_record(pool.inner(), id).await?;

    let is_active = matches!(record.status.as_str(), "downloading" | "paused" | "pending");

    if is_active {
        if let Some(tid) = record.torrent_id
            && let Err(e) = tm.remove(tid as usize).await
        {
            tracing::warn!(torrent_id = tid, error = %e, "failed to remove torrent");
        }

        let tmdb_id = record.tmdb_id.unwrap_or(0) as u64;
        let director = record.director.as_deref().unwrap_or("Unknown");
        let dir = index.compute_download_path(director, tmdb_id);
        if dir.exists() {
            if let Err(e) = std::fs::remove_dir_all(&dir) {
                tracing::warn!(error = %e, ?dir, "failed to remove download dir");
            } else {
                tracing::info!(?dir, "deleted download files");
                if let Some(parent) = dir.parent()
                    && parent.read_dir().is_ok_and(|mut d| d.next().is_none())
                    && let Err(e) = std::fs::remove_dir(parent)
                {
                    tracing::warn!(error = %e, ?parent, "failed to remove empty parent dir");
                }
            }
        }

        index.remove_entry(tmdb_id);
    }

    svc::delete_download_record(pool.inner(), id).await?;
    events.publish(DomainEvent::DownloadsChanged);
    if is_active {
        events.publish(DomainEvent::LibraryChanged);
    }
    Ok(())
}

#[tauri::command]
pub async fn list_download_existing_files(
    pool: tauri::State<'_, SqlitePool>,
    index: tauri::State<'_, Arc<LibraryIndex>>,
    id: i64,
) -> Result<Vec<String>, String> {
    let record = svc::get_download_record(pool.inner(), id).await?;

    let tmdb_id = record.tmdb_id.unwrap_or(0) as u64;
    let director = record.director.as_deref().unwrap_or("Unknown");
    let dir = index.compute_download_path(director, tmdb_id);

    if !dir.exists() {
        return Ok(Vec::new());
    }

    let files = std::fs::read_dir(&dir)
        .map_err(|e| e.to_string())?
        .filter_map(|e| e.ok())
        .filter(|e| e.path().is_file())
        .filter_map(|e| e.file_name().into_string().ok())
        .collect();

    Ok(files)
}

#[allow(clippy::too_many_arguments)]
#[tauri::command]
pub async fn redownload(
    app: tauri::AppHandle,
    events: tauri::State<'_, EventBus>,
    pool: tauri::State<'_, SqlitePool>,
    tm: tauri::State<'_, TorrentManager>,
    tracker_mgr: tauri::State<'_, TrackerManager>,
    index: tauri::State<'_, Arc<LibraryIndex>>,
    id: i64,
    only_files: Option<Vec<usize>>,
) -> Result<(), String> {
    let record = svc::get_redownload_record(pool.inner(), id).await?;
    let tmdb_id = record.tmdb_id.unwrap_or(0) as u64;

    validate_library_root(index.inner())?;
    let director = record.director.unwrap_or_else(|| "Unknown".to_string());
    let output_folder = index.compute_download_path(&director, tmdb_id);

    let trackers = tracker_mgr.hot_trackers().await;
    let (torrent_id, handle) = tm
        .start_download(
            &record.target,
            output_folder.clone(),
            only_files,
            Some(trackers),
        )
        .await?;

    svc::reset_for_redownload(pool.inner(), id, torrent_id as i64).await?;

    spawn_download_monitor(MonitorParams {
        app_handle: app,
        events: events.inner().clone(),
        pool: pool.inner().clone(),
        handle,
        download_id: id,
        output_folder,
        director,
        title: record.title,
        tmdb_id,
        year: record.year.map(|y| y as u32),
        genres: parse_genres_csv(record.genres.as_deref()),
    });

    events.publish(DomainEvent::DownloadsChanged);
    Ok(())
}
