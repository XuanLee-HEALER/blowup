use sqlx::SqlitePool;
use tauri::Emitter;

use super::{
    LibraryAssetEntry, LibraryItemDetail, LibraryItemSummary, LibraryStats, ScanResult, StatEntry,
};
use crate::ffmpeg::FfmpegTool;

// ── Video probe helper ──────────────────────────────────────────

struct VideoProbe {
    file_size: Option<i64>,
    duration_secs: Option<i64>,
    video_codec: Option<String>,
    audio_codec: Option<String>,
    resolution: Option<String>,
}

const VIDEO_EXTENSIONS: &[&str] = &[
    "mp4", "mkv", "avi", "mov", "ts", "flv", "wmv", "webm", "m4v",
];

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

fn is_video_file(path: &std::path::Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .map(|e| VIDEO_EXTENSIONS.contains(&e.to_lowercase().as_str()))
        .unwrap_or(false)
}

// ── Library item commands ───────────────────────────────────────

#[tauri::command]
pub async fn add_library_item(
    file_path: String,
    pool: tauri::State<'_, SqlitePool>,
) -> Result<i64, String> {
    let probe = probe_video_file(&file_path).await.unwrap_or(VideoProbe {
        file_size: None,
        duration_secs: None,
        video_codec: None,
        audio_codec: None,
        resolution: None,
    });

    let result = sqlx::query(
        "INSERT INTO library_items (file_path, file_size, duration_secs, video_codec, audio_codec, resolution)
         VALUES (?, ?, ?, ?, ?, ?)",
    )
    .bind(&file_path)
    .bind(probe.file_size)
    .bind(probe.duration_secs)
    .bind(&probe.video_codec)
    .bind(&probe.audio_codec)
    .bind(&probe.resolution)
    .execute(pool.inner())
    .await
    .map_err(|e| e.to_string())?;

    Ok(result.last_insert_rowid())
}

