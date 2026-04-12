use crate::commands::tracker::TrackerManager;
use crate::library_index::{IndexEntry, LibraryIndex};
use crate::torrent::{TorrentFileInfo, TorrentManager};
use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;
use tauri::{Emitter, Manager};

fn emit(app: &tauri::AppHandle, event: &str) {
    if let Err(e) = app.emit(event, ()) {
        tracing::warn!(error = %e, event, "failed to emit event");
    }
}

fn validate_library_root(index: &LibraryIndex) -> Result<(), String> {
    let root = index.root();
    if root.as_os_str().is_empty() {
        return Err("库目录未设置，请在设置中配置库目录路径".to_string());
    }
    // If the directory exists, check it is actually a directory and writable
    if root.exists() {
        if !root.is_dir() {
            return Err(format!(
                "库目录路径「{}」不是一个目录，请在设置中修改",
                root.display()
            ));
        }
        // Try creating a temp file to verify write permission
        let probe = root.join(".blowup_write_test");
        match std::fs::write(&probe, b"") {
            Ok(_) => {
                std::fs::remove_file(&probe).ok();
            }
            Err(_) => {
                return Err(format!(
                    "库目录「{}」没有写入权限，请在设置中修改或调整目录权限",
                    root.display()
                ));
            }
        }
        return Ok(());
    }
    // Directory doesn't exist — try to create it
    if let Err(e) = std::fs::create_dir_all(root) {
        return Err(format!(
            "无法创建库目录「{}」: {}，请在设置中修改路径",
            root.display(),
            e
        ));
    }
    Ok(())
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct DownloadRecord {
    pub id: i64,
    pub tmdb_id: Option<i64>,
    pub title: String,
    pub director: Option<String>,
    pub quality: Option<String>,
    pub target: String,
    pub status: String,
    pub torrent_id: Option<i64>,
    pub progress_bytes: i64,
    pub total_bytes: i64,
    pub error_message: Option<String>,
    pub started_at: String,
    pub completed_at: Option<String>,
    pub year: Option<i64>,
    pub genres: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StartDownloadRequest {
    pub title: String,
    pub target: String,
    pub director: String,
    pub tmdb_id: u64,
    pub year: Option<u32>,
    pub genres: Option<Vec<String>>,
    pub quality: Option<String>,
    pub only_files: Option<Vec<usize>>,
}

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
    pool: tauri::State<'_, SqlitePool>,
    tm: tauri::State<'_, TorrentManager>,
    tracker_mgr: tauri::State<'_, TrackerManager>,
    index: tauri::State<'_, LibraryIndex>,
    req: StartDownloadRequest,
) -> Result<i64, String> {
    validate_library_root(&index)?;
    let output_folder = index.compute_download_path(&req.director, req.tmdb_id);

    // Insert DB record
    let genres_csv = req
        .genres
        .as_deref()
        .map(|g| g.join(","))
        .unwrap_or_default();
    let download_id = sqlx::query_scalar::<_, i64>(
        "INSERT INTO downloads (tmdb_id, title, director, quality, target, status, year, genres) \
         VALUES (?, ?, ?, ?, ?, 'downloading', ?, ?) RETURNING id",
    )
    .bind(req.tmdb_id as i64)
    .bind(&req.title)
    .bind(&req.director)
    .bind(&req.quality)
    .bind(&req.target)
    .bind(req.year.map(|y| y as i64))
    .bind(&genres_csv)
    .fetch_one(pool.inner())
    .await
    .map_err(|e| e.to_string())?;

    // Start torrent with hot tracker list
    let trackers = tracker_mgr.hot_trackers().await;
    let (torrent_id, handle): (usize, crate::torrent::TorrentHandle) = match tm
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
            sqlx::query("UPDATE downloads SET status='failed', error_message=? WHERE id=?")
                .bind(&e)
                .bind(download_id)
                .execute(pool.inner())
                .await
                .ok();
            return Err(e);
        }
    };

    // Store torrent_id
    sqlx::query("UPDATE downloads SET torrent_id=? WHERE id=?")
        .bind(torrent_id as i64)
        .bind(download_id)
        .execute(pool.inner())
        .await
        .ok();

    // Spawn background monitor
    spawn_download_monitor(MonitorParams {
        app_handle: app,
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

    Ok(download_id)
}

struct MonitorParams {
    app_handle: tauri::AppHandle,
    pool: SqlitePool,
    handle: crate::torrent::TorrentHandle,
    download_id: i64,
    output_folder: std::path::PathBuf,
    director: String,
    title: String,
    tmdb_id: u64,
    year: Option<u32>,
    genres: Vec<String>,
}

fn spawn_download_monitor(p: MonitorParams) {
    let director_normalized = crate::common::normalize_director_name(&p.director);
    let director_display = p.director;

    tokio::spawn(async move {
        loop {
            tokio::time::sleep(std::time::Duration::from_secs(2)).await;
            let stats = p.handle.stats();

            sqlx::query(
                "UPDATE downloads SET progress_bytes=?, total_bytes=? \
                 WHERE id=? AND status='downloading'",
            )
            .bind(stats.progress_bytes as i64)
            .bind(stats.total_bytes as i64)
            .bind(p.download_id)
            .execute(&p.pool)
            .await
            .ok();

            emit(&p.app_handle, "downloads:changed");

            if stats.finished {
                tracing::info!(p.download_id, "download completed");
                sqlx::query(
                    "UPDATE downloads SET status='completed', progress_bytes=?, total_bytes=?, \
                     completed_at=datetime('now') WHERE id=?",
                )
                .bind(stats.total_bytes as i64)
                .bind(stats.total_bytes as i64)
                .bind(p.download_id)
                .execute(&p.pool)
                .await
                .ok();

                let files = crate::library_index::scan_dir_files(&p.output_folder);

                // Auto-extract embedded subtitles before adding to index
                for file in &files {
                    let ext = std::path::Path::new(file)
                        .extension()
                        .and_then(|e| e.to_str())
                        .unwrap_or("")
                        .to_lowercase();
                    if crate::library_index::VIDEO_EXTENSIONS.contains(&ext.as_str()) {
                        let video_path = p.output_folder.join(file);
                        blowup_core::subtitle::service::auto_extract_all_subtitles(&video_path).await;
                    }
                }

                // Rescan after extraction to pick up new SRTs
                let files = crate::library_index::scan_dir_files(&p.output_folder);
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
                // Update in-memory index + persist to disk
                if let Some(idx) = p
                    .app_handle
                    .try_state::<crate::library_index::LibraryIndex>()
                    && let Err(e) = idx.add_entry(entry)
                {
                    tracing::warn!(error = %e, "failed to add entry to library index");
                }

                emit(&p.app_handle, "library:changed");
                break;
            }

            if let Some(err) = &stats.error {
                tracing::error!(p.download_id, error = %err, "download failed");
                sqlx::query("UPDATE downloads SET status='failed', error_message=? WHERE id=?")
                    .bind(err)
                    .bind(p.download_id)
                    .execute(&p.pool)
                    .await
                    .ok();
                break;
            }
        }
    });
}

#[tauri::command]
pub async fn list_downloads(
    pool: tauri::State<'_, SqlitePool>,
) -> Result<Vec<DownloadRecord>, String> {
    sqlx::query_as::<_, DownloadRecord>("SELECT * FROM downloads ORDER BY started_at DESC")
        .fetch_all(pool.inner())
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn pause_download(
    app: tauri::AppHandle,
    pool: tauri::State<'_, SqlitePool>,
    tm: tauri::State<'_, TorrentManager>,
    id: i64,
) -> Result<(), String> {
    let record = sqlx::query_as::<_, DownloadRecord>(
        "SELECT * FROM downloads WHERE id=? AND status='downloading'",
    )
    .bind(id)
    .fetch_optional(pool.inner())
    .await
    .map_err(|e| e.to_string())?
    .ok_or("download not found or not active")?;

    if let Some(tid) = record.torrent_id {
        tm.pause(tid as usize).await.ok();
    }

    sqlx::query("UPDATE downloads SET status='paused' WHERE id=?")
        .bind(id)
        .execute(pool.inner())
        .await
        .map_err(|e| e.to_string())?;
    emit(&app, "downloads:changed");
    Ok(())
}

#[tauri::command]
pub async fn resume_download(
    app: tauri::AppHandle,
    pool: tauri::State<'_, SqlitePool>,
    tm: tauri::State<'_, TorrentManager>,
    tracker_mgr: tauri::State<'_, TrackerManager>,
    index: tauri::State<'_, LibraryIndex>,
    id: i64,
) -> Result<(), String> {
    let record = sqlx::query_as::<_, DownloadRecord>(
        "SELECT * FROM downloads WHERE id=? AND status='paused'",
    )
    .bind(id)
    .fetch_optional(pool.inner())
    .await
    .map_err(|e| e.to_string())?
    .ok_or("download not found or not paused")?;

    // Try to unpause the existing torrent in session
    let resumed = match record.torrent_id {
        Some(tid) => tm.unpause(tid as usize).await.is_ok(),
        None => false,
    };

    if resumed {
        sqlx::query("UPDATE downloads SET status='downloading' WHERE id=?")
            .bind(id)
            .execute(pool.inner())
            .await
            .map_err(|e| e.to_string())?;
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

    sqlx::query("UPDATE downloads SET status='downloading', torrent_id=? WHERE id=?")
        .bind(torrent_id as i64)
        .bind(id)
        .execute(pool.inner())
        .await
        .map_err(|e| e.to_string())?;

    spawn_download_monitor(MonitorParams {
        app_handle: app,
        pool: pool.inner().clone(),
        handle,
        download_id: id,
        output_folder,
        director,
        title: record.title,
        tmdb_id,
        year: record.year.map(|y| y as u32),
        genres: record
            .genres
            .as_deref()
            .unwrap_or("")
            .split(',')
            .filter(|s| !s.is_empty())
            .map(String::from)
            .collect(),
    });

    Ok(())
}

/// Delete a download record.
/// - Active tasks (downloading/paused): stop torrent, delete files, remove from index
/// - History (completed/failed): only delete DB record
#[tauri::command]
pub async fn delete_download(
    app: tauri::AppHandle,
    pool: tauri::State<'_, SqlitePool>,
    tm: tauri::State<'_, TorrentManager>,
    index: tauri::State<'_, LibraryIndex>,
    id: i64,
) -> Result<(), String> {
    let record = sqlx::query_as::<_, DownloadRecord>("SELECT * FROM downloads WHERE id=?")
        .bind(id)
        .fetch_optional(pool.inner())
        .await
        .map_err(|e| e.to_string())?
        .ok_or("download not found")?;

    let is_active = matches!(record.status.as_str(), "downloading" | "paused" | "pending");

    if is_active {
        // Stop torrent
        if let Some(tid) = record.torrent_id {
            tm.remove(tid as usize).await.ok();
        }

        // Delete files on disk
        let tmdb_id = record.tmdb_id.unwrap_or(0) as u64;
        let director = record.director.as_deref().unwrap_or("Unknown");
        let dir = index.compute_download_path(director, tmdb_id);
        if dir.exists() {
            std::fs::remove_dir_all(&dir).ok();
            tracing::info!(?dir, "deleted download files");
            if let Some(parent) = dir.parent()
                && parent.read_dir().is_ok_and(|mut d| d.next().is_none())
            {
                std::fs::remove_dir(parent).ok();
            }
        }

        // Remove from library index
        index.remove_entry(tmdb_id);
    }

    // Always delete DB record
    sqlx::query("DELETE FROM downloads WHERE id=?")
        .bind(id)
        .execute(pool.inner())
        .await
        .map_err(|e| e.to_string())?;
    emit(&app, "downloads:changed");
    if is_active {
        emit(&app, "library:changed");
    }
    Ok(())
}

/// List files that already exist on disk for a download record's output directory.
#[tauri::command]
pub async fn list_download_existing_files(
    pool: tauri::State<'_, SqlitePool>,
    index: tauri::State<'_, LibraryIndex>,
    id: i64,
) -> Result<Vec<String>, String> {
    let record = sqlx::query_as::<_, DownloadRecord>("SELECT * FROM downloads WHERE id=?")
        .bind(id)
        .fetch_optional(pool.inner())
        .await
        .map_err(|e| e.to_string())?
        .ok_or("download not found")?;

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

/// Re-download: reuse existing record, start download with selected files.
#[tauri::command]
pub async fn redownload(
    app: tauri::AppHandle,
    pool: tauri::State<'_, SqlitePool>,
    tm: tauri::State<'_, TorrentManager>,
    tracker_mgr: tauri::State<'_, TrackerManager>,
    index: tauri::State<'_, LibraryIndex>,
    id: i64,
    only_files: Option<Vec<usize>>,
) -> Result<(), String> {
    let record = sqlx::query_as::<_, DownloadRecord>(
        "SELECT * FROM downloads WHERE id=? AND status IN ('completed','failed')",
    )
    .bind(id)
    .fetch_optional(pool.inner())
    .await
    .map_err(|e| e.to_string())?
    .ok_or("download not found")?;

    let tmdb_id = record.tmdb_id.unwrap_or(0) as u64;

    validate_library_root(&index)?;
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

    // Reset existing record
    sqlx::query(
        "UPDATE downloads SET status='downloading', torrent_id=?, \
         progress_bytes=0, total_bytes=0, error_message=NULL, \
         completed_at=NULL, started_at=datetime('now') WHERE id=?",
    )
    .bind(torrent_id as i64)
    .bind(id)
    .execute(pool.inner())
    .await
    .map_err(|e| e.to_string())?;

    spawn_download_monitor(MonitorParams {
        app_handle: app,
        pool: pool.inner().clone(),
        handle,
        download_id: id,
        output_folder,
        director,
        title: record.title,
        tmdb_id,
        year: record.year.map(|y| y as u32),
        genres: record
            .genres
            .as_deref()
            .unwrap_or("")
            .split(',')
            .filter(|s| !s.is_empty())
            .map(String::from)
            .collect(),
    });

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[sqlx::test]
    async fn test_download_record_crud(pool: SqlitePool) {
        blowup_core::infra::db::MIGRATOR.run(&pool).await.unwrap();

        sqlx::query(
            "INSERT INTO downloads (tmdb_id, title, director, target, status) \
             VALUES (1, 'Test', 'Dir', 'magnet:test', 'downloading')",
        )
        .execute(&pool)
        .await
        .unwrap();

        let records =
            sqlx::query_as::<_, DownloadRecord>("SELECT * FROM downloads ORDER BY started_at DESC")
                .fetch_all(&pool)
                .await
                .unwrap();

        assert_eq!(records.len(), 1);
        assert_eq!(records[0].title, "Test");
        assert_eq!(records[0].status, "downloading");
    }

    #[sqlx::test]
    async fn test_pause_sets_status(pool: SqlitePool) {
        blowup_core::infra::db::MIGRATOR.run(&pool).await.unwrap();

        sqlx::query(
            "INSERT INTO downloads (tmdb_id, title, target, status) \
             VALUES (1, 'Test', 'magnet:x', 'downloading')",
        )
        .execute(&pool)
        .await
        .unwrap();

        sqlx::query("UPDATE downloads SET status='paused' WHERE id=1")
            .execute(&pool)
            .await
            .unwrap();

        let record = sqlx::query_as::<_, DownloadRecord>("SELECT * FROM downloads WHERE id=1")
            .fetch_one(&pool)
            .await
            .unwrap();

        assert_eq!(record.status, "paused");
    }

    #[sqlx::test]
    async fn test_delete_download(pool: SqlitePool) {
        blowup_core::infra::db::MIGRATOR.run(&pool).await.unwrap();

        sqlx::query(
            "INSERT INTO downloads (tmdb_id, title, target, status) \
             VALUES (1, 'Test', 'magnet:x', 'completed')",
        )
        .execute(&pool)
        .await
        .unwrap();

        sqlx::query("DELETE FROM downloads WHERE id=1")
            .execute(&pool)
            .await
            .unwrap();

        let count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM downloads")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(count.0, 0);
    }
}
