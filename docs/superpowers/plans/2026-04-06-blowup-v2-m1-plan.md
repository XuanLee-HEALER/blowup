# blowup v2 M1 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Transform blowup from a Rust CLI into a Tauri v2 desktop app with React frontend, SQLite data layer, and a working TMDB advanced search page.

**Architecture:** All existing Rust modules migrate into `src-tauri/src/` and are wrapped as `#[tauri::command]`. The current `src/` becomes the React frontend. A fixed sidebar shell routes to Search (M1 complete) and Settings (M1 complete); all other pages are opaque placeholders.

**Tech Stack:** Tauri v2 · React 19 · TypeScript · Vite · Tailwind CSS v4 · sqlx 0.8 (SQLite) · React Router v7 · @tauri-apps/api v2

---

## File Map

```
blowup/
├── Cargo.toml                          # [workspace] pointing to src-tauri
├── src/                                # React frontend (replaces old Rust src/)
│   ├── index.html
│   ├── main.tsx
│   ├── App.tsx
│   ├── index.css                       # Tailwind v4 + CSS tokens
│   ├── lib/
│   │   └── tauri.ts                    # typed invoke() wrappers
│   ├── components/
│   │   └── ui/
│   │       ├── NavItem.tsx
│   │       ├── TextInput.tsx
│   │       ├── Chip.tsx
│   │       └── Button.tsx
│   └── pages/
│       ├── Search.tsx
│       ├── Settings.tsx
│       └── Placeholder.tsx
├── src-tauri/
│   ├── Cargo.toml
│   ├── build.rs
│   ├── tauri.conf.json
│   ├── capabilities/
│   │   └── default.json
│   ├── migrations/
│   │   └── 001_initial.sql
│   └── src/
│       ├── main.rs
│       ├── lib.rs
│       ├── error.rs                    # ← src/error.rs (unchanged)
│       ├── common.rs                   # ← src/common.rs (unchanged)
│       ├── ffmpeg.rs                   # ← src/ffmpeg.rs (unchanged)
│       ├── config.rs                   # ← src/config.rs (+ library.root_dir)
│       ├── db/
│       │   └── mod.rs
│       └── commands/
│           ├── mod.rs
│           ├── search.rs               # ← src/search.rs
│           ├── tmdb.rs                 # ← src/tmdb.rs + discover + genres
│           ├── download.rs             # ← src/download.rs
│           ├── subtitle.rs             # ← src/sub/* merged
│           ├── tracker.rs              # ← src/tracker.rs
│           ├── media.rs                # ← ffmpeg commands
│           └── config.rs              # read_config / set_config_key commands
├── package.json
├── vite.config.ts
└── tsconfig.json
```

---

## Task 1: Project Restructure

**Files:**
- Create: `Cargo.toml` (root workspace)
- Create: `src-tauri/` directory skeleton
- Move: `src/*.rs` → `src-tauri/src/*.rs`
- Move: `src/sub/` → `src-tauri/src/sub/`
- Delete: `src/main.rs`, `src/ai.rs`, `src/config_cmd.rs`

- [ ] **Step 1: Create root workspace Cargo.toml**

```toml
# Cargo.toml (root — replace existing)
[workspace]
members = ["src-tauri"]
resolver = "2"
```

- [ ] **Step 2: Create src-tauri directory structure**

```bash
mkdir -p src-tauri/src/commands
mkdir -p src-tauri/src/db
mkdir -p src-tauri/migrations
mkdir -p src-tauri/capabilities
```

- [ ] **Step 3: Move existing Rust source files**

```bash
# Move modules that survive as-is
cp src/error.rs   src-tauri/src/error.rs
cp src/common.rs  src-tauri/src/common.rs
cp src/ffmpeg.rs  src-tauri/src/ffmpeg.rs
cp src/config.rs  src-tauri/src/config.rs

# Move modules that become commands (keep copies for editing)
cp src/search.rs     src-tauri/src/commands/search.rs
cp src/tmdb.rs       src-tauri/src/commands/tmdb.rs
cp src/download.rs   src-tauri/src/commands/download.rs
cp src/tracker.rs    src-tauri/src/commands/tracker.rs

# sub/ modules go into subtitle.rs (combined in Task 9)
# Keep originals for reference during migration
cp src/sub/fetch.rs  src-tauri/src/commands/_sub_fetch.rs
cp src/sub/align.rs  src-tauri/src/commands/_sub_align.rs
cp src/sub/shift.rs  src-tauri/src/commands/_sub_shift.rs
cp src/sub/mod.rs    src-tauri/src/commands/_sub_mod.rs
```

- [ ] **Step 4: Fix Windows-incompatible code in common.rs**

`src/common.rs` (now at `src-tauri/src/common.rs`) uses `std::os::unix::fs::MetadataExt`
which doesn't compile on Windows. Replace the `read_file_to_string` function:

```rust
// Remove this import:
//   use std::os::unix::fs::MetadataExt;

// Replace read_file_to_string with:
async fn read_file_to_string<P: AsRef<Path>>(idx: usize, file: P) -> Result<(usize, String)> {
    const SIZE_LIMIT: u64 = 1024 * 1024;
    let file_path = file.as_ref();
    let meta = std::fs::metadata(file_path).map_err(|_| CommonError::IoError)?;
    if !meta.is_file() || meta.len() > SIZE_LIMIT {
        return Err(CommonError::IoError);
    }
    let mut res = String::new();
    let mut f = File::open(file_path).await?;
    f.read_to_string(&mut res).await?;
    Ok((idx, res))
}
```

Also remove the `use std::os::unix::fs::MetadataExt;` import line at the top.

- [ ] **Step 5: Remove old CLI-only files**

```bash
# After verifying copies exist in src-tauri/
rm src/main.rs src/ai.rs src/config_cmd.rs src/lib.rs
rm src/search.rs src/tmdb.rs src/download.rs src/tracker.rs
rm -rf src/sub/
```

- [ ] **Step 6: Commit checkpoint**

```bash
git add -A
git commit -m "chore: restructure project skeleton for Tauri v2"
```

---

## Task 2: src-tauri/Cargo.toml + build.rs

**Files:**
- Create: `src-tauri/Cargo.toml`
- Create: `src-tauri/build.rs`

- [ ] **Step 1: Write src-tauri/Cargo.toml**

```toml
[package]
name = "blowup"
version = "2.0.0"
edition = "2024"

[lib]
name = "blowup_lib"
crate-type = ["staticlib", "cdylib", "rlib"]

[build-dependencies]
tauri-build = { version = "2", features = [] }

[dependencies]
tauri          = { version = "2", features = [] }
tauri-plugin-dialog = "2"
tauri-plugin-opener = "2"
serde          = { version = "1", features = ["derive"] }
serde_json     = "1"

# Retained from v1
reqwest        = { version = "0.12", features = ["json"] }
tokio          = { version = "1", features = ["full"] }
thiserror      = "2"
chrono         = "0.4"
regex          = "1"
shellexpand    = "3"
which          = "8"
walkdir        = "2"
toml           = "0.8"
toml_edit      = "0.22"
dirs           = "5"
anyhow         = "1"
flate2         = "1"
octocrab       = "0.44"

# New for v2
sqlx = { version = "0.8", features = ["sqlite", "runtime-tokio-native-tls", "migrate"] }
```

- [ ] **Step 2: Write src-tauri/build.rs**

```rust
fn main() {
    tauri_build::build()
}
```

- [ ] **Step 3: Commit**

```bash
git add src-tauri/Cargo.toml src-tauri/build.rs
git commit -m "chore: add src-tauri Cargo.toml and build.rs for Tauri v2"
```

---

## Task 3: Tauri Configuration Files

**Files:**
- Create: `src-tauri/tauri.conf.json`
- Create: `src-tauri/capabilities/default.json`

- [ ] **Step 1: Write tauri.conf.json**

```json
{
  "productName": "blowup",
  "version": "2.0.0",
  "identifier": "io.github.xuanlee-healer.blowup",
  "build": {
    "beforeDevCommand": "npm run dev",
    "devUrl": "http://localhost:1420",
    "beforeBuildCommand": "npm run build",
    "frontendDist": "../dist"
  },
  "app": {
    "withGlobalTauri": false,
    "windows": [
      {
        "title": "blowup",
        "width": 1200,
        "height": 800,
        "minWidth": 900,
        "minHeight": 600,
        "decorations": true
      }
    ]
  },
  "bundle": {
    "active": true,
    "targets": "all",
    "icon": []
  }
}
```

- [ ] **Step 2: Write capabilities/default.json**

```json
{
  "$schema": "../gen/schemas/desktop-schema.json",
  "identifier": "default",
  "description": "Default permissions for blowup desktop",
  "windows": ["main"],
  "permissions": [
    "core:default",
    "dialog:default",
    "opener:default"
  ]
}
```

- [ ] **Step 3: Commit**

```bash
git add src-tauri/tauri.conf.json src-tauri/capabilities/
git commit -m "chore: add Tauri v2 configuration and capabilities"
```

---

## Task 4: Tauri Entry Points

**Files:**
- Create: `src-tauri/src/main.rs`
- Create: `src-tauri/src/lib.rs` (skeleton — commands registered in Task 11)

