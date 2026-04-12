//! Library items domain: DB-backed media file catalogue + in-memory
//! LibraryIndex helpers. Types exposed here are also the Tauri
//! command input/output shapes.

use crate::infra::ffmpeg::FfmpegTool;
use crate::library::index::VIDEO_EXTENSIONS;
use serde::Serialize;
use sqlx::SqlitePool;
use std::path::Path;

// ── Public types ─────────────────────────────────────────────────

#[derive(Serialize, sqlx::FromRow)]
pub struct LibraryItemSummary {
    pub id: i64,
    pub file_path: String,
    pub file_size: Option<i64>,
    pub duration_secs: Option<i64>,
    pub video_codec: Option<String>,
    pub audio_codec: Option<String>,
    pub resolution: Option<String>,
    pub added_at: String,
}

#[derive(Serialize)]
pub struct LibraryItemDetail {
    pub id: i64,
    pub file_path: String,
    pub file_size: Option<i64>,
    pub duration_secs: Option<i64>,
    pub video_codec: Option<String>,
    pub audio_codec: Option<String>,
    pub resolution: Option<String>,
    pub added_at: String,
    pub assets: Vec<LibraryAssetEntry>,
}

#[derive(Serialize, sqlx::FromRow)]
pub struct LibraryAssetEntry {
    pub id: i64,
    pub asset_type: String,
    pub file_path: String,
    pub lang: Option<String>,
    pub created_at: String,
}

#[derive(Serialize)]
pub struct LibraryStats {
    pub total_items: i64,
    pub total_file_size: i64,
    pub by_resolution: Vec<StatEntry>,
}

#[derive(Serialize, sqlx::FromRow)]
pub struct StatEntry {
    pub label: String,
    pub count: i64,
}

#[derive(Serialize)]
pub struct ScanResult {
    pub added: i64,
    pub skipped: i64,
    pub errors: Vec<String>,
}

// ── Video probe helper ──────────────────────────────────────────

struct VideoProbe {
    file_size: Option<i64>,
    duration_secs: Option<i64>,
    video_codec: Option<String>,
    audio_codec: Option<String>,
    resolution: Option<String>,
}

async fn probe_video_file(path: &str) -> Result<VideoProbe, String> {
    let args: Vec<String> = [
        "-v",
        "quiet",
        "-print_format",
        "json",
        "-show_format",
        "-show_streams",
        "--",
        path,
    ]
    .iter()
    .map(|s| s.to_string())
    .collect();

    let (stdout, _) = FfmpegTool::Ffprobe
        .exec_with_options(None::<&str>, Some(args))
        .await
        .map_err(|e| e.to_string())?;

    let json: serde_json::Value =
        serde_json::from_str(&stdout).map_err(|e| format!("ffprobe parse error: {}", e))?;

    let format = &json["format"];
    let file_size = format["size"].as_str().and_then(|s| s.parse::<i64>().ok());
    let duration_secs = format["duration"]
        .as_str()
        .and_then(|s| s.parse::<f64>().ok())
        .map(|d| d as i64);

    let mut video_codec = None;
    let mut audio_codec = None;
    let mut width: Option<i64> = None;
    let mut height: Option<i64> = None;

    if let Some(streams) = json["streams"].as_array() {
        for s in streams {
            match s["codec_type"].as_str() {
                Some("video") if video_codec.is_none() => {
                    video_codec = s["codec_name"].as_str().map(String::from);
                    width = s["width"].as_i64();
                    height = s["height"].as_i64();
                }
                Some("audio") if audio_codec.is_none() => {
                    audio_codec = s["codec_name"].as_str().map(String::from);
                }
                _ => {}
            }
        }
    }

    let resolution = match (width, height) {
        (Some(w), Some(h)) => Some(format!("{}x{}", w, h)),
        _ => None,
    };

    Ok(VideoProbe {
        file_size,
        duration_secs,
        video_codec,
        audio_codec,
        resolution,
    })
}

fn is_video_file(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .map(|e| VIDEO_EXTENSIONS.contains(&e.to_lowercase().as_str()))
        .unwrap_or(false)
}

// ── library_items DB service ────────────────────────────────────

pub async fn add_library_item(pool: &SqlitePool, file_path: &str) -> Result<i64, String> {
    let probe = probe_video_file(file_path).await.unwrap_or(VideoProbe {
        file_size: None,
        duration_secs: None,
        video_codec: None,
        audio_codec: None,
        resolution: None,
    });

    sqlx::query(
        "INSERT INTO library_items (file_path, file_size, duration_secs, video_codec, audio_codec, resolution)
         VALUES (?, ?, ?, ?, ?, ?)",
    )
    .bind(file_path)
    .bind(probe.file_size)
    .bind(probe.duration_secs)
    .bind(&probe.video_codec)
    .bind(&probe.audio_codec)
    .bind(&probe.resolution)
    .execute(pool)
    .await
    .map(|r| r.last_insert_rowid())
    .map_err(|e| e.to_string())
}

