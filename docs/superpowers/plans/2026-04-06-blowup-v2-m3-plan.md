# Blowup v2 M3: Library Enhancement Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add film library management with local file tracking, directory scanning, filtered film browsing, and collection statistics — unlocking the `/library` route.

**Architecture:** New `library/items.rs` module handles library item CRUD, directory scanning (walkdir + ffprobe), asset management, and stats aggregation. Film filtering uses `sqlx::QueryBuilder` for dynamic SQL. The Library page follows the existing left-panel/right-panel layout with a stats bar, filter controls, and two tabs (Films / Unlinked Files).

**Tech Stack:** sqlx 0.8 (QueryBuilder), walkdir (directory traversal), ffprobe via existing `FfmpegTool` wrapper, React 19 + TypeScript, @tauri-apps/plugin-dialog (file/directory picker)

---

## Critical patterns (read before any task)

- **Pool access:** `pool.inner()` — NOT `&**pool`. Tauri v2 `State` only has one Deref level.
- **FromRow:** only derive on flat structs matching DB column names. Complex nested structs use only `Serialize`, built manually.
- **Runtime queries:** `sqlx::query_as::<_, T>("SQL")` — no compile-time DATABASE_URL.
- **Tests:** in-memory SQLite: `SqlitePool::connect(":memory:")` + `sqlx::migrate!("./migrations").run(&pool)`. Run from `src-tauri/` directory.
- **Frontend API calls:** use `useEffect(..., [deps])` for data loading, never `useState(() => { api })`.
- **Settings.tsx patterns:** uses `Section`/`Field` helper components — check file before modifying.
- **Error handling:** `Result<T, String>` for Tauri commands, `.map_err(|e| e.to_string())?`.

## File structure

| Action | File | Purpose |
|--------|------|---------|
| Create | `src-tauri/src/commands/library/items.rs` | Library items CRUD, scan, assets, stats, video probe |
| Modify | `src-tauri/src/commands/library/mod.rs` | New type definitions |
| Modify | `src-tauri/src/commands/library/films.rs` | Add `list_films_filtered` + fix `delete_film` cascade |
| Modify | `src-tauri/src/lib.rs` | Register 11 new commands |
| Modify | `src/lib/tauri.ts` | New types + invoke wrappers |
| Create | `src/pages/Library.tsx` | Library page (replace Placeholder) |
| Modify | `src/App.tsx` | Enable `/library` route, import Library |

---

### Task 1: Data types in mod.rs

**Files:**
- Modify: `src-tauri/src/commands/library/mod.rs`

- [ ] **Step 1: Read `mod.rs`**

Read `src-tauri/src/commands/library/mod.rs` to see current types and `pub mod` declarations.

- [ ] **Step 2: Add `pub mod items` and new types**

Add `pub mod items;` to the module declarations, then append these types after the existing `GraphLink` struct:

```rust
// ── Library Items ────────────────────────────────────────────────

#[derive(Serialize, sqlx::FromRow)]
pub struct LibraryItemSummary {
    pub id: i64,
    pub film_id: Option<i64>,
    pub file_path: String,
    pub file_size: Option<i64>,
    pub duration_secs: Option<i64>,
    pub video_codec: Option<String>,
    pub audio_codec: Option<String>,
    pub resolution: Option<String>,
    pub added_at: String,
    pub film_title: Option<String>,
    pub film_year: Option<i64>,
}

#[derive(Serialize)]
pub struct LibraryItemDetail {
    pub id: i64,
    pub film_id: Option<i64>,
    pub file_path: String,
    pub file_size: Option<i64>,
    pub duration_secs: Option<i64>,
    pub video_codec: Option<String>,
    pub audio_codec: Option<String>,
    pub resolution: Option<String>,
    pub added_at: String,
    pub film_title: Option<String>,
    pub film_year: Option<i64>,
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
    pub total_films: i64,
    pub films_with_files: i64,
    pub total_file_size: i64,
    pub unlinked_files: i64,
    pub by_decade: Vec<StatEntry>,
    pub by_genre: Vec<StatEntry>,
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

#[derive(Serialize, sqlx::FromRow)]
pub struct FilmListEntry {
    pub id: i64,
    pub title: String,
    pub original_title: Option<String>,
    pub year: Option<i64>,
    pub tmdb_rating: Option<f64>,
    pub poster_cache_path: Option<String>,
    pub has_file: i64,
}

#[derive(Serialize)]
pub struct FilmFilterResult {
    pub films: Vec<FilmListEntry>,
    pub total: i64,
    pub page: i64,
    pub page_size: i64,
}
```

- [ ] **Step 3: Verify build**