- [ ] **Step 1: Write main.rs**

```rust
// src-tauri/src/main.rs
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    blowup_lib::run()
}
```

- [ ] **Step 2: Write lib.rs skeleton**

```rust
// src-tauri/src/lib.rs
pub mod commands;
pub mod common;
pub mod config;
pub mod db;
pub mod error;
pub mod ffmpeg;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            let handle = app.handle().clone();
            tauri::async_runtime::block_on(async move {
                let pool = db::init_db(&handle)
                    .await
                    .expect("Failed to initialize database");
                handle.manage(pool);
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            // Commands registered in Task 11
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
```

- [ ] **Step 3: Commit**

```bash
git add src-tauri/src/main.rs src-tauri/src/lib.rs
git commit -m "chore: add Tauri entry points (main.rs, lib.rs skeleton)"
```

---

## Task 5: SQLite Migration File

**Files:**
- Create: `src-tauri/migrations/001_initial.sql`

- [ ] **Step 1: Write the migration SQL**

```sql
-- src-tauri/migrations/001_initial.sql

CREATE TABLE IF NOT EXISTS people (
  id            INTEGER PRIMARY KEY,
  tmdb_id       INTEGER UNIQUE,
  name          TEXT NOT NULL,
  born_date     TEXT,
  biography     TEXT,
  nationality   TEXT,
  primary_role  TEXT NOT NULL CHECK(primary_role IN (
                  'director','cinematographer','composer',
                  'editor','screenwriter','producer','actor')),
  created_at    TEXT DEFAULT (datetime('now')),
  updated_at    TEXT DEFAULT (datetime('now'))
);

CREATE TABLE IF NOT EXISTS genres (
  id          INTEGER PRIMARY KEY,
  name        TEXT NOT NULL,
  description TEXT,
  parent_id   INTEGER REFERENCES genres(id),
  period      TEXT
);

CREATE TABLE IF NOT EXISTS films (
  id                INTEGER PRIMARY KEY,
  tmdb_id           INTEGER UNIQUE,
  title             TEXT NOT NULL,
  original_title    TEXT,
  year              INTEGER,
  overview          TEXT,
  tmdb_rating       REAL,
  poster_cache_path TEXT,
  created_at        TEXT DEFAULT (datetime('now')),
  updated_at        TEXT DEFAULT (datetime('now'))
);

CREATE TABLE IF NOT EXISTS person_films (
  person_id INTEGER NOT NULL REFERENCES people(id),
  film_id   INTEGER NOT NULL REFERENCES films(id),
  role      TEXT NOT NULL,
  PRIMARY KEY (person_id, film_id, role)
);

CREATE TABLE IF NOT EXISTS film_genres (
  film_id  INTEGER NOT NULL REFERENCES films(id),
  genre_id INTEGER NOT NULL REFERENCES genres(id),
  PRIMARY KEY (film_id, genre_id)
);

CREATE TABLE IF NOT EXISTS person_genres (
  person_id INTEGER NOT NULL REFERENCES people(id),
  genre_id  INTEGER NOT NULL REFERENCES genres(id),
  PRIMARY KEY (person_id, genre_id)
);

CREATE TABLE IF NOT EXISTS person_relations (
  from_id       INTEGER NOT NULL REFERENCES people(id),
  to_id         INTEGER NOT NULL REFERENCES people(id),
  relation_type TEXT NOT NULL CHECK(relation_type IN (
                  'influenced','contemporary','collaborated')),
  PRIMARY KEY (from_id, to_id, relation_type)
);

CREATE TABLE IF NOT EXISTS wiki_entries (
  id          INTEGER PRIMARY KEY,
  entity_type TEXT NOT NULL CHECK(entity_type IN ('person','film','genre')),
  entity_id   INTEGER NOT NULL,
  content     TEXT NOT NULL DEFAULT '',
  updated_at  TEXT DEFAULT (datetime('now')),
  UNIQUE (entity_type, entity_id)
);

CREATE TABLE IF NOT EXISTS reviews (
  id          INTEGER PRIMARY KEY,
  film_id     INTEGER NOT NULL REFERENCES films(id),
  is_personal INTEGER NOT NULL DEFAULT 0,
  author      TEXT,
  content     TEXT NOT NULL,
  rating      REAL,
  created_at  TEXT DEFAULT (datetime('now'))
);

CREATE TABLE IF NOT EXISTS library_items (
  id            INTEGER PRIMARY KEY,
  film_id       INTEGER REFERENCES films(id),
  file_path     TEXT NOT NULL UNIQUE,
  file_size     INTEGER,
  duration_secs INTEGER,
  video_codec   TEXT,
  audio_codec   TEXT,
  resolution    TEXT,
  added_at      TEXT DEFAULT (datetime('now'))
);

CREATE TABLE IF NOT EXISTS library_assets (
  id         INTEGER PRIMARY KEY,
  item_id    INTEGER NOT NULL REFERENCES library_items(id),
  asset_type TEXT NOT NULL CHECK(asset_type IN ('subtitle','edited','poster')),
  file_path  TEXT NOT NULL,
  lang       TEXT,
  created_at TEXT DEFAULT (datetime('now'))
);
```

- [ ] **Step 2: Commit**

```bash
git add src-tauri/migrations/
git commit -m "feat: add SQLite initial schema migration"
```

---

## Task 6: SQLite db Module

**Files:**
- Create: `src-tauri/src/db/mod.rs`

- [ ] **Step 1: Write db/mod.rs**

```rust
// src-tauri/src/db/mod.rs
use sqlx::SqlitePool;
use tauri::AppHandle;
use tauri::Manager;

pub async fn init_db(app: &AppHandle) -> Result<SqlitePool, sqlx::Error> {
    let data_dir = app
        .path()
        .app_data_dir()
        .expect("could not resolve app data dir");
    std::fs::create_dir_all(&data_dir).ok();

    let db_path = data_dir.join("blowup.db");
    let url = format!(
        "sqlite://{}?mode=rwc",
        db_path.to_str().expect("non-utf8 db path")
    );

    let pool = SqlitePool::connect(&url).await?;
    sqlx::migrate!("../migrations").run(&pool).await?;
    Ok(pool)
}
```

- [ ] **Step 2: Verify it compiles (partial build)**

```bash
cd src-tauri && cargo check 2>&1 | head -30
```

Expected: errors only about missing `commands` module contents — not about `db`. If `db::init_db` shows type errors, fix them before proceeding.

- [ ] **Step 3: Commit**

```bash
git add src-tauri/src/db/
git commit -m "feat: add SQLite connection pool initialization"
```

---

## Task 7: commands/search.rs

**Files:**
- Modify: `src-tauri/src/commands/search.rs` (add `#[tauri::command]` wrapper)

The file was copied from `src/search.rs` in Task 1. Add a public command wrapper at the bottom; the internal logic stays identical.

- [ ] **Step 1: Add the Tauri command wrapper**

Open `src-tauri/src/commands/search.rs` and append after the existing `pub async fn search_yify` and helpers:

```rust
// ── Tauri command ─────────────────────────────────────────────
#[tauri::command]
pub async fn search_yify_cmd(
    query: String,
    year: Option<u32>,
) -> Result<Vec<MovieResult>, String> {
    search_yify(&query, year).await.map_err(|e| e.to_string())
}
```

Also add `use serde::Serialize;` at the top if not present, and derive `Serialize` on `MovieResult`:

```rust
#[derive(Debug, Clone, Serialize)]
pub struct MovieResult {
    pub title: String,
    pub year: u32,
    pub quality: String,
    pub magnet: Option<String>,
    pub torrent_url: Option<String>,
    pub seeds: u32,
}
```

- [ ] **Step 2: Run existing tests**

```bash
cd src-tauri && cargo test commands::search 2>&1
```

Expected: all existing search tests pass.

- [ ] **Step 3: Commit**

```bash
git add src-tauri/src/commands/search.rs
git commit -m "feat: wrap search_yify as Tauri command"
```

---

## Task 8: commands/tmdb.rs — Existing Wrapper + Discover + Genres

**Files:**
- Modify: `src-tauri/src/commands/tmdb.rs`

The file was copied from `src/tmdb.rs`. The existing `query_tmdb` + `TmdbMovie` stay as-is. Add three new Tauri commands: `search_movies`, `discover_movies`, `list_genres`.

- [ ] **Step 1: Add new public types at the top of the file**

After existing imports, add:

```rust
use serde::Serialize;

/// Lightweight result row shown in the search list.
#[derive(Debug, Serialize)]
pub struct MovieListItem {
    pub id: u64,
    pub title: String,
    pub original_title: String,
    pub year: String,          // empty string if unknown
    pub overview: String,
    pub vote_average: f64,
    pub poster_path: Option<String>,
    pub genre_ids: Vec<u64>,
}

#[derive(Debug, serde::Deserialize, Serialize)]
pub struct SearchFilters {
    pub year_from: Option<u32>,
    pub year_to: Option<u32>,
    pub genre_ids: Vec<u64>,
    pub min_rating: Option<f32>,
    pub sort_by: Option<String>,   // "popularity.desc" | "vote_average.desc" | "release_date.desc"
    pub page: Option<u32>,
}

#[derive(Debug, Serialize)]
pub struct TmdbGenre {
    pub id: u64,
    pub name: String,
}
```

