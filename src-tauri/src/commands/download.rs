use crate::library_index::{IndexEntry, LibraryIndex};
use crate::torrent::{TorrentFileInfo, TorrentManager};
use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;

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
    pool: tauri::State<'_, SqlitePool>,
    tm: tauri::State<'_, TorrentManager>,
    index: tauri::State<'_, LibraryIndex>,
    req: StartDownloadRequest,
) -> Result<i64, String> {
    let output_folder = index.compute_download_path(&req.director, req.tmdb_id);

    // Insert DB record
    let download_id = sqlx::query_scalar::<_, i64>(
        "INSERT INTO downloads (tmdb_id, title, director, quality, target, status) \
         VALUES (?, ?, ?, ?, ?, 'downloading') RETURNING id",
    )
    .bind(req.tmdb_id as i64)
    .bind(&req.title)
    .bind(&req.director)
    .bind(&req.quality)
    .bind(&req.target)
    .fetch_one(pool.inner())
    .await
    .map_err(|e| e.to_string())?;

    // Start torrent
    let (torrent_id, handle): (usize, crate::torrent::TorrentHandle) = match tm
        .start_download(&req.target, output_folder.clone(), req.only_files)
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
    let pool_clone = pool.inner().clone();
    let output_path = output_folder;
    let director_display = req.director.clone();
    let director_normalized = crate::common::normalize_director_name(&req.director);
    let title = req.title;
    let tmdb_id = req.tmdb_id;
    let year = req.year;
    let genres = req.genres.unwrap_or_default();

    tokio::spawn(async move {
        loop {
            tokio::time::sleep(std::time::Duration::from_secs(2)).await;
            let stats = handle.stats();

            // Update progress
            sqlx::query(
                "UPDATE downloads SET progress_bytes=?, total_bytes=? \
                 WHERE id=? AND status='downloading'",
            )
            .bind(stats.progress_bytes as i64)
            .bind(stats.total_bytes as i64)
            .bind(download_id)
            .execute(&pool_clone)
            .await
            .ok();

            if stats.finished {
                tracing::info!(download_id, "download completed");
                sqlx::query(
                    "UPDATE downloads SET status='completed', progress_bytes=?, total_bytes=?, \
                     completed_at=datetime('now') WHERE id=?",
                )
                .bind(stats.total_bytes as i64)
                .bind(stats.total_bytes as i64)
                .bind(download_id)
                .execute(&pool_clone)
                .await
                .ok();

                // Write to library index
                let files = crate::library_index::scan_dir_files(&output_path);
                let entry = IndexEntry {
                    tmdb_id,
                    title,
                    director: director_normalized.clone(),
                    director_display,
                    year,
                    genres,
                    path: format!("{}/{}", director_normalized, tmdb_id),
                    files,
                    added_at: chrono::Utc::now().to_rfc3339(),
                };
                if let Some(root) = output_path.parent().and_then(|p| p.parent()) {
                    append_to_index_file(&root.join(".index.json"), entry);
                }
                break;
            }

            if let Some(err) = &stats.error {
                tracing::error!(download_id, error = %err, "download failed");
                sqlx::query("UPDATE downloads SET status='failed', error_message=? WHERE id=?")
                    .bind(err)
                    .bind(download_id)
                    .execute(&pool_clone)
                    .await
                    .ok();
                break;
            }
        }
    });

    Ok(download_id)
}

/// Append an entry to the index file on disk (used from background tasks
/// that don't have access to the LibraryIndex managed state).
fn append_to_index_file(index_path: &std::path::Path, entry: IndexEntry) {
    #[derive(Serialize, Deserialize, Default)]
    struct IndexFile {
        #[serde(default)]
        version: u32,
        #[serde(default)]
        entries: Vec<IndexEntry>,
    }

    let mut index = if index_path.exists() {
        std::fs::read_to_string(index_path)
            .ok()
            .and_then(|c| serde_json::from_str::<IndexFile>(&c).ok())
            .unwrap_or_default()
    } else {
        IndexFile::default()
    };

    index.entries.retain(|e| e.tmdb_id != entry.tmdb_id);
    index.entries.push(entry);
    index.version = 1;

    if let Ok(content) = serde_json::to_string_pretty(&index) {
        std::fs::write(index_path, content).ok();
    }
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
    Ok(())
}

#[tauri::command]
pub async fn resume_download(
    pool: tauri::State<'_, SqlitePool>,
    tm: tauri::State<'_, TorrentManager>,
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

    if let Some(tid) = record.torrent_id {
        tm.unpause(tid as usize).await?;
    }

    sqlx::query("UPDATE downloads SET status='downloading' WHERE id=?")
        .bind(id)
        .execute(pool.inner())
        .await
        .map_err(|e| e.to_string())?;
    Ok(())
}