pub async fn list_library_items(pool: &SqlitePool) -> Result<Vec<LibraryItemSummary>, String> {
    sqlx::query_as::<_, LibraryItemSummary>(
        "SELECT id, file_path, file_size, duration_secs,
                video_codec, audio_codec, resolution, added_at
         FROM library_items
         ORDER BY added_at DESC",
    )
    .fetch_all(pool)
    .await
    .map_err(|e| e.to_string())
}

pub async fn get_library_item(pool: &SqlitePool, id: i64) -> Result<LibraryItemDetail, String> {
    let row = sqlx::query_as::<_, LibraryItemSummary>(
        "SELECT id, file_path, file_size, duration_secs,
                video_codec, audio_codec, resolution, added_at
         FROM library_items WHERE id = ?",
    )
    .bind(id)
    .fetch_one(pool)
    .await
    .map_err(|e| e.to_string())?;

    let assets = sqlx::query_as::<_, LibraryAssetEntry>(
        "SELECT id, asset_type, file_path, lang, created_at
         FROM library_assets WHERE item_id = ? ORDER BY created_at",
    )
    .bind(id)
    .fetch_all(pool)
    .await
    .map_err(|e| e.to_string())?;

    Ok(LibraryItemDetail {
        id: row.id,
        file_path: row.file_path,
        file_size: row.file_size,
        duration_secs: row.duration_secs,
        video_codec: row.video_codec,
        audio_codec: row.audio_codec,
        resolution: row.resolution,
        added_at: row.added_at,
        assets,
    })
}

/// Delete library_assets + library_items for a given item ID.
pub async fn delete_item_cascade(pool: &SqlitePool, id: i64) -> Result<(), String> {
    sqlx::query("DELETE FROM library_assets WHERE item_id = ?")
        .bind(id)
        .execute(pool)
        .await
        .map_err(|e| e.to_string())?;
    sqlx::query("DELETE FROM library_items WHERE id = ?")
        .bind(id)
        .execute(pool)
        .await
        .map_err(|e| e.to_string())?;
    Ok(())
}

pub async fn remove_library_item(pool: &SqlitePool, id: i64) -> Result<(), String> {
    delete_item_cascade(pool, id).await
}

pub async fn scan_library_directory(
    pool: &SqlitePool,
    dir_path: &str,
) -> Result<ScanResult, String> {
    use walkdir::WalkDir;

    let mut added: i64 = 0;
    let mut skipped: i64 = 0;
    let mut errors = Vec::new();

    for entry in WalkDir::new(dir_path).into_iter().filter_map(|e| e.ok()) {
        let path = entry.path();
        if !path.is_file() || !is_video_file(path) {
            continue;
        }

        let path_str = path.to_string_lossy().to_string();

        let exists =
            sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM library_items WHERE file_path = ?")
                .bind(&path_str)
                .fetch_one(pool)
                .await
                .unwrap_or(0);

        if exists > 0 {
            skipped += 1;
            continue;
        }

        let probe = probe_video_file(&path_str).await.unwrap_or(VideoProbe {
            file_size: None,
            duration_secs: None,
            video_codec: None,
            audio_codec: None,
            resolution: None,
        });

        match sqlx::query(
            "INSERT INTO library_items (file_path, file_size, duration_secs, video_codec, audio_codec, resolution)
             VALUES (?, ?, ?, ?, ?, ?)",
        )
        .bind(&path_str)
        .bind(probe.file_size)
        .bind(probe.duration_secs)
        .bind(&probe.video_codec)
        .bind(&probe.audio_codec)
        .bind(&probe.resolution)
        .execute(pool)
        .await
        {
            Ok(_) => added += 1,
            Err(e) => errors.push(format!("{}: {}", path_str, e)),
        }
    }

    Ok(ScanResult {
        added,
        skipped,
        errors,
    })
}

pub async fn add_library_asset(
    pool: &SqlitePool,
    item_id: i64,
    asset_type: &str,
    file_path: &str,
    lang: Option<&str>,
) -> Result<i64, String> {
    sqlx::query(
        "INSERT INTO library_assets (item_id, asset_type, file_path, lang) VALUES (?, ?, ?, ?)",
    )
    .bind(item_id)
    .bind(asset_type)
    .bind(file_path)
    .bind(lang)
    .execute(pool)
    .await
    .map(|r| r.last_insert_rowid())
    .map_err(|e| e.to_string())
}

pub async fn remove_library_asset(pool: &SqlitePool, id: i64) -> Result<(), String> {
    sqlx::query("DELETE FROM library_assets WHERE id = ?")
        .bind(id)
        .execute(pool)
        .await
        .map_err(|e| e.to_string())
        .map(|_| ())
}