- [ ] **Step 2: Add private TMDB deserialization structs**

```rust
#[derive(serde::Deserialize)]
struct ListResponse {
    results: Vec<ListItem>,
}

#[derive(serde::Deserialize)]
struct ListItem {
    id: u64,
    title: String,
    original_title: String,
    release_date: Option<String>,
    overview: String,
    vote_average: f64,
    poster_path: Option<String>,
    genre_ids: Vec<u64>,
}

#[derive(serde::Deserialize)]
struct GenreListResponse {
    genres: Vec<GenreItem>,
}

#[derive(serde::Deserialize)]
struct GenreItem {
    id: u64,
    name: String,
}

#[derive(serde::Deserialize)]
struct PersonSearchResponse {
    results: Vec<PersonItem>,
}

#[derive(serde::Deserialize)]
struct PersonItem {
    id: u64,
}
```

- [ ] **Step 3: Add helper to convert ListItem → MovieListItem**

```rust
fn to_list_item(item: ListItem) -> MovieListItem {
    let year = item
        .release_date
        .as_deref()
        .and_then(|d| d.get(..4))
        .unwrap_or("")
        .to_string();
    MovieListItem {
        id: item.id,
        title: item.title,
        original_title: item.original_title,
        year,
        overview: item.overview,
        vote_average: item.vote_average,
        poster_path: item.poster_path,
        genre_ids: item.genre_ids,
    }
}
```

- [ ] **Step 4: Add the three Tauri commands**

```rust
/// Search by title, optionally merge with director results.
#[tauri::command]
pub async fn search_movies(
    api_key: String,
    query: String,
    filters: SearchFilters,
) -> Result<Vec<MovieListItem>, String> {
    if api_key.is_empty() {
        return Err("TMDB API key not configured".into());
    }
    let client = reqwest::Client::new();
    let page = filters.page.unwrap_or(1);

    // ① Title search
    let mut params: Vec<(&str, String)> = vec![
        ("api_key", api_key.clone()),
        ("query", query.clone()),
        ("page", page.to_string()),
    ];
    if let Some(y) = filters.year_from {
        params.push(("year", y.to_string()));
    }
    let title_resp: ListResponse = client
        .get("https://api.themoviedb.org/3/search/movie")
        .query(&params)
        .send()
        .await
        .map_err(|e| e.to_string())?
        .json()
        .await
        .map_err(|e| e.to_string())?;

    let mut seen: std::collections::HashSet<u64> = std::collections::HashSet::new();
    let mut results: Vec<MovieListItem> = title_resp
        .results
        .into_iter()
        .map(|i| { seen.insert(i.id); to_list_item(i) })
        .collect();

    // ② Person search → discover
    let person_resp: Result<PersonSearchResponse, _> = client
        .get("https://api.themoviedb.org/3/search/person")
        .query(&[("api_key", &api_key), ("query", &query)])
        .send()
        .await
        .map_err(|e| e.to_string())?
        .json()
        .await;

    if let Ok(pr) = person_resp {
        if let Some(person) = pr.results.first() {
            let mut disc_params = build_discover_params(&api_key, &filters);
            disc_params.push(("with_people", person.id.to_string()));
            let disc_resp: Result<ListResponse, _> = client
                .get("https://api.themoviedb.org/3/discover/movie")
                .query(&disc_params)
                .send()
                .await
                .map_err(|e| e.to_string())?
                .json()
                .await;
            if let Ok(dr) = disc_resp {
                for item in dr.results {
                    if seen.insert(item.id) {
                        results.push(to_list_item(item));
                    }
                }
            }
        }
    }

    Ok(results)
}

/// Pure filter-based discovery (no text query).
#[tauri::command]
pub async fn discover_movies(
    api_key: String,
    filters: SearchFilters,
) -> Result<Vec<MovieListItem>, String> {
    if api_key.is_empty() {
        return Err("TMDB API key not configured".into());
    }
    let client = reqwest::Client::new();
    let params = build_discover_params(&api_key, &filters);
    let resp: ListResponse = client
        .get("https://api.themoviedb.org/3/discover/movie")
        .query(&params)
        .send()
        .await
        .map_err(|e| e.to_string())?
        .json()
        .await
        .map_err(|e| e.to_string())?;
    Ok(resp.results.into_iter().map(to_list_item).collect())
}

/// Fetch TMDB genre list (call once and cache in frontend).
#[tauri::command]
pub async fn list_genres(api_key: String) -> Result<Vec<TmdbGenre>, String> {
    if api_key.is_empty() {
        return Err("TMDB API key not configured".into());
    }
    let client = reqwest::Client::new();
    let resp: GenreListResponse = client
        .get("https://api.themoviedb.org/3/genre/movie/list")
        .query(&[("api_key", &api_key), ("language", &"en-US".to_string())])
        .send()
        .await
        .map_err(|e| e.to_string())?
        .json()
        .await
        .map_err(|e| e.to_string())?;
    Ok(resp.genres.into_iter().map(|g| TmdbGenre { id: g.id, name: g.name }).collect())
}

fn build_discover_params(api_key: &str, f: &SearchFilters) -> Vec<(&'static str, String)> {
    let mut p: Vec<(&'static str, String)> = vec![
        ("api_key", api_key.to_string()),
        ("language", "en-US".to_string()),
        ("page", f.page.unwrap_or(1).to_string()),
        (
            "sort_by",
            f.sort_by.clone().unwrap_or_else(|| "vote_average.desc".to_string()),
        ),
        ("vote_count.gte", "50".to_string()), // avoid films with 1 vote
    ];
    if let Some(y) = f.year_from {
        p.push(("primary_release_date.gte", format!("{y}-01-01")));
    }
    if let Some(y) = f.year_to {
        p.push(("primary_release_date.lte", format!("{y}-12-31")));
    }
    if !f.genre_ids.is_empty() {
        let ids: Vec<String> = f.genre_ids.iter().map(|id| id.to_string()).collect();
        p.push(("with_genres", ids.join(",")));
    }
    if let Some(r) = f.min_rating {
        p.push(("vote_average.gte", r.to_string()));
    }
    p
}
```

- [ ] **Step 5: Run tests (existing tmdb tests must still pass)**

```bash
cd src-tauri && cargo test commands::tmdb 2>&1
```

- [ ] **Step 6: Commit**

```bash
git add src-tauri/src/commands/tmdb.rs
git commit -m "feat: add search_movies, discover_movies, list_genres Tauri commands"
```

---

## Task 9: commands/download.rs, tracker.rs, subtitle.rs, media.rs

**Files:**
- Modify: `src-tauri/src/commands/download.rs`
- Modify: `src-tauri/src/commands/tracker.rs`
- Create: `src-tauri/src/commands/subtitle.rs`
- Create: `src-tauri/src/commands/media.rs`

- [ ] **Step 1: Add command wrapper to download.rs**

Append to `src-tauri/src/commands/download.rs`:

```rust
#[tauri::command]
pub async fn download_target(
    target: String,
    output_dir: String,
    aria2c_bin: String,
) -> Result<(), String> {
    let path = std::path::PathBuf::from(&output_dir);
    download(DownloadArgs {
        target: &target,
        output_dir: &path,
        aria2c_bin: &aria2c_bin,
    })
    .await
    .map_err(|e| e.to_string())
}
```

- [ ] **Step 2: Add command wrapper to tracker.rs**

Append to `src-tauri/src/commands/tracker.rs`:

```rust
#[tauri::command]
pub async fn update_trackers(source: Option<String>) -> Result<(), String> {
    update_tracker_list(source).await.map_err(|e| e.to_string())
}
```