#[tauri::command]
pub async fn list_library_items(
    pool: tauri::State<'_, SqlitePool>,
) -> Result<Vec<LibraryItemSummary>, String> {
    sqlx::query_as::<_, LibraryItemSummary>(
        "SELECT id, file_path, file_size, duration_secs,
                video_codec, audio_codec, resolution, added_at
         FROM library_items
         ORDER BY added_at DESC",
    )
    .fetch_all(pool.inner())
    .await
    .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_library_item(
    id: i64,
    pool: tauri::State<'_, SqlitePool>,
) -> Result<LibraryItemDetail, String> {
    let row = sqlx::query_as::<_, LibraryItemSummary>(
        "SELECT id, file_path, file_size, duration_secs,
                video_codec, audio_codec, resolution, added_at
         FROM library_items WHERE id = ?",
    )
    .bind(id)
    .fetch_one(pool.inner())
    .await
    .map_err(|e| e.to_string())?;

    let assets = sqlx::query_as::<_, LibraryAssetEntry>(
        "SELECT id, asset_type, file_path, lang, created_at
         FROM library_assets WHERE item_id = ? ORDER BY created_at",
    )
    .bind(id)
    .fetch_all(pool.inner())
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
async fn delete_item_cascade(pool: &SqlitePool, id: i64) -> Result<(), String> {
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

#[tauri::command]
pub async fn remove_library_item(
    id: i64,
    pool: tauri::State<'_, SqlitePool>,
) -> Result<(), String> {
    delete_item_cascade(pool.inner(), id).await
}

// ── Directory scan ──────────────────────────────────────────────

#[tauri::command]
pub async fn scan_library_directory(
    dir_path: String,
    pool: tauri::State<'_, SqlitePool>,
) -> Result<ScanResult, String> {
    use walkdir::WalkDir;

    let mut added: i64 = 0;
    let mut skipped: i64 = 0;
    let mut errors = Vec::new();

    for entry in WalkDir::new(&dir_path).into_iter().filter_map(|e| e.ok()) {
        let path = entry.path();
        if !path.is_file() || !is_video_file(path) {
            continue;
        }

        let path_str = path.to_string_lossy().to_string();

        let exists =
            sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM library_items WHERE file_path = ?")
                .bind(&path_str)
                .fetch_one(pool.inner())
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
        .execute(pool.inner())
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

// ── Library assets ──────────────────────────────────────────────

#[tauri::command]
pub async fn add_library_asset(
    item_id: i64,
    asset_type: String,
    file_path: String,
    lang: Option<String>,
    pool: tauri::State<'_, SqlitePool>,
) -> Result<i64, String> {
    let result = sqlx::query(
        "INSERT INTO library_assets (item_id, asset_type, file_path, lang) VALUES (?, ?, ?, ?)",
    )
    .bind(item_id)
    .bind(&asset_type)
    .bind(&file_path)
    .bind(&lang)
    .execute(pool.inner())
    .await
    .map_err(|e| e.to_string())?;

    Ok(result.last_insert_rowid())
}

#[tauri::command]
pub async fn remove_library_asset(
    id: i64,
    pool: tauri::State<'_, SqlitePool>,
) -> Result<(), String> {
    sqlx::query("DELETE FROM library_assets WHERE id = ?")
        .bind(id)
        .execute(pool.inner())
        .await
        .map_err(|e| e.to_string())?;
    Ok(())
}

// ── Library stats ───────────────────────────────────────────────

#[tauri::command]
pub async fn get_library_stats(pool: tauri::State<'_, SqlitePool>) -> Result<LibraryStats, String> {
    let total_items: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM library_items")
        .fetch_one(pool.inner())
        .await
        .map_err(|e| e.to_string())?;

    let total_file_size: i64 =
        sqlx::query_scalar("SELECT COALESCE(SUM(file_size), 0) FROM library_items")
            .fetch_one(pool.inner())
            .await
            .map_err(|e| e.to_string())?;

    let by_resolution = sqlx::query_as::<_, StatEntry>(
        "SELECT COALESCE(resolution, '未知') AS label, COUNT(*) AS count
         FROM library_items GROUP BY resolution ORDER BY count DESC",
    )
    .fetch_all(pool.inner())
    .await
    .map_err(|e| e.to_string())?;

    Ok(LibraryStats {
        total_items,
        total_file_size,
        by_resolution,
    })
}

// ── Library Index Commands ───────────────────────────────────────

#[tauri::command]
pub fn list_index_entries(
    index: tauri::State<'_, crate::library_index::LibraryIndex>,
) -> Result<Vec<crate::library_index::IndexEntry>, String> {
    Ok(index.list_entries())
}

#[tauri::command]
pub fn list_index_by_director(
    index: tauri::State<'_, crate::library_index::LibraryIndex>,
) -> Result<std::collections::BTreeMap<String, Vec<crate::library_index::IndexEntry>>, String> {
    Ok(index.list_by_director())
}

#[tauri::command]
pub fn search_index(
    index: tauri::State<'_, crate::library_index::LibraryIndex>,
    query: Option<String>,
    year_from: Option<u32>,
    year_to: Option<u32>,
    genre: Option<String>,
) -> Result<Vec<crate::library_index::IndexEntry>, String> {
    Ok(index.search(query.as_deref(), year_from, year_to, genre.as_deref()))
}

#[tauri::command]
pub fn rebuild_index(
    app: tauri::AppHandle,
    index: tauri::State<'_, crate::library_index::LibraryIndex>,
) -> Result<(), String> {
    index.rebuild_from_disk();
    if let Err(e) = app.emit("library:changed", ()) {
        tracing::warn!(error = %e, "failed to emit library:changed");
    }
    Ok(())
}

// ── Resource & Film directory deletion ─────────────────────────

/// Delete a single media resource: removes disk file + DB records.
#[tauri::command]
pub async fn delete_library_resource(
    file_path: String,
    pool: tauri::State<'_, SqlitePool>,
) -> Result<(), String> {
    match std::fs::remove_file(&file_path) {
        Ok(()) | Err(_) => {} // NotFound is fine — file may already be gone
    }

    let item_id: Option<i64> =
        sqlx::query_scalar("SELECT id FROM library_items WHERE file_path = ?")
            .bind(&file_path)
            .fetch_optional(pool.inner())
            .await
            .map_err(|e| e.to_string())?;

    if let Some(id) = item_id {
        delete_item_cascade(pool.inner(), id).await?;
    }

    Ok(())
}

/// Refresh an index entry's file list by re-scanning its directory.
#[tauri::command]
pub fn refresh_index_entry(
    app: tauri::AppHandle,
    tmdb_id: u64,
    index: tauri::State<'_, crate::library_index::LibraryIndex>,
) -> Result<(), String> {
    index.update_files(tmdb_id);
    if let Err(e) = app.emit("library:changed", ()) {
        tracing::warn!(error = %e, "failed to emit library:changed");
    }
    Ok(())
}

/// Delete a film's entire directory from disk + index.
#[tauri::command]
pub async fn delete_film_directory(
    app: tauri::AppHandle,
    tmdb_id: u64,
    index: tauri::State<'_, crate::library_index::LibraryIndex>,
) -> Result<(), String> {
    let entry = index
        .get_entry(tmdb_id)
        .ok_or_else(|| "索引中未找到该电影".to_string())?;

    let cfg = crate::config::load_config();
    let root_dir = shellexpand::tilde(&cfg.library.root_dir).to_string();
    let film_dir = format!("{}/{}", root_dir, entry.path);

    // Check if player is currently playing a file from this directory
    if let Some(current_file) = crate::player::get_current_file_path()
        && current_file.starts_with(&film_dir)
    {
        return Err("播放器正在播放该电影的文件，请先关闭播放器".to_string());
    }

    // Delete directory (NotFound is fine — may already be gone)
    match std::fs::remove_dir_all(&film_dir) {
        Ok(()) | Err(_) => {}
    }

    index.remove_entry(tmdb_id);
    if let Err(e) = app.emit("library:changed", ()) {
        tracing::warn!(error = %e, "failed to emit library:changed");
    }
    Ok(())
}

// ── Tests ───────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use sqlx::SqlitePool;

    async fn setup_pool() -> SqlitePool {
        let pool = SqlitePool::connect(":memory:").await.unwrap();
        sqlx::migrate!("./migrations").run(&pool).await.unwrap();
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

        let items: Vec<super::LibraryItemSummary> = sqlx::query_as(
            "SELECT id, file_path, file_size, duration_secs,
                    video_codec, audio_codec, resolution, added_at
             FROM library_items ORDER BY added_at DESC",
        )
        .fetch_all(&pool)
        .await
        .unwrap();

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

        sqlx::query("DELETE FROM library_assets WHERE item_id = 1")
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query("DELETE FROM library_items WHERE id = 1")
            .execute(&pool)
            .await
            .unwrap();

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

        let row: super::LibraryItemSummary = sqlx::query_as(
            "SELECT id, file_path, file_size, duration_secs,
                    video_codec, audio_codec, resolution, added_at
             FROM library_items WHERE id = ?",
        )
        .bind(1_i64)
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(row.video_codec, Some("hevc".to_string()));

        let assets: Vec<super::LibraryAssetEntry> = sqlx::query_as(
            "SELECT id, asset_type, file_path, lang, created_at
             FROM library_assets WHERE item_id = ? ORDER BY created_at",
        )
        .bind(1_i64)
        .fetch_all(&pool)
        .await
        .unwrap();
        assert_eq!(assets.len(), 1);
        assert_eq!(assets[0].lang, Some("zh".to_string()));
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

        let total: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM library_items")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(total, 2);

        let total_size: i64 =
            sqlx::query_scalar("SELECT COALESCE(SUM(file_size), 0) FROM library_items")
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(total_size, 8000000);
    }
}