pub async fn get_library_stats(pool: &SqlitePool) -> Result<LibraryStats, String> {
    let total_items: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM library_items")
        .fetch_one(pool)
        .await
        .map_err(|e| e.to_string())?;

    let total_file_size: i64 =
        sqlx::query_scalar("SELECT COALESCE(SUM(file_size), 0) FROM library_items")
            .fetch_one(pool)
            .await
            .map_err(|e| e.to_string())?;

    let by_resolution = sqlx::query_as::<_, StatEntry>(
        "SELECT COALESCE(resolution, '未知') AS label, COUNT(*) AS count
         FROM library_items GROUP BY resolution ORDER BY count DESC",
    )
    .fetch_all(pool)
    .await
    .map_err(|e| e.to_string())?;

    Ok(LibraryStats {
        total_items,
        total_file_size,
        by_resolution,
    })
}

/// Delete a media resource: removes disk file + DB records.
/// If deleting an SRT file, also cleans up any cached overlay ASS files.
pub async fn delete_library_resource(pool: &SqlitePool, file_path: &str) -> Result<(), String> {
    let path = Path::new(file_path);
    let is_srt = path
        .extension()
        .is_some_and(|ext| ext.eq_ignore_ascii_case("srt"));

    match std::fs::remove_file(file_path) {
        Ok(()) | Err(_) => {}
    }

    if is_srt && let Some(dir) = path.parent() {
        crate::subtitle::parser::cleanup_stale_overlays(dir);
    }

    let item_id: Option<i64> =
        sqlx::query_scalar("SELECT id FROM library_items WHERE file_path = ?")
            .bind(file_path)
            .fetch_optional(pool)
            .await
            .map_err(|e| e.to_string())?;

    if let Some(id) = item_id {
        delete_item_cascade(pool, id).await?;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    async fn setup_pool() -> SqlitePool {
        let pool = SqlitePool::connect(":memory:").await.unwrap();
        crate::infra::db::MIGRATOR.run(&pool).await.unwrap();
        pool
    }

    #[tokio::test]
    async fn test_add_and_list_library_items() {
        let pool = setup_pool().await;
        sqlx::query(
            "INSERT INTO library_items (file_path, file_size, duration_secs, video_codec, audio_codec, resolution)
             VALUES (?, ?, ?, ?, ?, ?)",
        )
        .bind("/movies/test.mp4")
        .bind(1073741824_i64)
        .bind(7200_i64)
        .bind("h264")
        .bind("aac")
        .bind("1920x1080")
        .execute(&pool)
        .await
        .unwrap();

        let items = list_library_items(&pool).await.unwrap();

        assert_eq!(items.len(), 1);
        assert_eq!(items[0].file_path, "/movies/test.mp4");
        assert_eq!(items[0].file_size, Some(1073741824));
    }

    #[tokio::test]
    async fn test_remove_item_cascades_assets() {
        let pool = setup_pool().await;
        sqlx::query("INSERT INTO library_items (file_path) VALUES (?)")
            .bind("/movies/test.mp4")
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query("INSERT INTO library_assets (item_id, asset_type, file_path) VALUES (?, ?, ?)")
            .bind(1_i64)
            .bind("subtitle")
            .bind("/movies/test.srt")
            .execute(&pool)
            .await
            .unwrap();

        delete_item_cascade(&pool, 1).await.unwrap();

        let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM library_items")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(count, 0);
        let asset_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM library_assets")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(asset_count, 0);
    }

    #[tokio::test]
    async fn test_get_library_item_with_assets() {
        let pool = setup_pool().await;
        sqlx::query(
            "INSERT INTO library_items (file_path, video_codec, resolution) VALUES (?, ?, ?)",
        )
        .bind("/movies/test.mkv")
        .bind("hevc")
        .bind("3840x2160")
        .execute(&pool)
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO library_assets (item_id, asset_type, file_path, lang) VALUES (?, ?, ?, ?)",
        )
        .bind(1_i64)
        .bind("subtitle")
        .bind("/movies/test.zh.srt")
        .bind("zh")
        .execute(&pool)
        .await
        .unwrap();

        let detail = get_library_item(&pool, 1).await.unwrap();
        assert_eq!(detail.video_codec, Some("hevc".to_string()));
        assert_eq!(detail.assets.len(), 1);
        assert_eq!(detail.assets[0].lang, Some("zh".to_string()));
    }

    #[tokio::test]
    async fn test_library_stats() {
        let pool = setup_pool().await;
        sqlx::query(
            "INSERT INTO library_items (file_path, file_size, resolution) VALUES (?, ?, ?)",
        )
        .bind("/movies/film1.mkv")
        .bind(5000000_i64)
        .bind("1920x1080")
        .execute(&pool)
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO library_items (file_path, file_size, resolution) VALUES (?, ?, ?)",
        )
        .bind("/movies/unknown.mp4")
        .bind(3000000_i64)
        .bind("1280x720")
        .execute(&pool)
        .await
        .unwrap();

        let stats = get_library_stats(&pool).await.unwrap();
        assert_eq!(stats.total_items, 2);
        assert_eq!(stats.total_file_size, 8000000);
    }
}