- [ ] **Step 3: Create subtitle.rs by merging sub/* modules**

Create `src-tauri/src/commands/subtitle.rs`:

```rust
// src-tauri/src/commands/subtitle.rs
//
// Merged from src/sub/{fetch,align,shift,mod}.rs

// ── fetch ──────────────────────────────────────────────────────
use crate::config::Config;
use crate::error::SubError;
use regex::Regex;
use std::io::Read;
use std::path::Path;

// (paste entire contents of _sub_fetch.rs here, removing `use crate::config::Config`
//  line if it conflicts — import at top of this file instead)

// ── align ──────────────────────────────────────────────────────
// (paste entire contents of _sub_align.rs here)

// ── shift ──────────────────────────────────────────────────────
// (paste entire contents of _sub_shift.rs here)

// ── extract / list (from sub/mod.rs) ──────────────────────────
use crate::ffmpeg::{FfmpegError, FfmpegTool};
use serde::{Deserialize, Serialize};

// (paste extract_sub_srt, list_all_subtitle_stream, and helper structs
//  from _sub_mod.rs here)

// ── Tauri commands ─────────────────────────────────────────────
#[tauri::command]
pub async fn fetch_subtitle_cmd(
    video: String,
    lang: String,
    api_key: String,
) -> Result<(), String> {
    let cfg = crate::config::load_config();
    let video_path = std::path::Path::new(&video);
    fetch_subtitle(video_path, &lang, SubSource::All, &cfg)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn align_subtitle_cmd(video: String, srt: String) -> Result<(), String> {
    let cfg = crate::config::load_config();
    align_subtitle(
        Path::new(&video),
        Path::new(&srt),
        Some(&cfg.tools.alass),
    )
    .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn extract_subtitle_cmd(
    video: String,
    stream: Option<u32>,
) -> Result<(), String> {
    extract_sub_srt(Path::new(&video), stream)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn list_subtitle_streams_cmd(video: String) -> Result<(), String> {
    list_all_subtitle_stream(Path::new(&video))
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn shift_subtitle_cmd(srt: String, offset_ms: i64) -> Result<(), String> {
    shift_srt(Path::new(&srt), offset_ms).map_err(|e| e.to_string())
}
```

Note: after writing the file, delete the temporary `_sub_*.rs` files:
```bash
rm src-tauri/src/commands/_sub_*.rs
```

- [ ] **Step 4: Create media.rs**

Create `src-tauri/src/commands/media.rs`:

```rust
// src-tauri/src/commands/media.rs
use crate::ffmpeg::FfmpegTool;

/// Returns (stdout, stderr) from ffprobe run on the given file.
#[tauri::command]
pub async fn probe_media(file: String) -> Result<String, String> {
    let args = vec![
        "-v".to_string(),
        "quiet".to_string(),
        "-print_format".to_string(),
        "json".to_string(),
        "-show_streams".to_string(),
        "--".to_string(),
        file,
    ];
    let (stdout, _) = FfmpegTool::Ffprobe
        .exec_with_options(None::<&str>, Some(args))
        .await
        .map_err(|e| e.to_string())?;
    Ok(stdout)
}
```

- [ ] **Step 5: Run cargo check**

```bash
cd src-tauri && cargo check 2>&1 | grep -E "^error" | head -20
```

Fix any compilation errors before proceeding.

- [ ] **Step 6: Commit**

```bash
git add src-tauri/src/commands/
git commit -m "feat: migrate download, tracker, subtitle, media modules as Tauri commands"
```

---

## Task 10: commands/config.rs + Update config.rs

**Files:**
- Create: `src-tauri/src/commands/config.rs`
- Modify: `src-tauri/src/config.rs` (add `library.root_dir`)

- [ ] **Step 1: Add `ffmpeg` to ToolsConfig + add LibraryConfig**

Open `src-tauri/src/config.rs`.

First, add `ffmpeg` to `ToolsConfig`:

```rust
#[derive(Debug, Deserialize, Serialize)]
pub struct ToolsConfig {
    #[serde(default = "default_aria2c")]
    pub aria2c: String,
    #[serde(default = "default_alass")]
    pub alass: String,
    #[serde(default = "default_ffmpeg")]   // ← new
    pub ffmpeg: String,                    // ← new
}

fn default_ffmpeg() -> String {
    "ffmpeg".to_string()
}

impl Default for ToolsConfig {
    fn default() -> Self {
        Self {
            aria2c: default_aria2c(),
            alass: default_alass(),
            ffmpeg: default_ffmpeg(),  // ← new
        }
    }
}
```

Then add `LibraryConfig`:

```rust
#[derive(Debug, Deserialize, Serialize)]
pub struct LibraryConfig {
    #[serde(default = "default_root_dir")]
    pub root_dir: String,
}

fn default_root_dir() -> String {
    dirs::home_dir()
        .map(|h| h.join("Movies").join("blowup").to_string_lossy().into_owned())
        .unwrap_or_else(|| "~/Movies/blowup".to_string())
}

impl Default for LibraryConfig {
    fn default() -> Self {
        Self { root_dir: default_root_dir() }
    }
}
```

Add `library` field to `Config`:

```rust
#[derive(Debug, Default, Deserialize, Serialize)]
pub struct Config {
    #[serde(default)]
    pub tools: ToolsConfig,
    #[serde(default)]
    pub search: SearchConfig,
    #[serde(default)]
    pub subtitle: SubtitleConfig,
    #[serde(default)]
    pub opensubtitles: OpenSubtitlesConfig,
    #[serde(default)]
    pub tmdb: TmdbConfig,
    #[serde(default)]                  // ← new
    pub library: LibraryConfig,        // ← new
}
```

- [ ] **Step 2: Write commands/config.rs**

```rust
// src-tauri/src/commands/config.rs
use crate::config::{load_config, Config};

#[tauri::command]
pub fn get_config() -> Result<Config, String> {
    Ok(load_config())
}

/// key format: "section.field"  e.g. "tmdb.api_key"
/// Reuses the existing toml_edit-based set logic from v1 config_cmd.
#[tauri::command]
pub fn set_config_key(key: String, value: String) -> Result<(), String> {
    use crate::config::config_path;
    use toml_edit::{DocumentMut, Item, Value};

    let path = config_path();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }

    let content = std::fs::read_to_string(&path).unwrap_or_default();
    let mut doc: DocumentMut = content.parse().map_err(|e: toml_edit::TomlError| e.to_string())?;

    let mut parts = key.splitn(2, '.');
    let section = parts
        .next()
        .filter(|s| !s.is_empty())
        .ok_or_else(|| format!("Invalid key: '{key}'"))?;
    let field = parts
        .next()
        .filter(|s| !s.is_empty())
        .ok_or_else(|| format!("Invalid key: '{key}'"))?;

    doc[section][field] = Item::Value(Value::from(value));
    std::fs::write(&path, doc.to_string()).map_err(|e| e.to_string())?;
    Ok(())
}
```

Note: `Config` must derive `Serialize` for Tauri to return it to the frontend. Add `use serde::Serialize;` and `#[derive(Serialize)]` on Config and all nested structs in `config.rs`.

- [ ] **Step 3: Add `Serialize` derives to config.rs**

In `src-tauri/src/config.rs`, change every `#[derive(Debug, ...Deserialize)]` to also include `Serialize`:

```rust
#[derive(Debug, Default, Deserialize, Serialize)]
pub struct Config { ... }

#[derive(Debug, Deserialize, Serialize)]
pub struct ToolsConfig { ... }

// etc. for SearchConfig, SubtitleConfig, OpenSubtitlesConfig, TmdbConfig, LibraryConfig
```

- [ ] **Step 4: Run tests**

```bash
cd src-tauri && cargo test config 2>&1
```

Expected: `default_config_has_sane_values`, `tmdb_default_api_key_is_empty`, `parse_partial_toml` all pass.

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/config.rs src-tauri/src/commands/config.rs
git commit -m "feat: add get_config and set_config_key Tauri commands, add library.root_dir"
```

---

## Task 11: commands/mod.rs + Register All Commands

**Files:**
- Create: `src-tauri/src/commands/mod.rs`
- Modify: `src-tauri/src/lib.rs` (fill in generate_handler!)

- [ ] **Step 1: Write commands/mod.rs**

```rust
// src-tauri/src/commands/mod.rs
pub mod config;
pub mod download;
pub mod media;
pub mod search;
pub mod subtitle;
pub mod tmdb;
pub mod tracker;
```

- [ ] **Step 2: Register all commands in lib.rs**

Replace the empty `generate_handler![]` in `src-tauri/src/lib.rs` with:

```rust
.invoke_handler(tauri::generate_handler![
    commands::search::search_yify_cmd,
    commands::tmdb::search_movies,
    commands::tmdb::discover_movies,
    commands::tmdb::list_genres,
    commands::download::download_target,
    commands::tracker::update_trackers,
    commands::subtitle::fetch_subtitle_cmd,
    commands::subtitle::align_subtitle_cmd,
    commands::subtitle::extract_subtitle_cmd,
    commands::subtitle::list_subtitle_streams_cmd,
    commands::subtitle::shift_subtitle_cmd,
    commands::media::probe_media,
    commands::config::get_config,
    commands::config::set_config_key,
])
```

- [ ] **Step 3: Full cargo build smoke test**

```bash
cd src-tauri && cargo build 2>&1 | tail -5
```

Expected: `Finished dev [unoptimized + debuginfo] target(s)` with no errors. Fix any remaining compilation errors before proceeding.

- [ ] **Step 4: Run all Rust tests**

```bash
cd src-tauri && cargo test 2>&1 | tail -20
```

Expected: all tests pass (search, tmdb, config, subtitle, download, tracker).

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/commands/mod.rs src-tauri/src/lib.rs
git commit -m "feat: register all Tauri commands — Rust backend complete"
```

---

## Task 12: Frontend Tooling Setup

**Files:**
- Create: `package.json`
- Create: `vite.config.ts`
- Create: `tsconfig.json`
- Create: `src/index.html`

- [ ] **Step 1: Write package.json**

```json
{
  "name": "blowup",
  "private": true,
  "version": "2.0.0",
  "type": "module",
  "scripts": {
    "dev": "vite",
    "build": "tsc && vite build",
    "preview": "vite preview",
    "tauri": "tauri"
  },
  "dependencies": {
    "@tauri-apps/api": "^2",
    "@tauri-apps/plugin-dialog": "^2",
    "@tauri-apps/plugin-opener": "^2",
    "react": "^19",
    "react-dom": "^19",
    "react-router-dom": "^7"
  },
  "devDependencies": {
    "@tauri-apps/cli": "^2",
    "@types/react": "^19",
    "@types/react-dom": "^19",
    "@vitejs/plugin-react": "^4",
    "@tailwindcss/vite": "^4",
    "tailwindcss": "^4",
    "typescript": "^5",
    "vite": "^6"
  }
}
```

- [ ] **Step 2: Write vite.config.ts**

```ts
import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";
import tailwindcss from "@tailwindcss/vite";

export default defineConfig({
  plugins: [react(), tailwindcss()],
  clearScreen: false,
  server: {
    port: 1420,
    strictPort: true,
  },
  envPrefix: ["VITE_", "TAURI_"],
  build: {
    target: "chrome105",
    minify: !process.env.TAURI_DEBUG,
    sourcemap: !!process.env.TAURI_DEBUG,
  },
});
```

- [ ] **Step 3: Write tsconfig.json**

```json
{
  "compilerOptions": {
    "target": "ES2021",
    "lib": ["ES2021", "DOM"],
    "module": "ESNext",
    "moduleResolution": "bundler",
    "jsx": "react-jsx",
    "strict": true,
    "noUnusedLocals": true,
    "noUnusedParameters": true,
    "noFallthroughCasesInSwitch": true,
    "outDir": "dist"
  },
  "include": ["src"]
}
```

- [ ] **Step 4: Write src/index.html**

```html
<!DOCTYPE html>
<html lang="zh">
  <head>
    <meta charset="UTF-8" />
    <meta name="viewport" content="width=device-width, initial-scale=1.0" />
    <title>blowup</title>
  </head>
  <body>
    <div id="root"></div>
    <script type="module" src="/src/main.tsx"></script>
  </body>
</html>
```

- [ ] **Step 5: Install npm dependencies**

```bash
npm install
```

Expected: `node_modules/` created, no peer dependency errors.

- [ ] **Step 6: Commit**

```bash
git add package.json vite.config.ts tsconfig.json src/index.html package-lock.json
git commit -m "chore: add frontend tooling (Vite, React 19, Tailwind CSS v4)"
```

---

## Task 13: Design System — CSS Tokens + Base Components

**Files:**
- Create: `src/index.css`
- Create: `src/main.tsx`
- Create: `src/components/ui/NavItem.tsx`
- Create: `src/components/ui/TextInput.tsx`
- Create: `src/components/ui/Chip.tsx`
- Create: `src/components/ui/Button.tsx`

- [ ] **Step 1: Write src/index.css**

```css
@import "tailwindcss";

@theme {
  /* Backgrounds */
  --color-bg-primary:    #0B1628;
  --color-bg-secondary:  #122040;
  --color-bg-elevated:   #1A2E56;
  --color-bg-control:    rgba(255 255 255 / 0.06);

  /* Separators */
  --color-separator:     rgba(100 130 200 / 0.12);

  /* Labels */
  --color-label-primary:    #FFFFFF;
  --color-label-secondary:  rgba(255 255 255 / 0.60);
  --color-label-tertiary:   rgba(255 255 255 / 0.30);
  --color-label-quaternary: rgba(255 255 255 / 0.16);

  /* Accent */
  --color-accent:       #C5A050;
  --color-accent-soft:  rgba(197 160 80 / 0.15);

  /* Font */
  --font-sans: -apple-system, BlinkMacSystemFont, "SF Pro Text",
               "Helvetica Neue", sans-serif;
}

