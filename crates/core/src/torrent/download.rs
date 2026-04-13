//! Download record management — DB-backed torrent download state.
//!
//! This module owns the `DownloadRecord` table operations and a couple
//! of purely functional helpers. The long-running torrent monitor task,
//! which also emits Tauri events and holds an `AppHandle`, stays in
//! blowup-tauri for now and consumes these functions for its DB mutations.

use crate::library::index::LibraryIndex;
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

/// Check that the library root directory exists (or can be created) and is writable.
pub fn validate_library_root(index: &LibraryIndex) -> Result<(), String> {
    let root = index.root();
    if root.as_os_str().is_empty() {
        return Err("库目录未设置，请在设置中配置库目录路径".to_string());
    }
    if root.exists() {
        if !root.is_dir() {
            return Err(format!(
                "库目录路径「{}」不是一个目录，请在设置中修改",
                root.display()
            ));
        }
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
    if let Err(e) = std::fs::create_dir_all(root) {
        return Err(format!(
            "无法创建库目录「{}」: {}，请在设置中修改路径",
            root.display(),
            e
        ));
    }
    Ok(())
}

// ── DB operations ──────────────────────────────────────────────────

pub async fn insert_download_record(
    pool: &SqlitePool,
    req: &StartDownloadRequest,
) -> Result<i64, String> {
    let genres_csv = req
        .genres
        .as_deref()
        .map(|g| g.join(","))
        .unwrap_or_default();
    sqlx::query_scalar::<_, i64>(
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
    .fetch_one(pool)
    .await
    .map_err(|e| e.to_string())
}

pub async fn set_torrent_id(
    pool: &SqlitePool,
    download_id: i64,
    torrent_id: i64,
) -> Result<(), String> {
    sqlx::query("UPDATE downloads SET torrent_id=? WHERE id=?")
        .bind(torrent_id)
        .bind(download_id)
        .execute(pool)
        .await
        .map_err(|e| e.to_string())
        .map(|_| ())
}

pub async fn update_progress(
    pool: &SqlitePool,
    download_id: i64,
    progress_bytes: i64,
    total_bytes: i64,
) {
    sqlx::query(
        "UPDATE downloads SET progress_bytes=?, total_bytes=? \
         WHERE id=? AND status='downloading'",
    )
    .bind(progress_bytes)
    .bind(total_bytes)
    .bind(download_id)
    .execute(pool)
    .await
    .ok();
}

pub async fn mark_completed(pool: &SqlitePool, download_id: i64, total_bytes: i64) {
    sqlx::query(
        "UPDATE downloads SET status='completed', progress_bytes=?, total_bytes=?, \
         completed_at=datetime('now') WHERE id=?",
    )
    .bind(total_bytes)
    .bind(total_bytes)
    .bind(download_id)
    .execute(pool)
    .await
    .ok();
}

pub async fn mark_failed(pool: &SqlitePool, download_id: i64, error: &str) {
    sqlx::query("UPDATE downloads SET status='failed', error_message=? WHERE id=?")
        .bind(error)
        .bind(download_id)
        .execute(pool)
        .await
        .ok();
}

pub async fn mark_paused(pool: &SqlitePool, download_id: i64) -> Result<(), String> {
    sqlx::query("UPDATE downloads SET status='paused' WHERE id=?")
        .bind(download_id)
        .execute(pool)
        .await
        .map_err(|e| e.to_string())
        .map(|_| ())
}

pub async fn mark_resumed(pool: &SqlitePool, download_id: i64) -> Result<(), String> {
    sqlx::query("UPDATE downloads SET status='downloading' WHERE id=?")
        .bind(download_id)
        .execute(pool)
        .await
        .map_err(|e| e.to_string())
        .map(|_| ())
}

pub async fn mark_resumed_with_new_torrent(
    pool: &SqlitePool,
    download_id: i64,
    torrent_id: i64,
) -> Result<(), String> {
    sqlx::query("UPDATE downloads SET status='downloading', torrent_id=? WHERE id=?")
        .bind(torrent_id)
        .bind(download_id)
        .execute(pool)
        .await
        .map_err(|e| e.to_string())
        .map(|_| ())
}

pub async fn reset_for_redownload(
    pool: &SqlitePool,
    download_id: i64,
    torrent_id: i64,
) -> Result<(), String> {
    sqlx::query(
        "UPDATE downloads SET status='downloading', torrent_id=?, \
         progress_bytes=0, total_bytes=0, error_message=NULL, \
         completed_at=NULL, started_at=datetime('now') WHERE id=?",
    )
    .bind(torrent_id)
    .bind(download_id)
    .execute(pool)
    .await
    .map_err(|e| e.to_string())
    .map(|_| ())
}

pub async fn list_downloads(pool: &SqlitePool) -> Result<Vec<DownloadRecord>, String> {
    sqlx::query_as::<_, DownloadRecord>("SELECT * FROM downloads ORDER BY started_at DESC")
        .fetch_all(pool)
        .await
        .map_err(|e| e.to_string())
}

pub async fn get_active_download(
    pool: &SqlitePool,
    id: i64,
    status: &str,
) -> Result<DownloadRecord, String> {
    sqlx::query_as::<_, DownloadRecord>("SELECT * FROM downloads WHERE id=? AND status=?")
        .bind(id)
        .bind(status)
        .fetch_optional(pool)
        .await
        .map_err(|e| e.to_string())?
        .ok_or_else(|| crate::error::status::not_found(format!("download not in {status} state")))
}

pub async fn get_download_record(pool: &SqlitePool, id: i64) -> Result<DownloadRecord, String> {
    sqlx::query_as::<_, DownloadRecord>("SELECT * FROM downloads WHERE id=?")
        .bind(id)
        .fetch_optional(pool)
        .await
        .map_err(|e| e.to_string())?
        .ok_or_else(|| crate::error::status::not_found("download"))
}

pub async fn get_redownload_record(pool: &SqlitePool, id: i64) -> Result<DownloadRecord, String> {
    sqlx::query_as::<_, DownloadRecord>(
        "SELECT * FROM downloads WHERE id=? AND status IN ('completed','failed')",
    )
    .bind(id)
    .fetch_optional(pool)
    .await
    .map_err(|e| e.to_string())?
    .ok_or_else(|| crate::error::status::not_found("download"))
}

pub async fn delete_download_record(pool: &SqlitePool, id: i64) -> Result<(), String> {
    sqlx::query("DELETE FROM downloads WHERE id=?")
        .bind(id)
        .execute(pool)
        .await
        .map_err(|e| e.to_string())
        .map(|_| ())
}

/// Parse the CSV `genres` column back into a Vec<String>.
pub fn parse_genres_csv(csv: Option<&str>) -> Vec<String> {
    csv.unwrap_or("")
        .split(',')
        .filter(|s| !s.is_empty())
        .map(String::from)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[sqlx::test]
    async fn test_download_record_crud(pool: SqlitePool) {
        crate::infra::db::MIGRATOR.run(&pool).await.unwrap();

        sqlx::query(
            "INSERT INTO downloads (tmdb_id, title, director, target, status) \
             VALUES (1, 'Test', 'Dir', 'magnet:test', 'downloading')",
        )
        .execute(&pool)
        .await
        .unwrap();

        let records = list_downloads(&pool).await.unwrap();

        assert_eq!(records.len(), 1);
        assert_eq!(records[0].title, "Test");
        assert_eq!(records[0].status, "downloading");
    }

    #[sqlx::test]
    async fn test_pause_sets_status(pool: SqlitePool) {
        crate::infra::db::MIGRATOR.run(&pool).await.unwrap();

        sqlx::query(
            "INSERT INTO downloads (tmdb_id, title, target, status) \
             VALUES (1, 'Test', 'magnet:x', 'downloading')",
        )
        .execute(&pool)
        .await
        .unwrap();

        mark_paused(&pool, 1).await.unwrap();

        let record = get_download_record(&pool, 1).await.unwrap();
        assert_eq!(record.status, "paused");
    }

    #[sqlx::test]
    async fn test_delete_download(pool: SqlitePool) {
        crate::infra::db::MIGRATOR.run(&pool).await.unwrap();

        sqlx::query(
            "INSERT INTO downloads (tmdb_id, title, target, status) \
             VALUES (1, 'Test', 'magnet:x', 'completed')",
        )
        .execute(&pool)
        .await
        .unwrap();

        delete_download_record(&pool, 1).await.unwrap();

        let count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM downloads")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(count.0, 0);
    }
}