/// Delete a download record.
/// - Active tasks (downloading/paused): stop torrent, delete files, remove from index
/// - History (completed/failed): only delete DB record
#[tauri::command]
pub async fn delete_download(
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
    Ok(())
}

/// Re-download from history: check if already in library, refuse if exists.
#[tauri::command]
pub async fn redownload(
    pool: tauri::State<'_, SqlitePool>,
    tm: tauri::State<'_, TorrentManager>,
    index: tauri::State<'_, LibraryIndex>,
    id: i64,
) -> Result<i64, String> {
    let record = sqlx::query_as::<_, DownloadRecord>(
        "SELECT * FROM downloads WHERE id=? AND status IN ('completed','failed')",
    )
    .bind(id)
    .fetch_optional(pool.inner())
    .await
    .map_err(|e| e.to_string())?
    .ok_or("download not found")?;

    let tmdb_id = record.tmdb_id.unwrap_or(0) as u64;

    // Check if already in library
    if let Some(entry) = index.get_entry(tmdb_id)
        && !entry.files.is_empty()
    {
        return Err(format!("「{}」已存在于电影库中", entry.title));
    }

    let director = record.director.unwrap_or_else(|| "Unknown".to_string());
    let output_folder = index.compute_download_path(&director, tmdb_id);

    let (torrent_id, handle) = tm
        .start_download(&record.target, output_folder.clone(), None)
        .await?;

    // Insert new record (keep old one in history)
    let new_id = sqlx::query_scalar::<_, i64>(
        "INSERT INTO downloads (tmdb_id, title, director, quality, target, status, torrent_id) \
         VALUES (?, ?, ?, ?, ?, 'downloading', ?) RETURNING id",
    )
    .bind(record.tmdb_id)
    .bind(&record.title)
    .bind(&director)
    .bind(&record.quality)
    .bind(&record.target)
    .bind(torrent_id as i64)
    .fetch_one(pool.inner())
    .await
    .map_err(|e| e.to_string())?;

    // Spawn background monitor
    let pool_clone = pool.inner().clone();
    let director_normalized = crate::common::normalize_director_name(&director);
    let director_display = director;
    let title = record.title;

    tokio::spawn(async move {
        loop {
            tokio::time::sleep(std::time::Duration::from_secs(2)).await;
            let stats = handle.stats();

            sqlx::query(
                "UPDATE downloads SET progress_bytes=?, total_bytes=? \
                 WHERE id=? AND status='downloading'",
            )
            .bind(stats.progress_bytes as i64)
            .bind(stats.total_bytes as i64)
            .bind(new_id)
            .execute(&pool_clone)
            .await
            .ok();

            if stats.finished {
                tracing::info!(new_id, "redownload completed");
                sqlx::query(
                    "UPDATE downloads SET status='completed', progress_bytes=?, total_bytes=?, \
                     completed_at=datetime('now') WHERE id=?",
                )
                .bind(stats.total_bytes as i64)
                .bind(stats.total_bytes as i64)
                .bind(new_id)
                .execute(&pool_clone)
                .await
                .ok();

                let files = crate::library_index::scan_dir_files(&output_folder);
                let entry = IndexEntry {
                    tmdb_id,
                    title,
                    director: director_normalized.clone(),
                    director_display,
                    year: None,
                    genres: Vec::new(),
                    path: format!("{}/{}", director_normalized, tmdb_id),
                    files,
                    added_at: chrono::Utc::now().to_rfc3339(),
                };
                if let Some(root) = output_folder.parent().and_then(|p| p.parent()) {
                    append_to_index_file(&root.join(".index.json"), entry);
                }
                break;
            }

            if let Some(err) = &stats.error {
                tracing::error!(new_id, error = %err, "redownload failed");
                sqlx::query("UPDATE downloads SET status='failed', error_message=? WHERE id=?")
                    .bind(err)
                    .bind(new_id)
                    .execute(&pool_clone)
                    .await
                    .ok();
                break;
            }
        }
    });

    Ok(new_id)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[sqlx::test]
    async fn test_download_record_crud(pool: SqlitePool) {
        sqlx::migrate!("./migrations").run(&pool).await.unwrap();

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
        sqlx::migrate!("./migrations").run(&pool).await.unwrap();

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
        sqlx::migrate!("./migrations").run(&pool).await.unwrap();

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