* {
  box-sizing: border-box;
}

body {
  margin: 0;
  background: var(--color-bg-primary);
  color: var(--color-label-primary);
  font-family: var(--font-sans);
  font-size: 14px;
  -webkit-font-smoothing: antialiased;
}

/* Scrollbar */
::-webkit-scrollbar { width: 6px; }
::-webkit-scrollbar-track { background: transparent; }
::-webkit-scrollbar-thumb {
  background: rgba(255 255 255 / 0.12);
  border-radius: 3px;
}
```

- [ ] **Step 2: Write src/main.tsx**

```tsx
import React from "react";
import ReactDOM from "react-dom/client";
import { HashRouter } from "react-router-dom";
import App from "./App";
import "./index.css";

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <HashRouter>
      <App />
    </HashRouter>
  </React.StrictMode>
);
```

- [ ] **Step 3: Write NavItem.tsx**

```tsx
// src/components/ui/NavItem.tsx
interface NavItemProps {
  icon: string;
  label: string;
  active?: boolean;
  disabled?: boolean;
  onClick?: () => void;
}

export function NavItem({ icon, label, active, disabled, onClick }: NavItemProps) {
  return (
    <button
      onClick={disabled ? undefined : onClick}
      style={{
        display: "flex",
        alignItems: "center",
        gap: "0.55rem",
        width: "100%",
        padding: "0.42rem 0.75rem",
        borderRadius: "6px",
        border: "none",
        cursor: disabled ? "default" : "pointer",
        fontSize: "0.82rem",
        fontFamily: "inherit",
        background: active ? "var(--color-bg-elevated)" : "transparent",
        color: disabled
          ? "var(--color-label-quaternary)"
          : active
          ? "var(--color-label-primary)"
          : "var(--color-label-secondary)",
        fontWeight: active ? 500 : 400,
        pointerEvents: disabled ? "none" : "auto",
        opacity: disabled ? 0.25 : 1,
        textAlign: "left",
      }}
    >
      <span style={{ width: 15, textAlign: "center", fontSize: "0.82rem" }}>
        {icon}
      </span>
      {label}
    </button>
  );
}
```

- [ ] **Step 4: Write TextInput.tsx**

```tsx
// src/components/ui/TextInput.tsx
interface TextInputProps extends React.InputHTMLAttributes<HTMLInputElement> {
  leadingIcon?: React.ReactNode;
}

export function TextInput({ leadingIcon, style, ...props }: TextInputProps) {
  return (
    <div
      style={{
        display: "flex",
        alignItems: "center",
        gap: "0.5rem",
        background: "var(--color-bg-control)",
        border: "1px solid var(--color-separator)",
        borderRadius: "8px",
        padding: "0 0.75rem",
        height: "34px",
        ...style,
      }}
    >
      {leadingIcon && (
        <span style={{ color: "var(--color-label-tertiary)", fontSize: "0.82rem" }}>
          {leadingIcon}
        </span>
      )}
      <input
        {...props}
        style={{
          background: "none",
          border: "none",
          outline: "none",
          color: "var(--color-label-primary)",
          fontSize: "0.85rem",
          fontFamily: "inherit",
          flex: 1,
          width: 0,
        }}
      />
    </div>
  );
}
```

- [ ] **Step 5: Write Chip.tsx**

```tsx
// src/components/ui/Chip.tsx
interface ChipProps {
  label: string;
  active?: boolean;
  onRemove?: () => void;
  onClick?: () => void;
}

export function Chip({ label, active, onRemove, onClick }: ChipProps) {
  return (
    <button
      onClick={onClick}
      style={{
        display: "inline-flex",
        alignItems: "center",
        gap: "0.3rem",
        background: active ? "var(--color-accent-soft)" : "var(--color-bg-control)",
        border: `1px solid ${active ? "rgba(197,160,80,0.3)" : "var(--color-separator)"}`,
        borderRadius: "100px",
        padding: "0.18rem 0.6rem",
        fontSize: "0.72rem",
        color: active ? "var(--color-accent)" : "var(--color-label-secondary)",
        cursor: "pointer",
        fontFamily: "inherit",
      }}
    >
      {label}
      {onRemove && (
        <span
          onClick={(e) => { e.stopPropagation(); onRemove(); }}
          style={{ opacity: 0.6, fontSize: "0.65rem" }}
        >
          ✕
        </span>
      )}
    </button>
  );
}
```

- [ ] **Step 6: Write Button.tsx**

```tsx
// src/components/ui/Button.tsx
interface ButtonProps extends React.ButtonHTMLAttributes<HTMLButtonElement> {
  variant?: "primary" | "ghost";
}

export function Button({ variant = "ghost", style, children, ...props }: ButtonProps) {
  return (
    <button
      {...props}
      style={{
        display: "inline-flex",
        alignItems: "center",
        gap: "0.35rem",
        padding: "0.35rem 0.75rem",
        borderRadius: "6px",
        border: "none",
        cursor: props.disabled ? "default" : "pointer",
        fontFamily: "inherit",
        fontSize: "0.82rem",
        background:
          variant === "primary"
            ? "var(--color-accent)"
            : "var(--color-bg-control)",
        color:
          variant === "primary"
            ? "#000"
            : "var(--color-label-secondary)",
        opacity: props.disabled ? 0.25 : 1,
        pointerEvents: props.disabled ? "none" : "auto",
        ...style,
      }}
    >
      {children}
    </button>
  );
}
```

- [ ] **Step 7: Commit**

```bash
git add src/
git commit -m "feat: add design system CSS tokens and base UI components"
```

---

## Task 14: App Shell — Sidebar + Router

**Files:**
- Create: `src/App.tsx`
- Create: `src/pages/Placeholder.tsx`

- [ ] **Step 1: Write Placeholder.tsx**

```tsx
// src/pages/Placeholder.tsx
interface PlaceholderProps {
  title: string;
  milestone: string;
}