Run: `cd src-tauri && cargo check 2>&1`
Expected: compiles (items module is declared but file doesn't exist yet — create an empty file to satisfy the compiler)

Create `src-tauri/src/commands/library/items.rs` with just:
```rust
// Library items commands — implemented in Task 2
```

- [ ] **Step 4: Commit**

```bash
cd C:/Users/lixuan/Documents/code/rust/blowup
git add src-tauri/src/commands/library/mod.rs src-tauri/src/commands/library/items.rs
git commit -m "feat: add library item, stats, and film filter data types"
```

---

### Task 2: Library items CRUD + video probe + tests

**Files:**
- Create: `src-tauri/src/commands/library/items.rs` (replace placeholder)

This task implements: `add_library_item`, `list_library_items`, `get_library_item`, `link_item_to_film`, `unlink_item_from_film`, `remove_library_item`, and a private `probe_video_file` helper.

**Key pattern:** `probe_video_file` uses the existing `FfmpegTool::Ffprobe` from `crate::ffmpeg`. It runs `ffprobe -v quiet -print_format json -show_format -show_streams -- <path>` and parses the JSON to extract file_size, duration, codecs, and resolution. If probe fails, the item is still added with NULL metadata.

- [ ] **Step 1: Write tests**

Replace `src-tauri/src/commands/library/items.rs` with the full implementation including tests at the bottom. Start with the tests:

```rust
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
            "SELECT li.id, li.film_id, li.file_path, li.file_size, li.duration_secs,
                    li.video_codec, li.audio_codec, li.resolution, li.added_at,
                    f.title AS film_title, f.year AS film_year
             FROM library_items li LEFT JOIN films f ON li.film_id = f.id
             ORDER BY li.added_at DESC",
        )
        .fetch_all(&pool)
        .await
        .unwrap();

        assert_eq!(items.len(), 1);
        assert_eq!(items[0].file_path, "/movies/test.mp4");
        assert_eq!(items[0].file_size, Some(1073741824));
        assert_eq!(items[0].film_id, None);
    }

    #[tokio::test]
    async fn test_link_and_unlink_item() {
        let pool = setup_pool().await;
        sqlx::query("INSERT INTO films (title, year) VALUES (?, ?)")
            .bind("Test Film")
            .bind(2024_i64)
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query("INSERT INTO library_items (file_path) VALUES (?)")
            .bind("/movies/test.mp4")
            .execute(&pool)
            .await
            .unwrap();

        // Link
        sqlx::query("UPDATE library_items SET film_id = 1 WHERE id = 1")
            .execute(&pool)
            .await
            .unwrap();
        let film_id: Option<i64> =
            sqlx::query_scalar("SELECT film_id FROM library_items WHERE id = 1")
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(film_id, Some(1));

        // Unlink
        sqlx::query("UPDATE library_items SET film_id = NULL WHERE id = 1")
            .execute(&pool)
            .await
            .unwrap();
        let film_id: Option<i64> =
            sqlx::query_scalar("SELECT film_id FROM library_items WHERE id = 1")
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(film_id, None);
    }

    #[tokio::test]
    async fn test_remove_item_cascades_assets() {
        let pool = setup_pool().await;
        sqlx::query("INSERT INTO library_items (file_path) VALUES (?)")
            .bind("/movies/test.mp4")
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query(
            "INSERT INTO library_assets (item_id, asset_type, file_path) VALUES (?, ?, ?)",
        )
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
            "SELECT li.id, li.film_id, li.file_path, li.file_size, li.duration_secs,
                    li.video_codec, li.audio_codec, li.resolution, li.added_at,
                    f.title AS film_title, f.year AS film_year
             FROM library_items li LEFT JOIN films f ON li.film_id = f.id
             WHERE li.id = ?",
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
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cd src-tauri && cargo test --lib commands::library::items::tests 2>&1`
Expected: FAIL — the `super::LibraryItemSummary` type exists but no implementation code yet.

Actually these tests use raw SQL so they should pass even without command implementations. The tests verify the data layer, not the Tauri commands. They should compile and pass since the types are in mod.rs. Run to verify.

- [ ] **Step 3: Write the full items.rs implementation**

Write the complete `src-tauri/src/commands/library/items.rs`:

```rust
use sqlx::SqlitePool;

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
    let args: Vec<String> = vec![
        "-v", "quiet", "-print_format", "json", "-show_format", "-show_streams", "--", path,
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
    let file_size = format["size"]
        .as_str()
        .and_then(|s| s.parse::<i64>().ok());
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
    film_id: Option<i64>,
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
        "INSERT INTO library_items (film_id, file_path, file_size, duration_secs, video_codec, audio_codec, resolution)
         VALUES (?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(film_id)
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
        "SELECT li.id, li.film_id, li.file_path, li.file_size, li.duration_secs,
                li.video_codec, li.audio_codec, li.resolution, li.added_at,
                f.title AS film_title, f.year AS film_year
         FROM library_items li
         LEFT JOIN films f ON li.film_id = f.id
         ORDER BY li.added_at DESC",
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
        "SELECT li.id, li.film_id, li.file_path, li.file_size, li.duration_secs,
                li.video_codec, li.audio_codec, li.resolution, li.added_at,
                f.title AS film_title, f.year AS film_year
         FROM library_items li
         LEFT JOIN films f ON li.film_id = f.id
         WHERE li.id = ?",
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
        film_id: row.film_id,
        file_path: row.file_path,
        file_size: row.file_size,
        duration_secs: row.duration_secs,
        video_codec: row.video_codec,
        audio_codec: row.audio_codec,
        resolution: row.resolution,
        added_at: row.added_at,
        film_title: row.film_title,
        film_year: row.film_year,
        assets,
    })
}

#[tauri::command]
pub async fn link_item_to_film(
    item_id: i64,
    film_id: i64,
    pool: tauri::State<'_, SqlitePool>,
) -> Result<(), String> {
    sqlx::query("UPDATE library_items SET film_id = ? WHERE id = ?")
        .bind(film_id)
        .bind(item_id)
        .execute(pool.inner())
        .await
        .map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub async fn unlink_item_from_film(
    item_id: i64,
    pool: tauri::State<'_, SqlitePool>,
) -> Result<(), String> {
    sqlx::query("UPDATE library_items SET film_id = NULL WHERE id = ?")
        .bind(item_id)
        .execute(pool.inner())
        .await
        .map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub async fn remove_library_item(
    id: i64,
    pool: tauri::State<'_, SqlitePool>,
) -> Result<(), String> {
    sqlx::query("DELETE FROM library_assets WHERE item_id = ?")
        .bind(id)
        .execute(pool.inner())
        .await
        .map_err(|e| e.to_string())?;
    sqlx::query("DELETE FROM library_items WHERE id = ?")
        .bind(id)
        .execute(pool.inner())
        .await
        .map_err(|e| e.to_string())?;
    Ok(())
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

    for entry in WalkDir::new(&dir_path)
        .into_iter()
        .filter_map(|e| e.ok())
    {
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
pub async fn get_library_stats(
    pool: tauri::State<'_, SqlitePool>,
) -> Result<LibraryStats, String> {
    let total_films = sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM films")
        .fetch_one(pool.inner())
        .await
        .map_err(|e| e.to_string())?;

    let films_with_files = sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(DISTINCT film_id) FROM library_items WHERE film_id IS NOT NULL",
    )
    .fetch_one(pool.inner())
    .await
    .map_err(|e| e.to_string())?;

    let total_file_size =
        sqlx::query_scalar::<_, i64>("SELECT COALESCE(SUM(file_size), 0) FROM library_items")
            .fetch_one(pool.inner())
            .await
            .map_err(|e| e.to_string())?;

    let unlinked_files = sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM library_items WHERE film_id IS NULL",
    )
    .fetch_one(pool.inner())
    .await
    .map_err(|e| e.to_string())?;

    let by_decade = sqlx::query_as::<_, StatEntry>(
        "SELECT (year / 10 * 10) || 's' AS label, COUNT(*) AS count
         FROM films WHERE year IS NOT NULL
         GROUP BY year / 10 ORDER BY label",
    )
    .fetch_all(pool.inner())
    .await
    .map_err(|e| e.to_string())?;

    let by_genre = sqlx::query_as::<_, StatEntry>(
        "SELECT g.name AS label, COUNT(*) AS count
         FROM film_genres fg JOIN genres g ON fg.genre_id = g.id
         GROUP BY g.id ORDER BY count DESC LIMIT 10",
    )
    .fetch_all(pool.inner())
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
        total_films,
        films_with_files,
        total_file_size,
        unlinked_files,
        by_decade,
        by_genre,
        by_resolution,
    })
}
// ── Tests (paste from Step 1 here) ──────────────────────────────
```

**IMPORTANT:** Paste the `#[cfg(test)] mod tests` block from Step 1 at the bottom of this file.

- [ ] **Step 4: Run tests**

Run: `cd src-tauri && cargo test --lib commands::library::items::tests -- --nocapture 2>&1`
Expected: 4 tests pass

- [ ] **Step 5: Commit**

```bash
cd C:/Users/lixuan/Documents/code/rust/blowup
git add src-tauri/src/commands/library/items.rs
git commit -m "feat: add library items CRUD, scan, assets, and stats commands"
```

---

### Task 3: Film list filtering command + tests

**Files:**
- Modify: `src-tauri/src/commands/library/films.rs`

This task adds `list_films_filtered` — a dynamic SQL query using `sqlx::QueryBuilder` with optional filters: text search, genre, year range, rating, has-file status, sorting, and pagination.

- [ ] **Step 1: Read `films.rs`**

Read `src-tauri/src/commands/library/films.rs` to see existing commands and test module.

- [ ] **Step 2: Add tests to the existing test module**

Append these tests inside the existing `#[cfg(test)] mod tests` block at the bottom of `films.rs`:

```rust
    #[tokio::test]
    async fn test_list_films_filtered_by_query() {
        let pool = setup_pool().await;
        sqlx::query("INSERT INTO films (title, year) VALUES (?, ?)")
            .bind("Blow-Up").bind(1966_i64).execute(&pool).await.unwrap();
        sqlx::query("INSERT INTO films (title, year) VALUES (?, ?)")
            .bind("Stalker").bind(1979_i64).execute(&pool).await.unwrap();
        sqlx::query("INSERT INTO films (title, year) VALUES (?, ?)")
            .bind("Blowout").bind(1981_i64).execute(&pool).await.unwrap();

        let result = super::list_films_filtered_inner(
            &pool,
            Some("Blow".to_string()), None, None, None, None, None, None, None, None, None,
        ).await.unwrap();

        assert_eq!(result.films.len(), 2);
        assert_eq!(result.total, 2);
    }

    #[tokio::test]
    async fn test_list_films_filtered_by_year_range() {
        let pool = setup_pool().await;
        sqlx::query("INSERT INTO films (title, year) VALUES (?, ?)")
            .bind("Film A").bind(1960_i64).execute(&pool).await.unwrap();
        sqlx::query("INSERT INTO films (title, year) VALUES (?, ?)")
            .bind("Film B").bind(1975_i64).execute(&pool).await.unwrap();
        sqlx::query("INSERT INTO films (title, year) VALUES (?, ?)")
            .bind("Film C").bind(1990_i64).execute(&pool).await.unwrap();

        let result = super::list_films_filtered_inner(
            &pool,
            None, None, Some(1970), Some(1980), None, None, None, None, None, None,
        ).await.unwrap();

        assert_eq!(result.films.len(), 1);
        assert_eq!(result.films[0].title, "Film B");
    }

    #[tokio::test]
    async fn test_list_films_filtered_has_file() {
        let pool = setup_pool().await;
        sqlx::query("INSERT INTO films (title) VALUES (?)").bind("Film A").execute(&pool).await.unwrap();
        sqlx::query("INSERT INTO films (title) VALUES (?)").bind("Film B").execute(&pool).await.unwrap();
        sqlx::query("INSERT INTO library_items (film_id, file_path) VALUES (?, ?)")
            .bind(1_i64).bind("/a.mp4").execute(&pool).await.unwrap();

        let with_file = super::list_films_filtered_inner(
            &pool,
            None, None, None, None, None, Some(true), None, None, None, None,
        ).await.unwrap();
        assert_eq!(with_file.films.len(), 1);
        assert_eq!(with_file.films[0].title, "Film A");

        let without_file = super::list_films_filtered_inner(
            &pool,
            None, None, None, None, None, Some(false), None, None, None, None,
        ).await.unwrap();
        assert_eq!(without_file.films.len(), 1);
        assert_eq!(without_file.films[0].title, "Film B");
    }

    #[tokio::test]
    async fn test_list_films_filtered_pagination() {
        let pool = setup_pool().await;
        for i in 1..=25 {
            sqlx::query("INSERT INTO films (title, year) VALUES (?, ?)")
                .bind(format!("Film {}", i)).bind(2000_i64 + i)
                .execute(&pool).await.unwrap();
        }

        let page1 = super::list_films_filtered_inner(
            &pool,
            None, None, None, None, None, None, None, None, Some(1), Some(10),
        ).await.unwrap();
        assert_eq!(page1.films.len(), 10);
        assert_eq!(page1.total, 25);
        assert_eq!(page1.page, 1);
        assert_eq!(page1.page_size, 10);

        let page3 = super::list_films_filtered_inner(
            &pool,
            None, None, None, None, None, None, None, None, Some(3), Some(10),
        ).await.unwrap();
        assert_eq!(page3.films.len(), 5);
    }
```

- [ ] **Step 3: Implement `list_films_filtered` and `list_films_filtered_inner`**

Add these functions and the `use` import at the top of `films.rs`:

At the top, add to imports:
```rust
use sqlx::QueryBuilder;
use super::{FilmListEntry, FilmFilterResult};
```

Then add these functions (before or after existing commands):

```rust
fn apply_film_filters(
    qb: &mut QueryBuilder<'_, sqlx::Sqlite>,
    query: &Option<String>,
    genre_id: Option<i64>,
    year_from: Option<i64>,
    year_to: Option<i64>,
    min_rating: Option<f64>,
    has_file: Option<bool>,
) {
    if let Some(ref q) = query {
        if !q.is_empty() {
            qb.push(" AND (f.title LIKE '%' || ");
            qb.push_bind(q.clone());
            qb.push(" || '%' OR f.original_title LIKE '%' || ");
            qb.push_bind(q.clone());
            qb.push(" || '%')");
        }
    }
    if let Some(gid) = genre_id {
        qb.push(" AND fg.genre_id = ");
        qb.push_bind(gid);
    }
    if let Some(yf) = year_from {
        qb.push(" AND f.year >= ");
        qb.push_bind(yf);
    }
    if let Some(yt) = year_to {
        qb.push(" AND f.year <= ");
        qb.push_bind(yt);
    }
    if let Some(mr) = min_rating {
        qb.push(" AND f.tmdb_rating >= ");
        qb.push_bind(mr);
    }
    if let Some(hf) = has_file {
        if hf {
            qb.push(" AND EXISTS(SELECT 1 FROM library_items li WHERE li.film_id = f.id)");
        } else {
            qb.push(" AND NOT EXISTS(SELECT 1 FROM library_items li WHERE li.film_id = f.id)");
        }
    }
}

/// Inner function testable without tauri::State
pub(crate) async fn list_films_filtered_inner(
    pool: &SqlitePool,
    query: Option<String>,
    genre_id: Option<i64>,
    year_from: Option<i64>,
    year_to: Option<i64>,
    min_rating: Option<f64>,
    has_file: Option<bool>,
    sort_by: Option<String>,
    sort_desc: Option<bool>,
    page: Option<i64>,
    page_size: Option<i64>,
) -> Result<FilmFilterResult, String> {
    let pg = page.unwrap_or(1).max(1);
    let ps = page_size.unwrap_or(20).clamp(1, 100);
    let offset = (pg - 1) * ps;

    // Count query
    let mut count_qb = QueryBuilder::<sqlx::Sqlite>::new(
        "SELECT COUNT(DISTINCT f.id) FROM films f",
    );
    if genre_id.is_some() {
        count_qb.push(" INNER JOIN film_genres fg ON f.id = fg.film_id");
    }
    count_qb.push(" WHERE 1=1");
    apply_film_filters(&mut count_qb, &query, genre_id, year_from, year_to, min_rating, has_file);
    let (total,): (i64,) = count_qb
        .build_query_as()
        .fetch_one(pool)
        .await
        .map_err(|e| e.to_string())?;

    // Data query
    let mut qb = QueryBuilder::<sqlx::Sqlite>::new(
        "SELECT DISTINCT f.id, f.title, f.original_title, f.year, f.tmdb_rating, f.poster_cache_path, \
         CASE WHEN EXISTS(SELECT 1 FROM library_items li WHERE li.film_id = f.id) THEN 1 ELSE 0 END AS has_file \
         FROM films f",
    );
    if genre_id.is_some() {
        qb.push(" INNER JOIN film_genres fg ON f.id = fg.film_id");
    }
    qb.push(" WHERE 1=1");
    apply_film_filters(&mut qb, &query, genre_id, year_from, year_to, min_rating, has_file);

    let order = match sort_by.as_deref() {
        Some("title") => "f.title",
        Some("year") => "f.year",
        Some("rating") => "f.tmdb_rating",
        _ => "f.created_at",
    };
    let dir = if sort_desc.unwrap_or(true) { "DESC" } else { "ASC" };
    qb.push(format!(" ORDER BY {order} {dir} NULLS LAST"));
    qb.push(" LIMIT ");
    qb.push_bind(ps);
    qb.push(" OFFSET ");
    qb.push_bind(offset);

    let films: Vec<FilmListEntry> = qb
        .build_query_as()
        .fetch_all(pool)
        .await
        .map_err(|e| e.to_string())?;

    Ok(FilmFilterResult {
        films,
        total,
        page: pg,
        page_size: ps,
    })
}

#[tauri::command]
pub async fn list_films_filtered(
    pool: tauri::State<'_, SqlitePool>,
    query: Option<String>,
    genre_id: Option<i64>,
    year_from: Option<i64>,
    year_to: Option<i64>,
    min_rating: Option<f64>,
    has_file: Option<bool>,
    sort_by: Option<String>,
    sort_desc: Option<bool>,
    page: Option<i64>,
    page_size: Option<i64>,
) -> Result<FilmFilterResult, String> {
    list_films_filtered_inner(
        pool.inner(), query, genre_id, year_from, year_to, min_rating, has_file,
        sort_by, sort_desc, page, page_size,
    )
    .await
}
```

**Note:** The `_inner` pattern lets tests call the logic directly with `&SqlitePool` instead of `tauri::State`.

- [ ] **Step 4: Fix `delete_film` cascade behavior**

In `delete_film`, change the `library_items` line from DELETE to UPDATE (preserve local files, just unlink them):

Change:
```rust
sqlx::query("DELETE FROM library_items WHERE film_id = ?").bind(id).execute(pool.inner()).await.map_err(|e| e.to_string())?;
```
To:
```rust
sqlx::query("UPDATE library_items SET film_id = NULL WHERE film_id = ?").bind(id).execute(pool.inner()).await.map_err(|e| e.to_string())?;
```

- [ ] **Step 5: Run tests**

Run: `cd src-tauri && cargo test --lib commands::library::films::tests -- --nocapture 2>&1`
Expected: All existing + new tests pass

- [ ] **Step 6: Commit**

```bash
cd C:/Users/lixuan/Documents/code/rust/blowup
git add src-tauri/src/commands/library/films.rs
git commit -m "feat: add film list filtering with dynamic SQL and fix delete_film cascade"
```

---

### Task 4: Register all new commands + full test suite

**Files:**
- Modify: `src-tauri/src/lib.rs`

- [ ] **Step 1: Read `lib.rs`**

Read `src-tauri/src/lib.rs` to see current command registration.

- [ ] **Step 2: Add new command imports and register**

Add 11 new commands to the `invoke_handler`. The new commands from `library::items` are:
- `add_library_item`
- `list_library_items`
- `get_library_item`
- `link_item_to_film`
- `unlink_item_from_film`
- `remove_library_item`
- `scan_library_directory`
- `add_library_asset`
- `remove_library_asset`
- `get_library_stats`

And from `library::films`:
- `list_films_filtered`

Add these to the `tauri::generate_handler![]` macro. Example placement (after existing library commands):

```rust
// In the generate_handler! macro, add:
commands::library::items::add_library_item,
commands::library::items::list_library_items,
commands::library::items::get_library_item,
commands::library::items::link_item_to_film,
commands::library::items::unlink_item_from_film,
commands::library::items::remove_library_item,
commands::library::items::scan_library_directory,
commands::library::items::add_library_asset,
commands::library::items::remove_library_asset,
commands::library::items::get_library_stats,
commands::library::films::list_films_filtered,
```

- [ ] **Step 3: Run full test suite**

Run: `cd src-tauri && cargo test 2>&1`
Expected: All tests pass (57 existing + ~8 new = ~65 tests)

- [ ] **Step 4: Run cargo check for clean build**

Run: `cd src-tauri && cargo check 2>&1`
Expected: No warnings/errors

- [ ] **Step 5: Commit**

```bash
cd C:/Users/lixuan/Documents/code/rust/blowup
git add src-tauri/src/lib.rs
git commit -m "feat: register all M3 library commands (11 new, 46 total)"
```

---

### Task 5: tauri.ts API layer extensions

**Files:**
- Modify: `src/lib/tauri.ts`

- [ ] **Step 1: Read `tauri.ts`**

Read `src/lib/tauri.ts` to see current types and the `library` object.

- [ ] **Step 2: Add new types**

Add these type definitions after the existing types (before the `library` object):

```typescript
export interface LibraryItemSummary {
  id: number;
  film_id: number | null;
  file_path: string;
  file_size: number | null;
  duration_secs: number | null;
  video_codec: string | null;
  audio_codec: string | null;
  resolution: string | null;
  added_at: string;
  film_title: string | null;
  film_year: number | null;
}

export interface LibraryItemDetail extends LibraryItemSummary {
  assets: LibraryAssetEntry[];
}

export interface LibraryAssetEntry {
  id: number;
  asset_type: string;
  file_path: string;
  lang: string | null;
  created_at: string;
}

export interface LibraryStats {
  total_films: number;
  films_with_files: number;
  total_file_size: number;
  unlinked_files: number;
  by_decade: StatEntry[];
  by_genre: StatEntry[];
  by_resolution: StatEntry[];
}

export interface StatEntry {
  label: string;
  count: number;
}

export interface ScanResult {
  added: number;
  skipped: number;
  errors: string[];
}

export interface FilmListEntry {
  id: number;
  title: string;
  original_title: string | null;
  year: number | null;
  tmdb_rating: number | null;
  poster_cache_path: string | null;
  has_file: number;
}

export interface FilmFilterResult {
  films: FilmListEntry[];
  total: number;
  page: number;
  page_size: number;
}

export interface FilmFilterParams {
  query?: string;
  genreId?: number;
  yearFrom?: number;
  yearTo?: number;
  minRating?: number;
  hasFile?: boolean;
  sortBy?: string;
  sortDesc?: boolean;
  page?: number;
  pageSize?: number;
}
```

- [ ] **Step 3: Add invoke wrappers**

Add these wrappers inside the `library` object (after existing `getGraphData`):

```typescript
  // Library items
  listLibraryItems: () =>
    invoke<LibraryItemSummary[]>("list_library_items"),
  getLibraryItem: (id: number) =>
    invoke<LibraryItemDetail>("get_library_item", { id }),
  addLibraryItem: (filePath: string, filmId?: number) =>
    invoke<number>("add_library_item", { filePath, filmId }),
  linkItemToFilm: (itemId: number, filmId: number) =>
    invoke<void>("link_item_to_film", { itemId, filmId }),
  unlinkItemFromFilm: (itemId: number) =>
    invoke<void>("unlink_item_from_film", { itemId }),
  removeLibraryItem: (id: number) =>
    invoke<void>("remove_library_item", { id }),
  scanLibraryDirectory: (dirPath: string) =>
    invoke<ScanResult>("scan_library_directory", { dirPath }),
  addLibraryAsset: (itemId: number, assetType: string, filePath: string, lang?: string) =>
    invoke<number>("add_library_asset", { itemId, assetType, filePath, lang }),
  removeLibraryAsset: (id: number) =>
    invoke<void>("remove_library_asset", { id }),
  getLibraryStats: () =>
    invoke<LibraryStats>("get_library_stats"),
  listFilmsFiltered: (params: FilmFilterParams) =>
    invoke<FilmFilterResult>("list_films_filtered", params),
```

- [ ] **Step 4: Type check**

Run: `cd C:/Users/lixuan/Documents/code/rust/blowup && npx tsc --noEmit 2>&1`
Expected: 0 errors

- [ ] **Step 5: Commit**

```bash
cd C:/Users/lixuan/Documents/code/rust/blowup
git add src/lib/tauri.ts
git commit -m "feat: add M3 library types and invoke wrappers in tauri.ts"
```

---

### Task 6: Library page UI

**Files:**
- Create: `src/pages/Library.tsx`

This is the main frontend page. Layout: stats bar at top, tab bar (Films / Unlinked Files), left panel (filter + list), right panel (detail).

**Key UX flows:**
1. **Browse films** — filterable KB film grid with has-file badges
2. **Link a file** — select film → click "关联文件" → Tauri file dialog → probe + insert + link
3. **Scan directory** — click "扫描目录" → Tauri directory dialog → backend scans → refresh
4. **View unlinked files** — switch to "待关联文件" tab → see orphan files → link to films

- [ ] **Step 1: Read existing pages for patterns**

Read `src/pages/People.tsx` (lines 1-50) to understand the established page patterns: layout structure, modal helpers, state management.

- [ ] **Step 2: Write Library.tsx**

Create `src/pages/Library.tsx` with the following structure. The page follows the left-panel + right-panel pattern used by People.tsx and Genres.tsx.

```tsx
import { useState, useEffect, useCallback } from "react";
import { library } from "../lib/tauri";
import type {
  FilmListEntry,
  FilmFilterResult,
  FilmFilterParams,
  LibraryStats,
  LibraryItemSummary,
  LibraryItemDetail,
  GenreTreeNode,
} from "../lib/tauri";
import { open } from "@tauri-apps/plugin-dialog";

// ── Helpers ──────────────────────────────────────────────────────

function formatSize(bytes: number | null): string {
  if (!bytes) return "—";
  if (bytes >= 1e9) return (bytes / 1e9).toFixed(1) + " GB";
  if (bytes >= 1e6) return (bytes / 1e6).toFixed(0) + " MB";
  return (bytes / 1e3).toFixed(0) + " KB";
}

function formatDuration(secs: number | null): string {
  if (!secs) return "—";
  const h = Math.floor(secs / 3600);
  const m = Math.floor((secs % 3600) / 60);
  return h > 0 ? `${h}h${m}m` : `${m}m`;
}

function flattenGenres(
  nodes: GenreTreeNode[]
): { id: number; name: string }[] {
  const result: { id: number; name: string }[] = [];
  function walk(ns: GenreTreeNode[]) {
    for (const n of ns) {
      result.push({ id: n.id, name: n.name });
      walk(n.children);
    }
  }
  walk(nodes);
  return result;
}

// ── Stats Bar ────────────────────────────────────────────────────

function StatsBar({
  stats,
  onScan,
}: {
  stats: LibraryStats | null;
  onScan: () => void;
}) {
  if (!stats) return null;
  const sizeStr = formatSize(stats.total_file_size);
  const pct =
    stats.total_films > 0
      ? ((stats.films_with_files / stats.total_films) * 100).toFixed(0)
      : "0";
  return (
    <div
      style={{
        padding: "10px 16px",
        borderBottom: "1px solid var(--color-separator)",
        display: "flex",
        alignItems: "center",
        gap: 16,
        fontSize: 13,
        color: "var(--color-label-secondary)",
      }}
    >
      <span>{stats.total_films} 部影片</span>
      <span>{stats.films_with_files} 部已关联</span>
      <span>{sizeStr}</span>
      <span>{pct}%</span>
      {stats.unlinked_files > 0 && (
        <span style={{ color: "var(--color-accent)" }}>
          {stats.unlinked_files} 个待关联
        </span>
      )}
      <button
        onClick={onScan}
        style={{
          marginLeft: "auto",
          background: "var(--color-bg-control)",
          border: "1px solid var(--color-separator)",
          borderRadius: 6,
          padding: "4px 12px",
          color: "var(--color-label-primary)",
          cursor: "pointer",
          fontSize: 13,
        }}
      >
        扫描目录
      </button>
    </div>
  );
}

// ── Film Card ────────────────────────────────────────────────────

function FilmCard({
  film,
  selected,
  onClick,
}: {
  film: FilmListEntry;
  selected: boolean;
  onClick: () => void;
}) {
  return (
    <div
      onClick={onClick}
      style={{
        padding: "10px 16px",
        cursor: "pointer",
        borderBottom: "1px solid var(--color-separator)",
        background: selected ? "var(--color-bg-selected)" : "transparent",
        display: "flex",
        alignItems: "center",
        gap: 10,
      }}
    >
      <div style={{ flex: 1, minWidth: 0 }}>
        <div
          style={{
            fontWeight: 500,
            whiteSpace: "nowrap",
            overflow: "hidden",
            textOverflow: "ellipsis",
          }}
        >
          {film.title}
        </div>
        <div style={{ fontSize: 12, color: "var(--color-label-secondary)" }}>
          {film.year ?? "—"}{" "}
          {film.tmdb_rating ? `⭐ ${film.tmdb_rating.toFixed(1)}` : ""}
        </div>
      </div>
      <span
        style={{
          fontSize: 14,
          color: film.has_file
            ? "var(--color-accent)"
            : "var(--color-label-tertiary)",
        }}
      >
        {film.has_file ? "✓" : "✗"}
      </span>
    </div>
  );
}

// ── File Card ────────────────────────────────────────────────────

function FileCard({
  item,
  selected,
  onClick,
}: {
  item: LibraryItemSummary;
  selected: boolean;
  onClick: () => void;
}) {
  const fileName = item.file_path.split(/[/\\]/).pop() ?? item.file_path;
  return (
    <div
      onClick={onClick}
      style={{
        padding: "10px 16px",
        cursor: "pointer",
        borderBottom: "1px solid var(--color-separator)",
        background: selected ? "var(--color-bg-selected)" : "transparent",
      }}
    >
      <div
        style={{
          fontWeight: 500,
          whiteSpace: "nowrap",
          overflow: "hidden",
          textOverflow: "ellipsis",
          fontSize: 13,
        }}
      >
        {fileName}
      </div>
      <div style={{ fontSize: 12, color: "var(--color-label-secondary)" }}>
        {formatSize(item.file_size)} · {item.resolution ?? "—"} ·{" "}
        {item.video_codec ?? "—"}
      </div>
    </div>
  );
}

// ── Film Detail Panel ────────────────────────────────────────────

function FilmDetailView({
  film,
  items,
  onLink,
  onUnlink,
  onRemoveItem,
  onRefresh,
}: {
  film: FilmListEntry;
  items: LibraryItemSummary[];
  onLink: () => void;
  onUnlink: (itemId: number) => void;
  onRemoveItem: (itemId: number) => void;
  onRefresh: () => void;
}) {
  const linkedItems = items.filter((i) => i.film_id === film.id);
  return (
    <div>
      <h2 style={{ margin: "0 0 4px" }}>{film.title}</h2>
      {film.original_title && (
        <div
          style={{
            color: "var(--color-label-secondary)",
            fontSize: 14,
            marginBottom: 4,
          }}
        >
          {film.original_title}
        </div>
      )}
      <div style={{ color: "var(--color-label-secondary)", fontSize: 13, marginBottom: 16 }}>
        {film.year ?? "—"} · {film.tmdb_rating ? `⭐ ${film.tmdb_rating.toFixed(1)}` : "未评分"}
      </div>

      <h3 style={{ fontSize: 14, marginBottom: 8 }}>本地文件</h3>
      {linkedItems.length === 0 ? (
        <div style={{ color: "var(--color-label-tertiary)", fontSize: 13, marginBottom: 8 }}>
          暂无关联文件
        </div>
      ) : (
        linkedItems.map((item) => (
          <div
            key={item.id}
            style={{
              background: "var(--color-bg-control)",
              borderRadius: 8,
              padding: 12,
              marginBottom: 8,
              fontSize: 13,
            }}
          >
            <div style={{ wordBreak: "break-all", marginBottom: 6 }}>
              {item.file_path.split(/[/\\]/).pop()}
            </div>
            <div style={{ color: "var(--color-label-secondary)", display: "flex", gap: 12 }}>
              <span>{formatSize(item.file_size)}</span>
              <span>{item.resolution ?? "—"}</span>
              <span>{item.video_codec ?? "—"}/{item.audio_codec ?? "—"}</span>
              <span>{formatDuration(item.duration_secs)}</span>
            </div>
            <div style={{ marginTop: 8, display: "flex", gap: 8 }}>
              <button
                onClick={() => onUnlink(item.id)}
                style={{
                  background: "none",
                  border: "1px solid var(--color-separator)",
                  borderRadius: 4,
                  padding: "2px 8px",
                  color: "var(--color-label-secondary)",
                  cursor: "pointer",
                  fontSize: 12,
                }}
              >
                取消关联
              </button>
              <button
                onClick={() => onRemoveItem(item.id)}
                style={{
                  background: "none",
                  border: "1px solid #e53935",
                  borderRadius: 4,
                  padding: "2px 8px",
                  color: "#e53935",
                  cursor: "pointer",
                  fontSize: 12,
                }}
              >
                移除
              </button>
            </div>
          </div>
        ))
      )}
      <button
        onClick={onLink}
        style={{
          background: "var(--color-accent)",
          color: "#fff",
          border: "none",
          borderRadius: 6,
          padding: "6px 16px",
          cursor: "pointer",
          fontSize: 13,
        }}
      >
        关联文件
      </button>
    </div>
  );
}

// ── File Detail Panel ────────────────────────────────────────────

function FileDetailView({
  item,
  films,
  onLink,
  onRemove,
}: {
  item: LibraryItemSummary;
  films: FilmListEntry[];
  onLink: (filmId: number) => void;
  onRemove: () => void;
}) {
  const [searchQ, setSearchQ] = useState("");
  const filtered = searchQ.length >= 2
    ? films.filter((f) =>
        f.title.toLowerCase().includes(searchQ.toLowerCase())
      )
    : [];

  return (
    <div>
      <h2 style={{ margin: "0 0 8px", fontSize: 16, wordBreak: "break-all" }}>
        {item.file_path.split(/[/\\]/).pop()}
      </h2>
      <div style={{ fontSize: 13, color: "var(--color-label-secondary)", marginBottom: 12, wordBreak: "break-all" }}>
        {item.file_path}
      </div>
      <div
        style={{
          display: "grid",
          gridTemplateColumns: "1fr 1fr",
          gap: 8,
          fontSize: 13,
          marginBottom: 16,
        }}
      >
        <div>大小: {formatSize(item.file_size)}</div>
        <div>时长: {formatDuration(item.duration_secs)}</div>
        <div>视频: {item.video_codec ?? "—"}</div>
        <div>音频: {item.audio_codec ?? "—"}</div>
        <div>分辨率: {item.resolution ?? "—"}</div>
      </div>

      {item.film_id ? (
        <div style={{ fontSize: 13, color: "var(--color-label-secondary)" }}>
          已关联: {item.film_title} ({item.film_year})
        </div>
      ) : (
        <div>
          <h3 style={{ fontSize: 14, marginBottom: 8 }}>关联到影片</h3>
          <input
            placeholder="搜索影片名称..."
            value={searchQ}
            onChange={(e) => setSearchQ(e.target.value)}
            style={{
              width: "100%",
              padding: "6px 10px",
              borderRadius: 6,
              border: "1px solid var(--color-separator)",
              background: "var(--color-bg-control)",
              color: "var(--color-label-primary)",
              fontSize: 13,
              marginBottom: 8,
              boxSizing: "border-box",
            }}
          />
          {filtered.slice(0, 10).map((f) => (
            <div
              key={f.id}
              onClick={() => onLink(f.id)}
              style={{
                padding: "6px 10px",
                cursor: "pointer",
                borderBottom: "1px solid var(--color-separator)",
                fontSize: 13,
              }}
            >
              {f.title} ({f.year ?? "—"})
            </div>
          ))}
        </div>
      )}

      <button
        onClick={onRemove}
        style={{
          marginTop: 16,
          background: "none",
          border: "1px solid #e53935",
          borderRadius: 6,
          padding: "6px 16px",
          color: "#e53935",
          cursor: "pointer",
          fontSize: 13,
        }}
      >
        移除文件
      </button>
    </div>
  );
}

// ── Main Page ────────────────────────────────────────────────────

export default function Library() {
  const [tab, setTab] = useState<"films" | "files">("films");
  const [stats, setStats] = useState<LibraryStats | null>(null);

  // Films tab state
  const [filmResult, setFilmResult] = useState<FilmFilterResult | null>(null);
  const [filters, setFilters] = useState<FilmFilterParams>({
    sortBy: "added",
    sortDesc: true,
    page: 1,
    pageSize: 50,
  });
  const [selectedFilmId, setSelectedFilmId] = useState<number | null>(null);
  const [genres, setGenres] = useState<{ id: number; name: string }[]>([]);

  // Files tab state
  const [items, setItems] = useState<LibraryItemSummary[]>([]);
  const [selectedItemId, setSelectedItemId] = useState<number | null>(null);

  // All films for file-linking search
  const [allFilms, setAllFilms] = useState<FilmListEntry[]>([]);

  const refresh = useCallback(() => {
    library.getLibraryStats().then(setStats);
    library.listLibraryItems().then(setItems);
    library
      .listFilmsFiltered({ pageSize: 9999 })
      .then((r) => setAllFilms(r.films));
  }, []);

  useEffect(() => {
    refresh();
    library
      .listGenresTree()
      .then((tree) => setGenres(flattenGenres(tree)));
  }, [refresh]);

  // Reload films when filters change
  useEffect(() => {
    library.listFilmsFiltered(filters).then(setFilmResult);
  }, [filters]);

  const handleScan = async () => {
    const dir = await open({ directory: true });
    if (!dir) return;
    const result = await library.scanLibraryDirectory(dir as string);
    alert(
      `扫描完成: 添加 ${result.added} 个, 跳过 ${result.skipped} 个` +
        (result.errors.length > 0
          ? `, 错误 ${result.errors.length} 个`
          : "")
    );
    refresh();
  };

  const handleLinkFile = async (filmId: number) => {
    const filePath = await open({
      multiple: false,
      filters: [
        {
          name: "Video",
          extensions: [
            "mp4", "mkv", "avi", "mov", "ts", "webm", "m4v", "flv", "wmv",
          ],
        },
      ],
    });
    if (!filePath) return;
    await library.addLibraryItem(filePath as string, filmId);
    refresh();
  };

  const handleUnlink = async (itemId: number) => {
    await library.unlinkItemFromFilm(itemId);
    refresh();
  };

  const handleRemoveItem = async (itemId: number) => {
    await library.removeLibraryItem(itemId);
    refresh();
  };

  const handleLinkItemToFilm = async (
    itemId: number,
    filmId: number
  ) => {
    await library.linkItemToFilm(itemId, filmId);
    refresh();
  };

  const selectedFilm = filmResult?.films.find(
    (f) => f.id === selectedFilmId
  );
  const selectedItem = items.find((i) => i.id === selectedItemId);
  const unlinkedItems = items.filter((i) => !i.film_id);

  return (
    <div style={{ display: "flex", flexDirection: "column", height: "100%" }}>
      <StatsBar stats={stats} onScan={handleScan} />

      {/* Tab bar */}
      <div
        style={{
          display: "flex",
          borderBottom: "1px solid var(--color-separator)",
        }}
      >
        {(
          [
            ["films", "影片"],
            ["files", `待关联文件 (${unlinkedItems.length})`],
          ] as const
        ).map(([key, label]) => (
          <button
            key={key}
            onClick={() => setTab(key)}
            style={{
              flex: 1,
              padding: "8px 0",
              background: "none",
              border: "none",
              borderBottom:
                tab === key ? "2px solid var(--color-accent)" : "2px solid transparent",
              color:
                tab === key
                  ? "var(--color-accent)"
                  : "var(--color-label-secondary)",
              cursor: "pointer",
              fontWeight: tab === key ? 600 : 400,
              fontSize: 13,
            }}
          >
            {label}
          </button>
        ))}
      </div>

      {/* Content area */}
      <div style={{ display: "flex", flex: 1, overflow: "hidden" }}>
        {/* Left panel */}
        <div
          style={{
            width: 340,
            borderRight: "1px solid var(--color-separator)",
            display: "flex",
            flexDirection: "column",
            overflow: "hidden",
          }}
        >
          {tab === "films" && (
            <>
              {/* Filter bar */}
              <div style={{ padding: "8px 12px", borderBottom: "1px solid var(--color-separator)" }}>
                <input
                  placeholder="搜索影片..."
                  value={filters.query ?? ""}
                  onChange={(e) =>
                    setFilters((prev) => ({
                      ...prev,
                      query: e.target.value || undefined,
                      page: 1,
                    }))
                  }
                  style={{
                    width: "100%",
                    padding: "6px 10px",
                    borderRadius: 6,
                    border: "1px solid var(--color-separator)",
                    background: "var(--color-bg-control)",
                    color: "var(--color-label-primary)",
                    fontSize: 13,
                    boxSizing: "border-box",
                    marginBottom: 6,
                  }}
                />
                <div style={{ display: "flex", gap: 6, flexWrap: "wrap" }}>
                  <select
                    value={filters.genreId ?? ""}
                    onChange={(e) =>
                      setFilters((prev) => ({
                        ...prev,
                        genreId: e.target.value
                          ? Number(e.target.value)
                          : undefined,
                        page: 1,
                      }))
                    }
                    style={{
                      flex: 1,
                      minWidth: 80,
                      background: "var(--color-bg-control)",
                      border: "1px solid var(--color-separator)",
                      borderRadius: 4,
                      padding: "3px 6px",
                      color: "var(--color-label-primary)",
                      fontSize: 12,
                    }}
                  >
                    <option value="">全部类型</option>
                    {genres.map((g) => (
                      <option key={g.id} value={g.id}>
                        {g.name}
                      </option>
                    ))}
                  </select>
                  <select
                    value={
                      filters.hasFile === true
                        ? "yes"
                        : filters.hasFile === false
                        ? "no"
                        : ""
                    }
                    onChange={(e) =>
                      setFilters((prev) => ({
                        ...prev,
                        hasFile:
                          e.target.value === "yes"
                            ? true
                            : e.target.value === "no"
                            ? false
                            : undefined,
                        page: 1,
                      }))
                    }
                    style={{
                      flex: 1,
                      minWidth: 80,
                      background: "var(--color-bg-control)",
                      border: "1px solid var(--color-separator)",
                      borderRadius: 4,
                      padding: "3px 6px",
                      color: "var(--color-label-primary)",
                      fontSize: 12,
                    }}
                  >
                    <option value="">全部状态</option>
                    <option value="yes">已关联</option>
                    <option value="no">未关联</option>
                  </select>
                  <select
                    value={filters.sortBy ?? "added"}
                    onChange={(e) =>
                      setFilters((prev) => ({
                        ...prev,
                        sortBy: e.target.value,
                        page: 1,
                      }))
                    }
                    style={{
                      flex: 1,
                      minWidth: 80,
                      background: "var(--color-bg-control)",
                      border: "1px solid var(--color-separator)",
                      borderRadius: 4,
                      padding: "3px 6px",
                      color: "var(--color-label-primary)",
                      fontSize: 12,
                    }}
                  >
                    <option value="added">最近添加</option>
                    <option value="title">标题</option>
                    <option value="year">年份</option>
                    <option value="rating">评分</option>
                  </select>
                </div>
              </div>
              {/* Film list */}
              <div style={{ flex: 1, overflowY: "auto" }}>
                {filmResult?.films.map((film) => (
                  <FilmCard
                    key={film.id}
                    film={film}
                    selected={film.id === selectedFilmId}
                    onClick={() => setSelectedFilmId(film.id)}
                  />
                ))}
                {filmResult && filmResult.total > filmResult.films.length && (
                  <div style={{ padding: 12, textAlign: "center" }}>
                    <button
                      onClick={() =>
                        setFilters((prev) => ({
                          ...prev,
                          pageSize: (prev.pageSize ?? 50) + 50,
                        }))
                      }
                      style={{
                        background: "none",
                        border: "1px solid var(--color-separator)",
                        borderRadius: 6,
                        padding: "6px 20px",
                        color: "var(--color-label-secondary)",
                        cursor: "pointer",
                        fontSize: 13,
                      }}
                    >
                      加载更多
                    </button>
                  </div>
                )}
              </div>
            </>
          )}

          {tab === "files" && (
            <div style={{ flex: 1, overflowY: "auto" }}>
              {unlinkedItems.length === 0 ? (
                <div
                  style={{
                    padding: 24,
                    textAlign: "center",
                    color: "var(--color-label-tertiary)",
                    fontSize: 13,
                  }}
                >
                  没有待关联文件。点击"扫描目录"添加文件。
                </div>
              ) : (
                unlinkedItems.map((item) => (
                  <FileCard
                    key={item.id}
                    item={item}
                    selected={item.id === selectedItemId}
                    onClick={() => setSelectedItemId(item.id)}
                  />
                ))
              )}
            </div>
          )}
        </div>

        {/* Right panel */}
        <div style={{ flex: 1, overflowY: "auto", padding: 24 }}>
          {tab === "films" && selectedFilm ? (
            <FilmDetailView
              film={selectedFilm}
              items={items}
              onLink={() => handleLinkFile(selectedFilm.id)}
              onUnlink={handleUnlink}
              onRemoveItem={handleRemoveItem}
              onRefresh={refresh}
            />
          ) : tab === "files" && selectedItem ? (
            <FileDetailView
              item={selectedItem}
              films={allFilms}
              onLink={(filmId) =>
                handleLinkItemToFilm(selectedItem.id, filmId)
              }
              onRemove={() => handleRemoveItem(selectedItem.id)}
            />
          ) : (
            <div
              style={{
                height: "100%",
                display: "flex",
                alignItems: "center",
                justifyContent: "center",
                color: "var(--color-label-tertiary)",
              }}
            >
              {tab === "films"
                ? "选择一部影片查看详情"
                : "选择一个文件查看详情"}
            </div>
          )}
        </div>
      </div>
    </div>
  );
}
```

- [ ] **Step 3: Type check**

Run: `cd C:/Users/lixuan/Documents/code/rust/blowup && npx tsc --noEmit 2>&1`
Expected: 0 errors (Library.tsx is not yet routed but should type-check)

- [ ] **Step 4: Commit**

```bash
cd C:/Users/lixuan/Documents/code/rust/blowup
git add src/pages/Library.tsx
git commit -m "feat: add Library page with film browsing, file linking, and stats"
```

---

### Task 7: App.tsx unlock + final build

**Files:**
- Modify: `src/App.tsx`

- [ ] **Step 1: Read `App.tsx`**

Read `src/App.tsx` to see current routing and nav structure.

- [ ] **Step 2: Import Library and enable route**

1. Add import at top:
```tsx
import Library from "./pages/Library";
```

2. In `NAV_SECTIONS`, find the nav item with `path: "/library"` and remove its `disabled: true` property.

3. In the `<Routes>` section, find the `/library` route and replace the Placeholder with Library:
```tsx
<Route path="/library" element={<Library />} />
```

4. In the `KB_PATHS` array, add `"/library"`:
```tsx
const KB_PATHS = ["/", "/people", "/genres", "/graph", "/library"];
```

- [ ] **Step 3: Type check**

Run: `cd C:/Users/lixuan/Documents/code/rust/blowup && npx tsc --noEmit 2>&1`
Expected: 0 errors

- [ ] **Step 4: Full frontend build**

Run: `cd C:/Users/lixuan/Documents/code/rust/blowup && npm run build 2>&1`
Expected: Build success

- [ ] **Step 5: Full Rust test suite**

Run: `cd C:/Users/lixuan/Documents/code/rust/blowup/src-tauri && cargo test 2>&1`
Expected: All tests pass (~65 tests)

- [ ] **Step 6: Commit**

```bash
cd C:/Users/lixuan/Documents/code/rust/blowup
git add src/App.tsx
git commit -m "feat: unlock /library route — M3 complete"
```

---

## Self-review checklist

- [x] **Spec coverage:** All M3 requirements covered — library items CRUD, scanning, assets, stats, film filtering, Library page UI
- [x] **No placeholders:** Every task has complete code
- [x] **Type consistency:** `LibraryItemSummary`, `FilmListEntry`, `FilmFilterResult`, `LibraryStats`, `ScanResult` — names match across Rust types, tauri.ts types, and frontend usage
- [x] **Pattern consistency:** `pool.inner()`, `query_as::<_, T>()`, `Result<T, String>`, `useEffect` for data loading
- [x] **File paths:** All exact
- [x] **Test commands:** All exact with expected output
- [x] **No DB migration needed:** `library_items.film_id` is already nullable in the existing schema