export default function Placeholder({ title, milestone }: PlaceholderProps) {
  return (
    <div
      style={{
        display: "flex",
        flexDirection: "column",
        alignItems: "center",
        justifyContent: "center",
        height: "100%",
        gap: "0.5rem",
      }}
    >
      <p style={{ color: "var(--color-label-tertiary)", fontSize: "1rem", fontWeight: 600 }}>
        {title}
      </p>
      <p style={{ color: "var(--color-label-quaternary)", fontSize: "0.78rem" }}>
        {milestone} 中实现
      </p>
    </div>
  );
}
```

- [ ] **Step 2: Write App.tsx**

```tsx
// src/App.tsx
import { Routes, Route, useNavigate, useLocation } from "react-router-dom";
import { NavItem } from "./components/ui/NavItem";
import Search from "./pages/Search";
import Settings from "./pages/Settings";
import Placeholder from "./pages/Placeholder";

const NAV_SECTIONS = [
  {
    label: null,
    items: [{ icon: "⌕", label: "搜索", path: "/" }],
  },
  {
    label: "知识库",
    items: [
      { icon: "◎", label: "影人", path: "/people", disabled: true },
      { icon: "◈", label: "流派", path: "/genres", disabled: true },
      { icon: "⋯", label: "关系图", path: "/graph", disabled: true },
    ],
  },
  {
    label: "资源",
    items: [
      { icon: "⊞", label: "我的库", path: "/library", disabled: true },
      { icon: "↓", label: "下载", path: "/download", disabled: true },
    ],
  },
  {
    label: "工具",
    items: [
      { icon: "◷", label: "字幕", path: "/subtitle", disabled: true },
      { icon: "▶", label: "媒体", path: "/media", disabled: true },
    ],
  },
];

export default function App() {
  const navigate = useNavigate();
  const { pathname } = useLocation();

  return (
    <div style={{ display: "flex", height: "100vh", overflow: "hidden" }}>
      {/* Sidebar */}
      <aside
        style={{
          width: 188,
          flexShrink: 0,
          background: "var(--color-bg-secondary)",
          borderRight: "1px solid var(--color-separator)",
          display: "flex",
          flexDirection: "column",
          padding: "1rem 0.5rem",
          gap: 1,
        }}
      >
        {NAV_SECTIONS.map((section, si) => (
          <div key={si}>
            {section.label && (
              <p
                style={{
                  fontSize: "0.62rem",
                  color: "var(--color-label-quaternary)",
                  letterSpacing: "0.08em",
                  textTransform: "uppercase",
                  padding: "0.85rem 0.75rem 0.3rem",
                  margin: 0,
                }}
              >
                {section.label}
              </p>
            )}
            {section.items.map((item) => (
              <NavItem
                key={item.path}
                icon={item.icon}
                label={item.label}
                active={pathname === item.path}
                disabled={"disabled" in item && item.disabled}
                onClick={() => navigate(item.path)}
              />
            ))}
          </div>
        ))}

        {/* Bottom: Settings */}
        <div
          style={{
            marginTop: "auto",
            borderTop: "1px solid var(--color-separator)",
            paddingTop: "0.5rem",
          }}
        >
          <NavItem
            icon="⚙"
            label="设置"
            active={pathname === "/settings"}
            onClick={() => navigate("/settings")}
          />
        </div>
      </aside>

      {/* Content */}
      <main style={{ flex: 1, overflow: "hidden", display: "flex", flexDirection: "column" }}>
        <Routes>
          <Route path="/" element={<Search />} />
          <Route path="/settings" element={<Settings />} />
          <Route path="/people"   element={<Placeholder title="影人" milestone="M2" />} />
          <Route path="/genres"   element={<Placeholder title="流派" milestone="M2" />} />
          <Route path="/graph"    element={<Placeholder title="关系图" milestone="M2" />} />
          <Route path="/library"  element={<Placeholder title="我的库" milestone="M3" />} />
          <Route path="/download" element={<Placeholder title="下载" milestone="M3" />} />
          <Route path="/subtitle" element={<Placeholder title="字幕" milestone="M4" />} />
          <Route path="/media"    element={<Placeholder title="媒体工具" milestone="M4" />} />
        </Routes>
      </main>
    </div>
  );
}
```

- [ ] **Step 3: Quick dev server check (frontend only)**

```bash
npm run dev
```

Open http://localhost:1420. Should show a sidebar with "搜索" active, content area empty (Search page not yet written). Fix any TypeScript errors shown in the terminal.

- [ ] **Step 4: Commit**

```bash
git add src/App.tsx src/pages/Placeholder.tsx
git commit -m "feat: add app shell with sidebar navigation and routing"
```

---

## Task 15: lib/tauri.ts — Typed Invoke Wrappers

**Files:**
- Create: `src/lib/tauri.ts`

- [ ] **Step 1: Write tauri.ts**

```ts
// src/lib/tauri.ts
import { invoke } from "@tauri-apps/api/core";

export interface SearchFilters {
  year_from?: number;
  year_to?: number;
  genre_ids: number[];
  min_rating?: number;
  sort_by?: string;
  page?: number;
}

export interface MovieListItem {
  id: number;
  title: string;
  original_title: string;
  year: string;
  overview: string;
  vote_average: number;
  poster_path: string | null;
  genre_ids: number[];
}

export interface TmdbGenre {
  id: number;
  name: string;
}

export interface AppConfig {
  tools: { aria2c: string; alass: string; ffmpeg: string };
  search: { rate_limit_secs: number };
  subtitle: { default_lang: string };
  opensubtitles: { api_key: string };
  tmdb: { api_key: string };
  library: { root_dir: string };
}

export const tmdb = {
  searchMovies: (apiKey: string, query: string, filters: SearchFilters) =>
    invoke<MovieListItem[]>("search_movies", { apiKey, query, filters }),

  discoverMovies: (apiKey: string, filters: SearchFilters) =>
    invoke<MovieListItem[]>("discover_movies", { apiKey, filters }),

  listGenres: (apiKey: string) =>
    invoke<TmdbGenre[]>("list_genres", { apiKey }),
};

export const config = {
  get: () => invoke<AppConfig>("get_config"),
  set: (key: string, value: string) =>
    invoke<void>("set_config_key", { key, value }),
};
```

- [ ] **Step 2: Commit**

```bash
git add src/lib/tauri.ts
git commit -m "feat: add typed Tauri invoke wrappers"
```

---

## Task 16: Search Page

**Files:**
- Create: `src/pages/Search.tsx`

- [ ] **Step 1: Write Search.tsx**

```tsx
// src/pages/Search.tsx
import { useState, useEffect, useRef, useCallback } from "react";
import { TextInput } from "../components/ui/TextInput";
import { Chip } from "../components/ui/Chip";
import { tmdb, config, type MovieListItem, type TmdbGenre, type SearchFilters } from "../lib/tauri";

const SORT_OPTIONS = [
  { value: "vote_average.desc", label: "按评分排序" },
  { value: "popularity.desc",   label: "按热度排序" },
  { value: "release_date.desc", label: "按年份排序" },
];

export default function Search() {
  const [apiKey, setApiKey] = useState("");
  const [query, setQuery] = useState("");
  const [genres, setGenres] = useState<TmdbGenre[]>([]);
  const [results, setResults] = useState<MovieListItem[]>([]);
  const [selected, setSelected] = useState<MovieListItem | null>(null);
  const [loading, setLoading] = useState(false);
  const [page, setPage] = useState(1);
  const [hasMore, setHasMore] = useState(false);

  // Filters
  const [yearFrom, setYearFrom] = useState<number | undefined>();
  const [yearTo, setYearTo]     = useState<number | undefined>();
  const [genreIds, setGenreIds] = useState<number[]>([]);
  const [minRating, setMinRating] = useState<number | undefined>();
  const [sortBy, setSortBy]     = useState("vote_average.desc");

  const searchTimer = useRef<ReturnType<typeof setTimeout>>();

  // Load API key and genre list on mount
  useEffect(() => {
    config.get().then((cfg) => {
      setApiKey(cfg.tmdb.api_key);
      if (cfg.tmdb.api_key) {
        tmdb.listGenres(cfg.tmdb.api_key).then(setGenres).catch(() => {});
      }
    });
  }, []);

  const buildFilters = useCallback(
    (p = 1): SearchFilters => ({
      year_from: yearFrom,
      year_to: yearTo,
      genre_ids: genreIds,
      min_rating: minRating,
      sort_by: sortBy,
      page: p,
    }),
    [yearFrom, yearTo, genreIds, minRating, sortBy]
  );

  const runSearch = useCallback(
    async (q: string, p: number, append: boolean) => {
      if (!apiKey) return;
      setLoading(true);
      try {
        const filters = buildFilters(p);
        const rows =
          q.trim()
            ? await tmdb.searchMovies(apiKey, q.trim(), filters)
            : await tmdb.discoverMovies(apiKey, filters);

        setResults((prev) => (append ? [...prev, ...rows] : rows));
        setHasMore(rows.length === 20);
        setPage(p);
      } catch (e) {
        console.error(e);
      } finally {
        setLoading(false);
      }
    },
    [apiKey, buildFilters]
  );

  // Debounced search on query / filter change
  useEffect(() => {
    clearTimeout(searchTimer.current);
    searchTimer.current = setTimeout(() => runSearch(query, 1, false), 400);
    return () => clearTimeout(searchTimer.current);
  }, [query, yearFrom, yearTo, genreIds, minRating, sortBy, apiKey]);

  const loadMore = () => runSearch(query, page + 1, true);

  // Selected genre names for chip display
  const selectedGenreNames = genres
    .filter((g) => genreIds.includes(g.id))
    .map((g) => g.name);

  return (
    <div style={{ display: "flex", height: "100%", overflow: "hidden" }}>
      {/* Left: search + list */}
      <div style={{ flex: 1, display: "flex", flexDirection: "column", overflow: "hidden" }}>
        {/* Header */}
        <div style={{ padding: "1.4rem 1.5rem 0" }}>
          <h1
            style={{
              fontSize: "1.6rem",
              fontWeight: 700,
              letterSpacing: "-0.035em",
              marginBottom: "1.1rem",
            }}
          >
            搜索
          </h1>

          <TextInput
            leadingIcon="⌕"
            placeholder="电影名称、导演…"
            value={query}
            onChange={(e) => setQuery(e.target.value)}
            style={{ marginBottom: "0.7rem" }}
          />

          {/* Filter chips */}
          <div style={{ display: "flex", gap: "0.4rem", flexWrap: "wrap", marginBottom: "1rem" }}>
            {/* Year range */}
            <Chip
              label={yearFrom || yearTo ? `${yearFrom ?? "?"} – ${yearTo ?? "?"}` : "年代"}
              active={!!(yearFrom || yearTo)}
              onRemove={
                yearFrom || yearTo
                  ? () => { setYearFrom(undefined); setYearTo(undefined); }
                  : undefined
              }
              onClick={() => {
                const from = prompt("起始年份 (留空跳过)");
                const to   = prompt("结束年份 (留空跳过)");
                setYearFrom(from ? parseInt(from) : undefined);
                setYearTo(to   ? parseInt(to)   : undefined);
              }}
            />

            {/* Genres */}
            {selectedGenreNames.length > 0
              ? selectedGenreNames.map((name, i) => (
                  <Chip
                    key={genreIds[i]}
                    label={name}
                    active
                    onRemove={() =>
                      setGenreIds((ids) => ids.filter((id) => id !== genreIds[i]))
                    }
                  />
                ))
              : (
                <Chip
                  label="类型"
                  onClick={() => {
                    const names = genres.map((g, i) => `${i + 1}. ${g.name}`).join("\n");
                    const pick = prompt(`选择类型序号（逗号分隔）:\n${names}`);
                    if (pick) {
                      const ids = pick.split(",")
                        .map((s) => genres[parseInt(s.trim()) - 1]?.id)
                        .filter(Boolean) as number[];
                      setGenreIds(ids);
                    }
                  }}
                />
              )}

            {/* Rating */}
            <Chip
              label={minRating ? `≥ ${minRating}` : "评分"}
              active={!!minRating}
              onRemove={minRating ? () => setMinRating(undefined) : undefined}
              onClick={() => {
                const r = prompt("最低评分 (0–10)");
                setMinRating(r ? parseFloat(r) : undefined);
              }}
            />

            {/* Sort */}
            <Chip
              label={SORT_OPTIONS.find((o) => o.value === sortBy)?.label ?? "排序"}
              active
              onClick={() => {
                const opts = SORT_OPTIONS.map((o, i) => `${i + 1}. ${o.label}`).join("\n");
                const pick = prompt(`排序方式:\n${opts}`);
                if (pick) {
                  const opt = SORT_OPTIONS[parseInt(pick) - 1];
                  if (opt) setSortBy(opt.value);
                }
              }}
            />
          </div>
        </div>

        {/* Divider */}
        <div style={{ height: 1, background: "var(--color-separator)", margin: "0 1.5rem" }} />

        {/* Results */}
        <div style={{ flex: 1, overflowY: "auto", padding: "0.9rem 1.5rem" }}>
          {!apiKey && (
            <p style={{ color: "var(--color-label-tertiary)", fontSize: "0.82rem" }}>
              请先在设置中配置 TMDB API Key。
            </p>
          )}

          {results.map((film) => (
            <FilmRow
              key={film.id}
              film={film}
              selected={selected?.id === film.id}
              onClick={() => setSelected(film)}
            />
          ))}

          {loading && (
            <p style={{ color: "var(--color-label-tertiary)", fontSize: "0.75rem", padding: "0.5rem 0" }}>
              加载中…
            </p>
          )}

          {hasMore && !loading && (
            <button
              onClick={loadMore}
              style={{
                background: "none",
                border: "none",
                color: "var(--color-label-tertiary)",
                fontSize: "0.75rem",
                cursor: "pointer",
                padding: "0.5rem 0",
                fontFamily: "inherit",
              }}
            >
              加载更多
            </button>
          )}
        </div>
      </div>

      {/* Right: detail panel */}
      {selected && (
        <FilmDetailPanel film={selected} onClose={() => setSelected(null)} />
      )}
    </div>
  );
}

// ── FilmRow ──────────────────────────────────────────────────────
function FilmRow({
  film,
  selected,
  onClick,
}: {
  film: MovieListItem;
  selected: boolean;
  onClick: () => void;
}) {
  return (
    <div
      onClick={onClick}
      style={{
        display: "flex",
        alignItems: "center",
        gap: "0.85rem",
        padding: "0.55rem 0.6rem",
        borderRadius: "7px",
        cursor: "pointer",
        background: selected ? "var(--color-bg-elevated)" : "transparent",
      }}
      onMouseEnter={(e) => {
        if (!selected)
          (e.currentTarget as HTMLDivElement).style.background =
            "rgba(255,255,255,0.04)";
      }}
      onMouseLeave={(e) => {
        if (!selected)
          (e.currentTarget as HTMLDivElement).style.background = "transparent";
      }}
    >
      {/* Poster */}
      <div
        style={{
          width: 34,
          height: 48,
          background: "var(--color-bg-elevated)",
          borderRadius: 3,
          flexShrink: 0,
          overflow: "hidden",
          display: "flex",
          alignItems: "center",
          justifyContent: "center",
          color: "var(--color-label-quaternary)",
          fontSize: "0.9rem",
        }}
      >
        {film.poster_path ? (
          <img
            src={`https://image.tmdb.org/t/p/w92${film.poster_path}`}
            alt=""
            style={{ width: "100%", height: "100%", objectFit: "cover" }}
          />
        ) : (
          "🎬"
        )}
      </div>

      {/* Info */}
      <div style={{ flex: 1, minWidth: 0 }}>
        <p
          style={{
            margin: 0,
            fontSize: "0.86rem",
            fontWeight: 500,
            letterSpacing: "-0.01em",
            whiteSpace: "nowrap",
            overflow: "hidden",
            textOverflow: "ellipsis",
          }}
        >
          {film.title}
        </p>
        <p
          style={{
            margin: "0.12rem 0 0",
            fontSize: "0.7rem",
            color: "var(--color-label-tertiary)",
          }}
        >
          {[film.year].filter(Boolean).join(" · ")}
        </p>
      </div>

      {/* Score */}
      <span style={{ fontSize: "0.72rem", color: "var(--color-label-tertiary)", flexShrink: 0 }}>
        <strong style={{ color: "var(--color-label-secondary)", fontWeight: 500, fontSize: "0.8rem" }}>
          {film.vote_average.toFixed(1)}
        </strong>{" "}
        / 10
      </span>
    </div>
  );
}

// ── FilmDetailPanel ───────────────────────────────────────────────
function FilmDetailPanel({
  film,
  onClose,
}: {
  film: MovieListItem;
  onClose: () => void;
}) {
  return (
    <div
      style={{
        width: 300,
        flexShrink: 0,
        borderLeft: "1px solid var(--color-separator)",
        background: "var(--color-bg-secondary)",
        overflowY: "auto",
        padding: "1.25rem 1.25rem 2rem",
        display: "flex",
        flexDirection: "column",
        gap: "0.75rem",
      }}
    >
      {/* Close */}
      <div style={{ display: "flex", justifyContent: "flex-end" }}>
        <button
          onClick={onClose}
          style={{
            background: "none",
            border: "none",
            color: "var(--color-label-tertiary)",
            cursor: "pointer",
            fontSize: "1rem",
            lineHeight: 1,
            padding: 0,
          }}
        >
          ✕
        </button>
      </div>

      {/* Poster */}
      {film.poster_path && (
        <img
          src={`https://image.tmdb.org/t/p/w300${film.poster_path}`}
          alt={film.title}
          style={{ width: "100%", borderRadius: 6 }}
        />
      )}

      {/* Title */}
      <div>
        <h2 style={{ margin: 0, fontSize: "1rem", fontWeight: 700, letterSpacing: "-0.02em" }}>
          {film.title}
        </h2>
        {film.original_title !== film.title && (
          <p style={{ margin: "0.15rem 0 0", fontSize: "0.72rem", color: "var(--color-label-tertiary)" }}>
            {film.original_title}
          </p>
        )}
      </div>

      {/* Meta */}
      <div style={{ display: "flex", gap: "0.5rem", alignItems: "center" }}>
        {film.year && (
          <span style={{ fontSize: "0.75rem", color: "var(--color-label-secondary)" }}>
            {film.year}
          </span>
        )}
        <span style={{ fontSize: "0.75rem", color: "var(--color-label-tertiary)" }}>·</span>
        <span style={{ fontSize: "0.75rem", color: "var(--color-accent)", fontWeight: 500 }}>
          ★ {film.vote_average.toFixed(1)}
        </span>
      </div>

      {/* Overview */}
      <p
        style={{
          margin: 0,
          fontSize: "0.78rem",
          color: "var(--color-label-secondary)",
          lineHeight: 1.6,
        }}
      >
        {film.overview || "暂无简介。"}
      </p>

      {/* Actions — disabled until M2/M3 */}
      <div
        style={{
          borderTop: "1px solid var(--color-separator)",
          paddingTop: "0.75rem",
          display: "flex",
          flexDirection: "column",
          gap: "0.4rem",
        }}
      >
        <button
          disabled
          style={{
            background: "var(--color-bg-control)",
            border: "none",
            borderRadius: 6,
            padding: "0.4rem 0.75rem",
            color: "var(--color-label-quaternary)",
            fontSize: "0.78rem",
            cursor: "default",
            fontFamily: "inherit",
            textAlign: "left",
            opacity: 0.4,
          }}
        >
          加入知识库（M2）
        </button>
        <button
          disabled
          style={{
            background: "var(--color-bg-control)",
            border: "none",
            borderRadius: 6,
            padding: "0.4rem 0.75rem",
            color: "var(--color-label-quaternary)",
            fontSize: "0.78rem",
            cursor: "default",
            fontFamily: "inherit",
            textAlign: "left",
            opacity: 0.4,
          }}
        >
          搜索资源（M3）
        </button>
      </div>
    </div>
  );
}
```

- [ ] **Step 2: Commit**

```bash
git add src/pages/Search.tsx
git commit -m "feat: add TMDB search page with filter chips and detail panel"
```

---

## Task 17: Settings Page

**Files:**
- Create: `src/pages/Settings.tsx`

- [ ] **Step 1: Write Settings.tsx**

```tsx
// src/pages/Settings.tsx
import { useState, useEffect } from "react";
import { open } from "@tauri-apps/plugin-dialog";
import { TextInput } from "../components/ui/TextInput";
import { Button } from "../components/ui/Button";
import { config, type AppConfig } from "../lib/tauri";

const LANG_OPTIONS = [
  { value: "zh", label: "中文 (zh)" },
  { value: "en", label: "English (en)" },
  { value: "ja", label: "日本語 (ja)" },
];

export default function Settings() {
  const [cfg, setCfg]       = useState<AppConfig | null>(null);
  const [showKey, setShowKey] = useState(false);
  const [saving, setSaving]   = useState<string | null>(null);

  useEffect(() => {
    config.get().then(setCfg);
  }, []);

  const save = async (key: string, value: string) => {
    setSaving(key);
    try {
      await config.set(key, value);
      setCfg((prev) => {
        if (!prev) return prev;
        const [section, field] = key.split(".");
        return { ...prev, [section]: { ...(prev as never)[section], [field]: value } };
      });
    } catch (e) {
      console.error(e);
    } finally {
      setSaving(null);
    }
  };

  const pickDir = async () => {
    const dir = await open({ directory: true, multiple: false });
    if (typeof dir === "string") save("library.root_dir", dir);
  };

  if (!cfg) {
    return (
      <div style={{ padding: "2rem", color: "var(--color-label-tertiary)", fontSize: "0.82rem" }}>
        加载中…
      </div>
    );
  }

  return (
    <div style={{ flex: 1, overflowY: "auto", padding: "1.4rem 1.75rem 3rem" }}>
      <h1
        style={{
          fontSize: "1.6rem",
          fontWeight: 700,
          letterSpacing: "-0.035em",
          marginBottom: "2rem",
        }}
      >
        设置
      </h1>

      <Section title="TMDB">
        <Field label="API Key">
          <div style={{ display: "flex", gap: "0.5rem" }}>
            <TextInput
              type={showKey ? "text" : "password"}
              defaultValue={cfg.tmdb.api_key}
              placeholder="在 themoviedb.org 免费申请"
              style={{ flex: 1 }}
              onBlur={(e) => save("tmdb.api_key", e.currentTarget.value)}
            />
            <Button onClick={() => setShowKey((v) => !v)}>
              {showKey ? "隐藏" : "显示"}
            </Button>
          </div>
        </Field>
      </Section>

      <Section title="字幕">
        <Field label="默认语言">
          <select
            value={cfg.subtitle.default_lang}
            onChange={(e) => save("subtitle.default_lang", e.target.value)}
            style={{
              background: "var(--color-bg-control)",
              border: "1px solid var(--color-separator)",
              borderRadius: 8,
              padding: "0 0.75rem",
              height: 34,
              color: "var(--color-label-primary)",
              fontSize: "0.85rem",
              fontFamily: "inherit",
            }}
          >
            {LANG_OPTIONS.map((o) => (
              <option key={o.value} value={o.value}>
                {o.label}
              </option>
            ))}
          </select>
        </Field>
      </Section>

      <Section title="工具路径">
        {(["aria2c", "alass", "ffmpeg"] as const).map((tool) => (
          <Field key={tool} label={tool}>
            <TextInput
              defaultValue={cfg.tools[tool]}
              placeholder={tool}
              onBlur={(e) => save(`tools.${tool}`, e.currentTarget.value)}
            />
          </Field>
        ))}
      </Section>

      <Section title="库目录">
        <Field label="本地库根目录">
          <div style={{ display: "flex", gap: "0.5rem" }}>
            <TextInput
              value={cfg.library.root_dir}
              readOnly
              style={{ flex: 1 }}
              onChange={() => {}}
            />
            <Button onClick={pickDir}>选择…</Button>
          </div>
        </Field>
      </Section>

      {saving && (
        <p style={{ color: "var(--color-label-tertiary)", fontSize: "0.72rem", marginTop: "1rem" }}>
          保存中…
        </p>
      )}
    </div>
  );
}

function Section({ title, children }: { title: string; children: React.ReactNode }) {
  return (
    <div style={{ marginBottom: "2rem" }}>
      <p
        style={{
          margin: "0 0 0.75rem",
          fontSize: "0.7rem",
          color: "var(--color-label-quaternary)",
          letterSpacing: "0.08em",
          textTransform: "uppercase",
        }}
      >
        {title}
      </p>
      <div
        style={{
          background: "var(--color-bg-secondary)",
          border: "1px solid var(--color-separator)",
          borderRadius: 10,
          padding: "0.1rem 1rem",
        }}
      >
        {children}
      </div>
    </div>
  );
}

function Field({ label, children }: { label: string; children: React.ReactNode }) {
  return (
    <div
      style={{
        display: "flex",
        alignItems: "center",
        gap: "1rem",
        padding: "0.65rem 0",
        borderBottom: "1px solid var(--color-separator)",
      }}
    >
      <span
        style={{
          width: 120,
          flexShrink: 0,
          fontSize: "0.82rem",
          color: "var(--color-label-secondary)",
        }}
      >
        {label}
      </span>
      <div style={{ flex: 1 }}>{children}</div>
    </div>
  );
}
```

- [ ] **Step 2: Commit**

```bash
git add src/pages/Settings.tsx
git commit -m "feat: add Settings page with config read/write and directory picker"
```

---

## Task 18: End-to-End Integration + Final Commit

- [ ] **Step 1: Run full Tauri dev build**

```bash
npm run tauri dev
```

Expected: window opens showing blowup app with sidebar. Navigate to 搜索, enter a movie name — results should appear (requires TMDB key in settings). Navigate to 设置, verify all fields load.

- [ ] **Step 2: Verify Rust tests still pass**

```bash
cd src-tauri && cargo test 2>&1 | tail -10
```

Expected: all tests pass.

- [ ] **Step 3: Check TypeScript**

```bash
npm run build 2>&1 | tail -10
```

Expected: no TypeScript errors, `dist/` output created.

- [ ] **Step 4: Final commit**

```bash
git add -A
git commit -m "feat: blowup v2 M1 complete — Tauri v2 desktop app with TMDB search and settings"
```

---

## Spec Coverage Checklist

| Spec requirement | Task |
|------------------|------|
| Tauri v2 scaffolding | T1–T4 |
| React 19 + TypeScript + Tailwind CSS v4 | T12–T13 |
| Apple HIG 青金石主题设计系统 | T13 |
| 固定侧边栏 Shell | T14 |
| M2-M4 模块占位置灰 | T14 |
| SQLite 建库（完整 schema，空表） | T5–T6 |
| 现有 CLI 模块迁移为 Tauri commands | T1, T7–T11 |
| library.root_dir 配置项 | T10 |
| TMDB 高级搜索（双路搜索 + 4 种过滤器）| T8, T15–T16 |
| 详情面板（加入知识库/搜索资源置灰）| T16 |
| Settings 页（全部配置项 + 文件夹选择）| T10, T17 |
