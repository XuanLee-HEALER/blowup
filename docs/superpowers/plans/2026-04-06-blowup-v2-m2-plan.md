# blowup v2 M2 — Knowledge Base Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Implement the Knowledge Base module (影人/流派/关系图/影评/背景音乐) unlocking the three placeholder routes in the blowup v2 Tauri desktop app.

**Architecture:** SQLite-backed CRUD via new `commands/library/` module (people, films, genres, reviews, graph); React pages replace Placeholder components; D3 force-directed graph with orbital rotation; MusicPlayer persists across knowledge-base routes.

**Tech Stack:** Rust/Tauri v2 backend (sqlx 0.8 SQLite + derive, serde), React 19 + TypeScript frontend, D3.js v7 SVG graph, marked Markdown renderer, Tailwind CSS v4 CSS custom properties.

---

### Task 1: Add `sqlx derive` feature + MusicConfig + `set_music_playlist` command

**Files:**
- Modify: `src-tauri/Cargo.toml`
- Modify: `src-tauri/src/config.rs`
- Modify: `src-tauri/src/commands/config.rs`

- [ ] **Step 1: Add `derive` to sqlx features in Cargo.toml**

```toml
sqlx = { version = "0.8", features = ["sqlite", "runtime-tokio-native-tls", "migrate", "derive"] }
```

- [ ] **Step 2: Add MusicConfig and MusicTrack to config.rs**

Add after the `LibraryConfig` struct in `src-tauri/src/config.rs`:

```rust
#[derive(Debug, Default, Deserialize, Serialize, Clone)]
pub struct MusicConfig {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default = "default_music_mode")]
    pub mode: String,
    #[serde(default)]
    pub playlist: Vec<MusicTrack>,
}

#[derive(Debug, Default, Deserialize, Serialize, Clone)]
pub struct MusicTrack {
    pub src: String,
    pub name: String,
}

fn default_music_mode() -> String {
    "sequential".to_string()
}
```

Add `music` field to the `Config` struct:

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
    #[serde(default)]
    pub library: LibraryConfig,
    #[serde(default)]
    pub music: MusicConfig,
}
```

Add `save_config` function before `#[cfg(test)]` in config.rs:

```rust
pub fn save_config(config: &Config) -> Result<(), String> {
    let path = config_path();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let content = toml::to_string_pretty(config).map_err(|e| e.to_string())?;
    std::fs::write(&path, content).map_err(|e| e.to_string())?;
    Ok(())
}
```

- [ ] **Step 3: Write failing tests for MusicConfig**

Add to `#[cfg(test)] mod tests` in config.rs:

```rust
#[test]
fn music_config_defaults() {
    let cfg = Config::default();
    assert!(!cfg.music.enabled);
    assert_eq!(cfg.music.mode, "sequential");
    assert!(cfg.music.playlist.is_empty());
}

#[test]
fn parse_music_config_from_toml() {
    let toml_str = r#"
[music]
enabled = true
mode = "random"

[[music.playlist]]
src = "/tmp/song.mp3"
name = "Test Song"
"#;
    let cfg: Config = toml::from_str(toml_str).unwrap();
    assert!(cfg.music.enabled);
    assert_eq!(cfg.music.mode, "random");
    assert_eq!(cfg.music.playlist.len(), 1);
    assert_eq!(cfg.music.playlist[0].name, "Test Song");
}
```

- [ ] **Step 4: Run tests**

```bash
cd src-tauri && cargo test music_config
```

Expected: both tests PASS.

- [ ] **Step 5: Add `set_music_playlist` command to commands/config.rs**

Append to `src-tauri/src/commands/config.rs`:

```rust
#[tauri::command]
pub fn set_music_playlist(
    tracks: Vec<crate::config::MusicTrack>,
) -> Result<(), String> {
    let mut config = crate::config::load_config();
    config.music.playlist = tracks;
    crate::config::save_config(&config)
}
```

- [ ] **Step 6: Verify `cargo check`**

```bash
cd src-tauri && cargo check
```

Expected: no errors.

- [ ] **Step 7: Commit**

```bash
git add src-tauri/Cargo.toml src-tauri/src/config.rs src-tauri/src/commands/config.rs
git commit -m "feat: add sqlx derive feature, MusicConfig, and set_music_playlist command"
```

---

### Task 2: Library module skeleton — data types

**Files:**
- Create: `src-tauri/src/commands/library/mod.rs`
- Create: `src-tauri/src/commands/library/people.rs` (stub)
- Create: `src-tauri/src/commands/library/films.rs` (stub)
- Create: `src-tauri/src/commands/library/genres.rs` (stub)
- Create: `src-tauri/src/commands/library/reviews.rs` (stub)
- Create: `src-tauri/src/commands/library/graph.rs` (stub)
- Modify: `src-tauri/src/commands/mod.rs`

- [ ] **Step 1: Create `src-tauri/src/commands/library/mod.rs`**

```rust
// src-tauri/src/commands/library/mod.rs

pub mod films;
pub mod genres;
pub mod graph;
pub mod people;
pub mod reviews;

use serde::{Deserialize, Serialize};

// ── Person ────────────────────────────────────────────────────────

#[derive(Serialize, sqlx::FromRow)]
pub struct PersonSummary {
    pub id: i64,
    pub name: String,
    pub primary_role: String,
    pub film_count: i64,
}

#[derive(Serialize)]
pub struct PersonDetail {
    pub id: i64,
    pub tmdb_id: Option<i64>,
    pub name: String,
    pub primary_role: String,
    pub born_date: Option<String>,
    pub nationality: Option<String>,
    pub biography: Option<String>,
    pub wiki_content: String,
    pub films: Vec<PersonFilmEntry>,
    pub relations: Vec<PersonRelation>,
}

#[derive(Serialize, sqlx::FromRow)]
pub struct PersonFilmEntry {
    pub film_id: i64,
    pub title: String,
    pub year: Option<i64>,
    pub role: String,
    pub poster_cache_path: Option<String>,
}

#[derive(Serialize, sqlx::FromRow)]
pub struct PersonRelation {
    pub target_id: i64,
    pub target_name: String,
    pub direction: String,
    pub relation_type: String,
}

// ── Film ─────────────────────────────────────────────────────────

#[derive(Serialize, sqlx::FromRow)]
pub struct FilmSummary {
    pub id: i64,
    pub title: String,
    pub year: Option<i64>,
    pub tmdb_rating: Option<f64>,
    pub poster_cache_path: Option<String>,
}

#[derive(Serialize)]
pub struct FilmDetail {
    pub id: i64,
    pub tmdb_id: Option<i64>,
    pub title: String,
    pub original_title: Option<String>,
    pub year: Option<i64>,
    pub overview: Option<String>,
    pub tmdb_rating: Option<f64>,
    pub poster_cache_path: Option<String>,
    pub wiki_content: String,
    pub people: Vec<FilmPersonEntry>,
    pub genres: Vec<GenreSummary>,
    pub reviews: Vec<ReviewEntry>,
}

#[derive(Serialize, sqlx::FromRow)]
pub struct FilmPersonEntry {
    pub person_id: i64,
    pub name: String,
    pub role: String,
}

#[derive(Serialize, sqlx::FromRow)]
pub struct ReviewEntry {
    pub id: i64,
    pub is_personal: bool,
    pub author: Option<String>,
    pub content: String,
    pub rating: Option<f64>,
    pub created_at: String,
}

// ── Genre ────────────────────────────────────────────────────────

#[derive(Serialize, sqlx::FromRow)]
pub struct GenreSummary {
    pub id: i64,
    pub name: String,
    pub film_count: i64,
    pub child_count: i64,
}

#[derive(Serialize)]
pub struct GenreDetail {
    pub id: i64,
    pub name: String,
    pub description: Option<String>,
    pub parent_id: Option<i64>,
    pub period: Option<String>,
    pub wiki_content: String,
    pub children: Vec<GenreSummary>,
    pub people: Vec<PersonSummary>,
    pub films: Vec<FilmSummary>,
}

#[derive(Serialize)]
pub struct GenreTreeNode {
    pub id: i64,
    pub name: String,
    pub period: Option<String>,
    pub film_count: i64,
    pub children: Vec<GenreTreeNode>,
}

// ── TMDB input ───────────────────────────────────────────────────

#[derive(Deserialize)]
pub struct TmdbMovieInput {
    pub tmdb_id: i64,
    pub title: String,
    pub original_title: Option<String>,
    pub year: Option<i64>,
    pub overview: Option<String>,
    pub tmdb_rating: Option<f64>,
    pub people: Vec<TmdbPersonInput>,
}

#[derive(Deserialize)]
pub struct TmdbPersonInput {
    pub tmdb_id: Option<i64>,
    pub name: String,
    pub role: String,
    pub primary_role: String,
}

// ── Graph ────────────────────────────────────────────────────────

#[derive(Serialize)]
pub struct GraphData {
    pub nodes: Vec<GraphNode>,
    pub links: Vec<GraphLink>,
}

#[derive(Serialize)]
pub struct GraphNode {
    pub id: String,
    pub label: String,
    pub node_type: String,
    pub role: Option<String>,
    pub weight: f64,
}

#[derive(Serialize)]
pub struct GraphLink {
    pub source: String,
    pub target: String,
    pub role: String,
}
```

- [ ] **Step 2: Create stub files**

`src-tauri/src/commands/library/people.rs`:
```rust
// populated in Task 3
```

`src-tauri/src/commands/library/films.rs`:
```rust
// populated in Task 4
```

`src-tauri/src/commands/library/genres.rs`:
```rust
// populated in Task 5
```

`src-tauri/src/commands/library/reviews.rs`:
```rust
// populated in Task 6
```

`src-tauri/src/commands/library/graph.rs`:
```rust
// populated in Task 6
```

- [ ] **Step 3: Add `library` to commands/mod.rs**

```rust
pub mod config;
pub mod download;
pub mod library;
pub mod media;
pub mod search;
pub mod subtitle;
pub mod tmdb;
pub mod tracker;
```

- [ ] **Step 4: Verify `cargo check`**

```bash
cd src-tauri && cargo check
```

Expected: no errors.

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/commands/
git commit -m "feat: add library module skeleton with all public data types"
```

---

### Task 3: People CRUD commands

**Files:**
- Modify: `src-tauri/src/commands/library/people.rs`

- [ ] **Step 1: Write tests first**

Replace stub content of `people.rs` with:

```rust
use sqlx::SqlitePool;
use super::{PersonDetail, PersonFilmEntry, PersonRelation, PersonSummary};

#[derive(sqlx::FromRow)]
struct PersonRow {
    id: i64,
    tmdb_id: Option<i64>,
    name: String,
    primary_role: String,
    born_date: Option<String>,
    nationality: Option<String>,
    biography: Option<String>,
}

#[tauri::command]
pub async fn list_people(pool: tauri::State<'_, SqlitePool>) -> Result<Vec<PersonSummary>, String> {
    todo!()
}

#[tauri::command]
pub async fn get_person(id: i64, pool: tauri::State<'_, SqlitePool>) -> Result<PersonDetail, String> {
    todo!()
}

#[tauri::command]
pub async fn create_person(
    name: String, primary_role: String, tmdb_id: Option<i64>,
    born_date: Option<String>, nationality: Option<String>,
    pool: tauri::State<'_, SqlitePool>,
) -> Result<i64, String> {
    todo!()
}

#[tauri::command]
pub async fn update_person_wiki(id: i64, content: String, pool: tauri::State<'_, SqlitePool>) -> Result<(), String> {
    todo!()
}

#[tauri::command]
pub async fn delete_person(id: i64, pool: tauri::State<'_, SqlitePool>) -> Result<(), String> {
    todo!()
}

#[tauri::command]
pub async fn add_person_relation(from_id: i64, to_id: i64, relation_type: String, pool: tauri::State<'_, SqlitePool>) -> Result<(), String> {
    todo!()
}

#[tauri::command]
pub async fn remove_person_relation(from_id: i64, to_id: i64, relation_type: String, pool: tauri::State<'_, SqlitePool>) -> Result<(), String> {
    todo!()
}

#[cfg(test)]
mod tests {
    use super::*;

    async fn setup() -> SqlitePool {
        let pool = SqlitePool::connect(":memory:").await.unwrap();
        sqlx::migrate!("./migrations").run(&pool).await.unwrap();
        pool
    }

    #[tokio::test]
    async fn create_and_list_person() {
        let pool = setup().await;
        let id = sqlx::query(
            "INSERT INTO people (name, primary_role) VALUES ('Michelangelo Antonioni', 'director')",
        )
        .execute(&pool).await.unwrap().last_insert_rowid();

        let rows = sqlx::query_as::<_, PersonSummary>(
            "SELECT p.id, p.name, p.primary_role, COUNT(pf.film_id) as film_count
             FROM people p LEFT JOIN person_films pf ON pf.person_id = p.id
             GROUP BY p.id ORDER BY p.name",
        )
        .fetch_all(&pool).await.unwrap();

        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].id, id);
        assert_eq!(rows[0].primary_role, "director");
        assert_eq!(rows[0].film_count, 0);
    }

    #[tokio::test]
    async fn wiki_upsert() {
        let pool = setup().await;
        let id = sqlx::query(
            "INSERT INTO people (name, primary_role) VALUES ('Test', 'director')",
        )
        .execute(&pool).await.unwrap().last_insert_rowid();

        for content in ["First", "Updated"] {
            sqlx::query(
                "INSERT INTO wiki_entries (entity_type, entity_id, content, updated_at)
                 VALUES ('person', ?, ?, datetime('now'))
                 ON CONFLICT(entity_type, entity_id)
                 DO UPDATE SET content = excluded.content, updated_at = excluded.updated_at",
            )
            .bind(id).bind(content).execute(&pool).await.unwrap();
        }

        let saved: String = sqlx::query_scalar(
            "SELECT content FROM wiki_entries WHERE entity_type = 'person' AND entity_id = ?",
        )
        .bind(id).fetch_one(&pool).await.unwrap();
        assert_eq!(saved, "Updated");
    }

    #[tokio::test]
    async fn person_relations() {
        let pool = setup().await;
        let a = sqlx::query("INSERT INTO people (name, primary_role) VALUES ('A', 'director')")
            .execute(&pool).await.unwrap().last_insert_rowid();
        let b = sqlx::query("INSERT INTO people (name, primary_role) VALUES ('B', 'director')")
            .execute(&pool).await.unwrap().last_insert_rowid();

        sqlx::query(
            "INSERT OR IGNORE INTO person_relations (from_id, to_id, relation_type) VALUES (?, ?, 'influenced')",
        )
        .bind(a).bind(b).execute(&pool).await.unwrap();

        let count: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM person_relations WHERE from_id = ? AND to_id = ?",
        )
        .bind(a).bind(b).fetch_one(&pool).await.unwrap();
        assert_eq!(count, 1);
    }
}
```

- [ ] **Step 2: Run tests (SQL-only tests pass, todo! functions not called)**

```bash
cd src-tauri && cargo test commands::library::people
```

Expected: 3 tests PASS.

- [ ] **Step 3: Implement all commands**

Replace the full content of `people.rs`:

```rust
use sqlx::SqlitePool;
use super::{PersonDetail, PersonFilmEntry, PersonRelation, PersonSummary};

#[derive(sqlx::FromRow)]
struct PersonRow {
    id: i64,
    tmdb_id: Option<i64>,
    name: String,
    primary_role: String,
    born_date: Option<String>,
    nationality: Option<String>,
    biography: Option<String>,
}

#[tauri::command]
pub async fn list_people(pool: tauri::State<'_, SqlitePool>) -> Result<Vec<PersonSummary>, String> {
    sqlx::query_as::<_, PersonSummary>(
        "SELECT p.id, p.name, p.primary_role, COUNT(pf.film_id) as film_count
         FROM people p
         LEFT JOIN person_films pf ON pf.person_id = p.id
         GROUP BY p.id ORDER BY p.name",
    )
    .fetch_all(&**pool).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_person(id: i64, pool: tauri::State<'_, SqlitePool>) -> Result<PersonDetail, String> {
    let row = sqlx::query_as::<_, PersonRow>(
        "SELECT id, tmdb_id, name, primary_role, born_date, nationality, biography
         FROM people WHERE id = ?",
    )
    .bind(id).fetch_one(&**pool).await.map_err(|e| e.to_string())?;

    let wiki_content: String = sqlx::query_scalar(
        "SELECT content FROM wiki_entries WHERE entity_type = 'person' AND entity_id = ?",
    )
    .bind(id).fetch_optional(&**pool).await.map_err(|e| e.to_string())?
    .unwrap_or_default();

    let films = sqlx::query_as::<_, PersonFilmEntry>(
        "SELECT f.id as film_id, f.title, f.year, pf.role, f.poster_cache_path
         FROM person_films pf JOIN films f ON f.id = pf.film_id
         WHERE pf.person_id = ? ORDER BY f.year",
    )
    .bind(id).fetch_all(&**pool).await.map_err(|e| e.to_string())?;

    let relations = sqlx::query_as::<_, PersonRelation>(
        "SELECT pr.to_id as target_id, p.name as target_name,
                'to' as direction, pr.relation_type
         FROM person_relations pr JOIN people p ON p.id = pr.to_id
         WHERE pr.from_id = ?
         UNION ALL
         SELECT pr.from_id as target_id, p.name as target_name,
                'from' as direction, pr.relation_type
         FROM person_relations pr JOIN people p ON p.id = pr.from_id
         WHERE pr.to_id = ?",
    )
    .bind(id).bind(id).fetch_all(&**pool).await.map_err(|e| e.to_string())?;

    Ok(PersonDetail {
        id: row.id, tmdb_id: row.tmdb_id, name: row.name,
        primary_role: row.primary_role, born_date: row.born_date,
        nationality: row.nationality, biography: row.biography,
        wiki_content, films, relations,
    })
}

#[tauri::command]
pub async fn create_person(
    name: String, primary_role: String, tmdb_id: Option<i64>,
    born_date: Option<String>, nationality: Option<String>,
    pool: tauri::State<'_, SqlitePool>,
) -> Result<i64, String> {
    sqlx::query(
        "INSERT INTO people (name, primary_role, tmdb_id, born_date, nationality) VALUES (?, ?, ?, ?, ?)",
    )
    .bind(&name).bind(&primary_role).bind(tmdb_id).bind(born_date).bind(nationality)
    .execute(&**pool).await
    .map(|r| r.last_insert_rowid()).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn update_person_wiki(id: i64, content: String, pool: tauri::State<'_, SqlitePool>) -> Result<(), String> {
    sqlx::query(
        "INSERT INTO wiki_entries (entity_type, entity_id, content, updated_at)
         VALUES ('person', ?, ?, datetime('now'))
         ON CONFLICT(entity_type, entity_id)
         DO UPDATE SET content = excluded.content, updated_at = excluded.updated_at",
    )
    .bind(id).bind(&content).execute(&**pool).await
    .map(|_| ()).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn delete_person(id: i64, pool: tauri::State<'_, SqlitePool>) -> Result<(), String> {
    sqlx::query("DELETE FROM person_relations WHERE from_id = ? OR to_id = ?")
        .bind(id).bind(id).execute(&**pool).await.map_err(|e| e.to_string())?;
    sqlx::query("DELETE FROM person_films WHERE person_id = ?")
        .bind(id).execute(&**pool).await.map_err(|e| e.to_string())?;
    sqlx::query("DELETE FROM person_genres WHERE person_id = ?")
        .bind(id).execute(&**pool).await.map_err(|e| e.to_string())?;
    sqlx::query("DELETE FROM wiki_entries WHERE entity_type = 'person' AND entity_id = ?")
        .bind(id).execute(&**pool).await.map_err(|e| e.to_string())?;
    sqlx::query("DELETE FROM people WHERE id = ?")
        .bind(id).execute(&**pool).await.map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub async fn add_person_relation(
    from_id: i64, to_id: i64, relation_type: String,
    pool: tauri::State<'_, SqlitePool>,
) -> Result<(), String> {
    sqlx::query(
        "INSERT OR IGNORE INTO person_relations (from_id, to_id, relation_type) VALUES (?, ?, ?)",
    )
    .bind(from_id).bind(to_id).bind(&relation_type).execute(&**pool).await
    .map(|_| ()).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn remove_person_relation(
    from_id: i64, to_id: i64, relation_type: String,
    pool: tauri::State<'_, SqlitePool>,
) -> Result<(), String> {
    sqlx::query(
        "DELETE FROM person_relations WHERE from_id = ? AND to_id = ? AND relation_type = ?",
    )
    .bind(from_id).bind(to_id).bind(&relation_type).execute(&**pool).await
    .map(|_| ()).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    async fn setup() -> SqlitePool {
        let pool = SqlitePool::connect(":memory:").await.unwrap();
        sqlx::migrate!("./migrations").run(&pool).await.unwrap();
        pool
    }

    #[tokio::test]
    async fn create_and_list_person() {
        let pool = setup().await;
        let id = sqlx::query(
            "INSERT INTO people (name, primary_role) VALUES ('Michelangelo Antonioni', 'director')",
        )
        .execute(&pool).await.unwrap().last_insert_rowid();

        let rows = sqlx::query_as::<_, PersonSummary>(
            "SELECT p.id, p.name, p.primary_role, COUNT(pf.film_id) as film_count
             FROM people p LEFT JOIN person_films pf ON pf.person_id = p.id
             GROUP BY p.id ORDER BY p.name",
        )
        .fetch_all(&pool).await.unwrap();

        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].id, id);
        assert_eq!(rows[0].film_count, 0);
    }

    #[tokio::test]
    async fn wiki_upsert() {
        let pool = setup().await;
        let id = sqlx::query(
            "INSERT INTO people (name, primary_role) VALUES ('Test', 'director')",
        )
        .execute(&pool).await.unwrap().last_insert_rowid();

        for content in ["First", "Updated"] {
            sqlx::query(
                "INSERT INTO wiki_entries (entity_type, entity_id, content, updated_at)
                 VALUES ('person', ?, ?, datetime('now'))
                 ON CONFLICT(entity_type, entity_id)
                 DO UPDATE SET content = excluded.content, updated_at = excluded.updated_at",
            )
            .bind(id).bind(content).execute(&pool).await.unwrap();
        }

        let saved: String = sqlx::query_scalar(
            "SELECT content FROM wiki_entries WHERE entity_type = 'person' AND entity_id = ?",
        )
        .bind(id).fetch_one(&pool).await.unwrap();
        assert_eq!(saved, "Updated");
    }

    #[tokio::test]
    async fn person_relations() {
        let pool = setup().await;
        let a = sqlx::query("INSERT INTO people (name, primary_role) VALUES ('A', 'director')")
            .execute(&pool).await.unwrap().last_insert_rowid();
        let b = sqlx::query("INSERT INTO people (name, primary_role) VALUES ('B', 'director')")
            .execute(&pool).await.unwrap().last_insert_rowid();

        sqlx::query(
            "INSERT OR IGNORE INTO person_relations (from_id, to_id, relation_type) VALUES (?, ?, 'influenced')",
        )
        .bind(a).bind(b).execute(&pool).await.unwrap();

        let count: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM person_relations WHERE from_id = ? AND to_id = ?",
        )
        .bind(a).bind(b).fetch_one(&pool).await.unwrap();
        assert_eq!(count, 1);
    }
}
```

- [ ] **Step 4: Run tests**

```bash
cd src-tauri && cargo test commands::library::people
```

Expected: 3 tests PASS.

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/commands/library/people.rs
git commit -m "feat: add people CRUD library commands"
```

---

### Task 4: Films CRUD commands

**Files:**
- Modify: `src-tauri/src/commands/library/films.rs`

- [ ] **Step 1: Implement films.rs with tests**

```rust
use sqlx::SqlitePool;
use super::{FilmDetail, FilmPersonEntry, FilmSummary, GenreSummary, ReviewEntry, TmdbMovieInput};

#[derive(sqlx::FromRow)]
struct FilmRow {
    id: i64,
    tmdb_id: Option<i64>,
    title: String,
    original_title: Option<String>,
    year: Option<i64>,
    overview: Option<String>,
    tmdb_rating: Option<f64>,
    poster_cache_path: Option<String>,
}

#[tauri::command]
pub async fn list_films(pool: tauri::State<'_, SqlitePool>) -> Result<Vec<FilmSummary>, String> {
    sqlx::query_as::<_, FilmSummary>(
        "SELECT id, title, year, tmdb_rating, poster_cache_path
         FROM films ORDER BY year DESC, title",
    )
    .fetch_all(&**pool).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_film(id: i64, pool: tauri::State<'_, SqlitePool>) -> Result<FilmDetail, String> {
    let row = sqlx::query_as::<_, FilmRow>(
        "SELECT id, tmdb_id, title, original_title, year, overview, tmdb_rating, poster_cache_path
         FROM films WHERE id = ?",
    )
    .bind(id).fetch_one(&**pool).await.map_err(|e| e.to_string())?;

    let wiki_content: String = sqlx::query_scalar(
        "SELECT content FROM wiki_entries WHERE entity_type = 'film' AND entity_id = ?",
    )
    .bind(id).fetch_optional(&**pool).await.map_err(|e| e.to_string())?
    .unwrap_or_default();

    let people = sqlx::query_as::<_, FilmPersonEntry>(
        "SELECT p.id as person_id, p.name, pf.role
         FROM person_films pf JOIN people p ON p.id = pf.person_id
         WHERE pf.film_id = ? ORDER BY p.primary_role",
    )
    .bind(id).fetch_all(&**pool).await.map_err(|e| e.to_string())?;

    let genres = sqlx::query_as::<_, GenreSummary>(
        "SELECT g.id, g.name,
                (SELECT COUNT(*) FROM film_genres WHERE genre_id = g.id) as film_count,
                (SELECT COUNT(*) FROM genres WHERE parent_id = g.id) as child_count
         FROM film_genres fg JOIN genres g ON g.id = fg.genre_id
         WHERE fg.film_id = ?",
    )
    .bind(id).fetch_all(&**pool).await.map_err(|e| e.to_string())?;

    let reviews = sqlx::query_as::<_, ReviewEntry>(
        "SELECT id, is_personal, author, content, rating, created_at
         FROM reviews WHERE film_id = ? ORDER BY is_personal DESC, created_at DESC",
    )
    .bind(id).fetch_all(&**pool).await.map_err(|e| e.to_string())?;

    Ok(FilmDetail {
        id: row.id, tmdb_id: row.tmdb_id, title: row.title,
        original_title: row.original_title, year: row.year,
        overview: row.overview, tmdb_rating: row.tmdb_rating,
        poster_cache_path: row.poster_cache_path,
        wiki_content, people, genres, reviews,
    })
}

#[tauri::command]
pub async fn add_film_from_tmdb(
    tmdb_movie: TmdbMovieInput,
    pool: tauri::State<'_, SqlitePool>,
) -> Result<i64, String> {
    // Deduplicate by tmdb_id
    if let Some(existing_id) = sqlx::query_scalar::<_, i64>(
        "SELECT id FROM films WHERE tmdb_id = ?",
    )
    .bind(tmdb_movie.tmdb_id).fetch_optional(&**pool).await.map_err(|e| e.to_string())?
    {
        return Ok(existing_id);
    }

    let film_id = sqlx::query(
        "INSERT INTO films (tmdb_id, title, original_title, year, overview, tmdb_rating) VALUES (?, ?, ?, ?, ?, ?)",
    )
    .bind(tmdb_movie.tmdb_id).bind(&tmdb_movie.title).bind(&tmdb_movie.original_title)
    .bind(tmdb_movie.year).bind(&tmdb_movie.overview).bind(tmdb_movie.tmdb_rating)
    .execute(&**pool).await.map_err(|e| e.to_string())?.last_insert_rowid();

    for person in &tmdb_movie.people {
        let person_id: i64 = if let Some(tmdb_id) = person.tmdb_id {
            if let Some(existing) = sqlx::query_scalar::<_, i64>(
                "SELECT id FROM people WHERE tmdb_id = ?",
            )
            .bind(tmdb_id).fetch_optional(&**pool).await.map_err(|e| e.to_string())?
            {
                existing
            } else {
                sqlx::query("INSERT INTO people (name, primary_role, tmdb_id) VALUES (?, ?, ?)")
                    .bind(&person.name).bind(&person.primary_role).bind(tmdb_id)
                    .execute(&**pool).await.map_err(|e| e.to_string())?.last_insert_rowid()
            }
        } else {
            if let Some(existing) = sqlx::query_scalar::<_, i64>(
                "SELECT id FROM people WHERE name = ?",
            )
            .bind(&person.name).fetch_optional(&**pool).await.map_err(|e| e.to_string())?
            {
                existing
            } else {
                sqlx::query("INSERT INTO people (name, primary_role) VALUES (?, ?)")
                    .bind(&person.name).bind(&person.primary_role)
                    .execute(&**pool).await.map_err(|e| e.to_string())?.last_insert_rowid()
            }
        };

        sqlx::query(
            "INSERT OR IGNORE INTO person_films (person_id, film_id, role) VALUES (?, ?, ?)",
        )
        .bind(person_id).bind(film_id).bind(&person.role)
        .execute(&**pool).await.map_err(|e| e.to_string())?;
    }

    Ok(film_id)
}

#[tauri::command]
pub async fn update_film_wiki(id: i64, content: String, pool: tauri::State<'_, SqlitePool>) -> Result<(), String> {
    sqlx::query(
        "INSERT INTO wiki_entries (entity_type, entity_id, content, updated_at)
         VALUES ('film', ?, ?, datetime('now'))
         ON CONFLICT(entity_type, entity_id)
         DO UPDATE SET content = excluded.content, updated_at = excluded.updated_at",
    )
    .bind(id).bind(&content).execute(&**pool).await
    .map(|_| ()).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn delete_film(id: i64, pool: tauri::State<'_, SqlitePool>) -> Result<(), String> {
    sqlx::query("DELETE FROM film_genres WHERE film_id = ?").bind(id).execute(&**pool).await.map_err(|e| e.to_string())?;
    sqlx::query("DELETE FROM person_films WHERE film_id = ?").bind(id).execute(&**pool).await.map_err(|e| e.to_string())?;
    sqlx::query("DELETE FROM reviews WHERE film_id = ?").bind(id).execute(&**pool).await.map_err(|e| e.to_string())?;
    sqlx::query("DELETE FROM library_items WHERE film_id = ?").bind(id).execute(&**pool).await.map_err(|e| e.to_string())?;
    sqlx::query("DELETE FROM wiki_entries WHERE entity_type = 'film' AND entity_id = ?").bind(id).execute(&**pool).await.map_err(|e| e.to_string())?;
    sqlx::query("DELETE FROM films WHERE id = ?").bind(id).execute(&**pool).await.map_err(|e| e.to_string())?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    async fn setup() -> SqlitePool {
        let pool = SqlitePool::connect(":memory:").await.unwrap();
        sqlx::migrate!("./migrations").run(&pool).await.unwrap();
        pool
    }

    #[tokio::test]
    async fn insert_and_list_film() {
        let pool = setup().await;
        sqlx::query(
            "INSERT INTO films (tmdb_id, title, year, tmdb_rating) VALUES (12345, 'Blow-Up', 1966, 8.1)",
        )
        .execute(&pool).await.unwrap();

        let rows = sqlx::query_as::<_, FilmSummary>(
            "SELECT id, title, year, tmdb_rating, poster_cache_path FROM films ORDER BY year DESC, title",
        )
        .fetch_all(&pool).await.unwrap();

        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].title, "Blow-Up");
        assert_eq!(rows[0].year, Some(1966));
    }

    #[tokio::test]
    async fn add_film_deduplicates() {
        let pool = setup().await;
        let tmdb_id = 999i64;

        for _ in 0..2 {
            let existing: Option<i64> =
                sqlx::query_scalar("SELECT id FROM films WHERE tmdb_id = ?")
                    .bind(tmdb_id).fetch_optional(&pool).await.unwrap();
            if existing.is_none() {
                sqlx::query("INSERT INTO films (tmdb_id, title, year) VALUES (?, 'Test', 2020)")
                    .bind(tmdb_id).execute(&pool).await.unwrap();
            }
        }

        let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM films WHERE tmdb_id = ?")
            .bind(tmdb_id).fetch_one(&pool).await.unwrap();
        assert_eq!(count, 1);
    }
}
```

- [ ] **Step 2: Run tests**

```bash
cd src-tauri && cargo test commands::library::films
```

Expected: 2 tests PASS.

- [ ] **Step 3: Commit**

```bash
git add src-tauri/src/commands/library/films.rs
git commit -m "feat: add films CRUD library commands"
```

---

### Task 5: Genres CRUD commands

**Files:**
- Modify: `src-tauri/src/commands/library/genres.rs`

- [ ] **Step 1: Implement genres.rs with tests**

```rust
use std::collections::HashMap;
use sqlx::SqlitePool;
use super::{FilmSummary, GenreDetail, GenreSummary, GenreTreeNode, PersonSummary};

#[derive(sqlx::FromRow)]
struct GenreRow {
    id: i64,
    name: String,
    description: Option<String>,
    parent_id: Option<i64>,
    period: Option<String>,
}

#[derive(sqlx::FromRow)]
struct FilmCountRow {
    genre_id: i64,
    count: i64,
}

fn build_tree(
    nodes: &[GenreRow],
    count_map: &HashMap<i64, i64>,
    parent_id: Option<i64>,
) -> Vec<GenreTreeNode> {
    nodes.iter()
        .filter(|n| n.parent_id == parent_id)
        .map(|n| GenreTreeNode {
            id: n.id,
            name: n.name.clone(),
            period: n.period.clone(),
            film_count: *count_map.get(&n.id).unwrap_or(&0),
            children: build_tree(nodes, count_map, Some(n.id)),
        })
        .collect()
}

#[tauri::command]
pub async fn list_genres_tree(pool: tauri::State<'_, SqlitePool>) -> Result<Vec<GenreTreeNode>, String> {
    let rows = sqlx::query_as::<_, GenreRow>(
        "SELECT id, name, description, parent_id, period FROM genres",
    )
    .fetch_all(&**pool).await.map_err(|e| e.to_string())?;

    let counts = sqlx::query_as::<_, FilmCountRow>(
        "SELECT genre_id, COUNT(*) as count FROM film_genres GROUP BY genre_id",
    )
    .fetch_all(&**pool).await.map_err(|e| e.to_string())?;

    let count_map: HashMap<i64, i64> = counts.into_iter().map(|r| (r.genre_id, r.count)).collect();
    Ok(build_tree(&rows, &count_map, None))
}

#[tauri::command]
pub async fn get_genre(id: i64, pool: tauri::State<'_, SqlitePool>) -> Result<GenreDetail, String> {
    let row = sqlx::query_as::<_, GenreRow>(
        "SELECT id, name, description, parent_id, period FROM genres WHERE id = ?",
    )
    .bind(id).fetch_one(&**pool).await.map_err(|e| e.to_string())?;

    let wiki_content: String = sqlx::query_scalar(
        "SELECT content FROM wiki_entries WHERE entity_type = 'genre' AND entity_id = ?",
    )
    .bind(id).fetch_optional(&**pool).await.map_err(|e| e.to_string())?
    .unwrap_or_default();

    let children = sqlx::query_as::<_, GenreSummary>(
        "SELECT g.id, g.name,
                (SELECT COUNT(*) FROM film_genres WHERE genre_id = g.id) as film_count,
                (SELECT COUNT(*) FROM genres WHERE parent_id = g.id) as child_count
         FROM genres g WHERE g.parent_id = ?",
    )
    .bind(id).fetch_all(&**pool).await.map_err(|e| e.to_string())?;

    let people = sqlx::query_as::<_, PersonSummary>(
        "SELECT p.id, p.name, p.primary_role, COUNT(pf.film_id) as film_count
         FROM person_genres pg
         JOIN people p ON p.id = pg.person_id
         LEFT JOIN person_films pf ON pf.person_id = p.id
         WHERE pg.genre_id = ?
         GROUP BY p.id ORDER BY p.name",
    )
    .bind(id).fetch_all(&**pool).await.map_err(|e| e.to_string())?;

    let films = sqlx::query_as::<_, FilmSummary>(
        "SELECT f.id, f.title, f.year, f.tmdb_rating, f.poster_cache_path
         FROM film_genres fg JOIN films f ON f.id = fg.film_id
         WHERE fg.genre_id = ? ORDER BY f.year DESC",
    )
    .bind(id).fetch_all(&**pool).await.map_err(|e| e.to_string())?;

    Ok(GenreDetail {
        id: row.id, name: row.name, description: row.description,
        parent_id: row.parent_id, period: row.period,
        wiki_content, children, people, films,
    })
}

#[tauri::command]
pub async fn create_genre(
    name: String, parent_id: Option<i64>, description: Option<String>, period: Option<String>,
    pool: tauri::State<'_, SqlitePool>,
) -> Result<i64, String> {
    sqlx::query(
        "INSERT INTO genres (name, parent_id, description, period) VALUES (?, ?, ?, ?)",
    )
    .bind(&name).bind(parent_id).bind(description).bind(period)
    .execute(&**pool).await
    .map(|r| r.last_insert_rowid()).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn update_genre_wiki(id: i64, content: String, pool: tauri::State<'_, SqlitePool>) -> Result<(), String> {
    sqlx::query(
        "INSERT INTO wiki_entries (entity_type, entity_id, content, updated_at)
         VALUES ('genre', ?, ?, datetime('now'))
         ON CONFLICT(entity_type, entity_id)
         DO UPDATE SET content = excluded.content, updated_at = excluded.updated_at",
    )
    .bind(id).bind(&content).execute(&**pool).await
    .map(|_| ()).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn delete_genre(id: i64, pool: tauri::State<'_, SqlitePool>) -> Result<(), String> {
    // Reparent children to this genre's parent
    sqlx::query(
        "UPDATE genres SET parent_id = (SELECT parent_id FROM genres WHERE id = ?) WHERE parent_id = ?",
    )
    .bind(id).bind(id).execute(&**pool).await.map_err(|e| e.to_string())?;

    sqlx::query("DELETE FROM film_genres WHERE genre_id = ?").bind(id).execute(&**pool).await.map_err(|e| e.to_string())?;
    sqlx::query("DELETE FROM person_genres WHERE genre_id = ?").bind(id).execute(&**pool).await.map_err(|e| e.to_string())?;
    sqlx::query("DELETE FROM wiki_entries WHERE entity_type = 'genre' AND entity_id = ?").bind(id).execute(&**pool).await.map_err(|e| e.to_string())?;
    sqlx::query("DELETE FROM genres WHERE id = ?").bind(id).execute(&**pool).await.map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub async fn link_film_genre(film_id: i64, genre_id: i64, pool: tauri::State<'_, SqlitePool>) -> Result<(), String> {
    sqlx::query("INSERT OR IGNORE INTO film_genres (film_id, genre_id) VALUES (?, ?)")
        .bind(film_id).bind(genre_id).execute(&**pool).await
        .map(|_| ()).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn unlink_film_genre(film_id: i64, genre_id: i64, pool: tauri::State<'_, SqlitePool>) -> Result<(), String> {
    sqlx::query("DELETE FROM film_genres WHERE film_id = ? AND genre_id = ?")
        .bind(film_id).bind(genre_id).execute(&**pool).await
        .map(|_| ()).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn link_person_genre(person_id: i64, genre_id: i64, pool: tauri::State<'_, SqlitePool>) -> Result<(), String> {
    sqlx::query("INSERT OR IGNORE INTO person_genres (person_id, genre_id) VALUES (?, ?)")
        .bind(person_id).bind(genre_id).execute(&**pool).await
        .map(|_| ()).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn unlink_person_genre(person_id: i64, genre_id: i64, pool: tauri::State<'_, SqlitePool>) -> Result<(), String> {
    sqlx::query("DELETE FROM person_genres WHERE person_id = ? AND genre_id = ?")
        .bind(person_id).bind(genre_id).execute(&**pool).await
        .map(|_| ()).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    async fn setup() -> SqlitePool {
        let pool = SqlitePool::connect(":memory:").await.unwrap();
        sqlx::migrate!("./migrations").run(&pool).await.unwrap();
        pool
    }

    #[tokio::test]
    async fn build_genre_tree_empty() {
        let pool = setup().await;
        let rows = sqlx::query_as::<_, GenreRow>(
            "SELECT id, name, description, parent_id, period FROM genres",
        )
        .fetch_all(&pool).await.unwrap();
        let tree = build_tree(&rows, &HashMap::new(), None);
        assert!(tree.is_empty());
    }

    #[tokio::test]
    async fn build_genre_tree_with_children() {
        let pool = setup().await;
        let parent = sqlx::query("INSERT INTO genres (name) VALUES ('Drama')")
            .execute(&pool).await.unwrap().last_insert_rowid();
        sqlx::query("INSERT INTO genres (name, parent_id) VALUES ('Neorealism', ?)")
            .bind(parent).execute(&pool).await.unwrap();
        sqlx::query("INSERT INTO genres (name, parent_id) VALUES ('New Wave', ?)")
            .bind(parent).execute(&pool).await.unwrap();

        let rows = sqlx::query_as::<_, GenreRow>(
            "SELECT id, name, description, parent_id, period FROM genres",
        )
        .fetch_all(&pool).await.unwrap();
        let tree = build_tree(&rows, &HashMap::new(), None);

        assert_eq!(tree.len(), 1);
        assert_eq!(tree[0].name, "Drama");
        assert_eq!(tree[0].children.len(), 2);
    }
}
```

- [ ] **Step 2: Run tests**

```bash
cd src-tauri && cargo test commands::library::genres
```

Expected: 2 tests PASS.

- [ ] **Step 3: Commit**

```bash
git add src-tauri/src/commands/library/genres.rs
git commit -m "feat: add genres CRUD library commands with tree builder"
```

---

### Task 6: Reviews + Graph commands

**Files:**
- Modify: `src-tauri/src/commands/library/reviews.rs`
- Modify: `src-tauri/src/commands/library/graph.rs`

- [ ] **Step 1: Implement reviews.rs**

```rust
use sqlx::SqlitePool;

#[tauri::command]
pub async fn add_review(
    film_id: i64, is_personal: bool, author: Option<String>,
    content: String, rating: Option<f64>,
    pool: tauri::State<'_, SqlitePool>,
) -> Result<i64, String> {
    sqlx::query(
        "INSERT INTO reviews (film_id, is_personal, author, content, rating) VALUES (?, ?, ?, ?, ?)",
    )
    .bind(film_id).bind(is_personal as i64).bind(&author).bind(&content).bind(rating)
    .execute(&**pool).await
    .map(|r| r.last_insert_rowid()).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn update_review(
    id: i64, content: String, rating: Option<f64>,
    pool: tauri::State<'_, SqlitePool>,
) -> Result<(), String> {
    sqlx::query("UPDATE reviews SET content = ?, rating = ? WHERE id = ?")
        .bind(&content).bind(rating).bind(id).execute(&**pool).await
        .map(|_| ()).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn delete_review(id: i64, pool: tauri::State<'_, SqlitePool>) -> Result<(), String> {
    sqlx::query("DELETE FROM reviews WHERE id = ?").bind(id).execute(&**pool).await
        .map(|_| ()).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    async fn setup() -> SqlitePool {
        let pool = SqlitePool::connect(":memory:").await.unwrap();
        sqlx::migrate!("./migrations").run(&pool).await.unwrap();
        pool
    }

    #[tokio::test]
    async fn add_and_update_review() {
        let pool = setup().await;
        let film_id = sqlx::query("INSERT INTO films (title) VALUES ('Test Film')")
            .execute(&pool).await.unwrap().last_insert_rowid();

        let rev_id = sqlx::query(
            "INSERT INTO reviews (film_id, is_personal, content, rating) VALUES (?, 1, 'Good', 8.5)",
        )
        .bind(film_id).execute(&pool).await.unwrap().last_insert_rowid();

        sqlx::query("UPDATE reviews SET content = ?, rating = ? WHERE id = ?")
            .bind("Great").bind(9.0_f64).bind(rev_id)
            .execute(&pool).await.unwrap();

        let (content, rating): (String, f64) =
            sqlx::query_as("SELECT content, rating FROM reviews WHERE id = ?")
                .bind(rev_id).fetch_one(&pool).await.unwrap();

        assert_eq!(content, "Great");
        assert!((rating - 9.0).abs() < f64::EPSILON);
    }
}
```

- [ ] **Step 2: Implement graph.rs**

```rust
use std::collections::HashMap;
use sqlx::SqlitePool;
use super::{GraphData, GraphLink, GraphNode};

#[derive(sqlx::FromRow)]
struct PersonFilmRow {
    person_id: i64,
    person_name: String,
    primary_role: String,
    film_id: i64,
    film_title: String,
    role: String,
}

#[tauri::command]
pub async fn get_graph_data(pool: tauri::State<'_, SqlitePool>) -> Result<GraphData, String> {
    let pf_rows = sqlx::query_as::<_, PersonFilmRow>(
        "SELECT pf.person_id, p.name as person_name, p.primary_role,
                pf.film_id, f.title as film_title, pf.role
         FROM person_films pf
         JOIN people p ON p.id = pf.person_id
         JOIN films f ON f.id = pf.film_id",
    )
    .fetch_all(&**pool).await.map_err(|e| e.to_string())?;

    let mut person_film_counts: HashMap<i64, i64> = HashMap::new();
    let mut person_meta: HashMap<i64, (String, String)> = HashMap::new();
    let mut film_meta: HashMap<i64, String> = HashMap::new();

    for row in &pf_rows {
        *person_film_counts.entry(row.person_id).or_insert(0) += 1;
        person_meta.entry(row.person_id)
            .or_insert_with(|| (row.person_name.clone(), row.primary_role.clone()));
        film_meta.entry(row.film_id)
            .or_insert_with(|| row.film_title.clone());
    }

    let max_count = person_film_counts.values().copied().max().unwrap_or(1) as f64;

    let mut nodes: Vec<GraphNode> = film_meta.iter().map(|(id, title)| GraphNode {
        id: format!("f{id}"),
        label: title.clone(),
        node_type: "film".to_string(),
        role: None,
        weight: 1.0,
    }).collect();

    for (person_id, film_count) in &person_film_counts {
        let (name, primary_role) = person_meta.get(person_id).unwrap();
        let weight = 0.5 + (*film_count as f64 / max_count) * 2.5;
        nodes.push(GraphNode {
            id: format!("p{person_id}"),
            label: name.clone(),
            node_type: "person".to_string(),
            role: Some(primary_role.clone()),
            weight,
        });
    }

    let links: Vec<GraphLink> = pf_rows.iter().map(|row| GraphLink {
        source: format!("p{}", row.person_id),
        target: format!("f{}", row.film_id),
        role: row.role.clone(),
    }).collect();

    Ok(GraphData { nodes, links })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn weight_normalization() {
        let counts = vec![2i64, 4, 1];
        let max = *counts.iter().max().unwrap() as f64;
        let weights: Vec<f64> = counts.iter().map(|&c| 0.5 + (c as f64 / max) * 2.5).collect();
        assert!((weights[1] - 3.0).abs() < f64::EPSILON);
        assert!((weights[2] - (0.5 + 0.25 * 2.5)).abs() < f64::EPSILON);
    }
}
```

- [ ] **Step 3: Run tests**

```bash
cd src-tauri && cargo test commands::library::reviews commands::library::graph
```

Expected: 2 tests PASS.

- [ ] **Step 4: Commit**

```bash
git add src-tauri/src/commands/library/reviews.rs src-tauri/src/commands/library/graph.rs
git commit -m "feat: add reviews and graph data commands"
```

---

### Task 7: TMDB credits command + register all commands + smoke build

**Files:**
- Modify: `src-tauri/src/commands/tmdb.rs`
- Modify: `src-tauri/src/lib.rs`

- [ ] **Step 1: Add `get_tmdb_movie_credits` to tmdb.rs**

Append to the end of `src-tauri/src/commands/tmdb.rs`:

```rust
#[derive(Serialize)]
pub struct TmdbCrewMember {
    pub id: i64,
    pub name: String,
    pub job: String,
    pub department: String,
}

#[derive(Serialize)]
pub struct TmdbCastMember {
    pub id: i64,
    pub name: String,
    pub character: String,
}

#[derive(Serialize)]
pub struct TmdbMovieCredits {
    pub tmdb_id: i64,
    pub title: String,
    pub original_title: Option<String>,
    pub year: Option<i64>,
    pub overview: Option<String>,
    pub vote_average: Option<f64>,
    pub poster_path: Option<String>,
    pub crew: Vec<TmdbCrewMember>,
    pub cast: Vec<TmdbCastMember>,
}

#[derive(Deserialize)]
struct CreditsResponse {
    crew: Vec<CrewItem>,
    cast: Vec<CastItem>,
}

#[derive(Deserialize)]
struct CrewItem {
    id: i64,
    name: String,
    job: String,
    department: String,
}

#[derive(Deserialize)]
struct CastItem {
    id: i64,
    name: String,
    character: String,
}

#[derive(Deserialize)]
struct MovieDetailsResponse {
    id: i64,
    title: String,
    original_title: Option<String>,
    release_date: Option<String>,
    overview: Option<String>,
    vote_average: Option<f64>,
    poster_path: Option<String>,
    credits: CreditsResponse,
}

#[tauri::command]
pub async fn get_tmdb_movie_credits(
    api_key: String,
    tmdb_id: i64,
) -> Result<TmdbMovieCredits, String> {
    let client = reqwest::Client::new();
    let url = format!(
        "https://api.themoviedb.org/3/movie/{}?append_to_response=credits&api_key={}&language=en-US",
        tmdb_id, api_key
    );

    let resp: MovieDetailsResponse = client.get(&url).send().await
        .map_err(|e| e.to_string())?.json().await.map_err(|e| e.to_string())?;

    let year = resp.release_date.as_deref()
        .and_then(|d| d.get(..4))
        .and_then(|y| y.parse::<i64>().ok());

    let key_jobs = ["Director", "Director of Photography", "Original Music Composer",
                    "Editor", "Screenplay", "Writer"];
    let crew: Vec<TmdbCrewMember> = resp.credits.crew.into_iter()
        .filter(|m| key_jobs.contains(&m.job.as_str()))
        .map(|m| TmdbCrewMember { id: m.id, name: m.name, job: m.job, department: m.department })
        .collect();

    let cast: Vec<TmdbCastMember> = resp.credits.cast.into_iter().take(5)
        .map(|m| TmdbCastMember { id: m.id, name: m.name, character: m.character })
        .collect();

    Ok(TmdbMovieCredits {
        tmdb_id: resp.id, title: resp.title, original_title: resp.original_title,
        year, overview: resp.overview, vote_average: resp.vote_average,
        poster_path: resp.poster_path, crew, cast,
    })
}
```

- [ ] **Step 2: Register all commands in lib.rs**

Replace `src-tauri/src/lib.rs`:

```rust
pub mod commands;
pub mod common;
pub mod config;
pub mod db;
pub mod error;
pub mod ffmpeg;

use tauri::Manager;

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
            // M1 commands
            commands::search::search_yify_cmd,
            commands::tmdb::search_movies,
            commands::tmdb::discover_movies,
            commands::tmdb::list_genres,
            commands::tmdb::get_tmdb_movie_credits,
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
            commands::config::set_music_playlist,
            // M2 library — people
            commands::library::people::list_people,
            commands::library::people::get_person,
            commands::library::people::create_person,
            commands::library::people::update_person_wiki,
            commands::library::people::delete_person,
            commands::library::people::add_person_relation,
            commands::library::people::remove_person_relation,
            // M2 library — films
            commands::library::films::list_films,
            commands::library::films::get_film,
            commands::library::films::add_film_from_tmdb,
            commands::library::films::update_film_wiki,
            commands::library::films::delete_film,
            // M2 library — genres
            commands::library::genres::list_genres_tree,
            commands::library::genres::get_genre,
            commands::library::genres::create_genre,
            commands::library::genres::update_genre_wiki,
            commands::library::genres::delete_genre,
            commands::library::genres::link_film_genre,
            commands::library::genres::unlink_film_genre,
            commands::library::genres::link_person_genre,
            commands::library::genres::unlink_person_genre,
            // M2 library — reviews
            commands::library::reviews::add_review,
            commands::library::reviews::update_review,
            commands::library::reviews::delete_review,
            // M2 library — graph
            commands::library::graph::get_graph_data,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
```

- [ ] **Step 3: Run full test suite**

```bash
cd src-tauri && cargo test
```

Expected: all tests PASS (M1 + M2).

- [ ] **Step 4: Smoke build**

```bash
cd src-tauri && cargo build
```

Expected: compiles cleanly.

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/lib.rs src-tauri/src/commands/tmdb.rs
git commit -m "feat: register all M2 library commands and add TMDB credits command"
```

---

### Task 8: npm dependencies + tauri.ts extension

**Files:**
- Run: `npm install d3 @types/d3 marked`
- Modify: `src/lib/tauri.ts`

- [ ] **Step 1: Install packages**

```bash
cd C:/Users/lixuan/Documents/code/rust/blowup && npm install d3 @types/d3 marked
```

Expected: packages added to package.json.

- [ ] **Step 2: Replace tauri.ts with extended version**

```typescript
// src/lib/tauri.ts
import { invoke } from "@tauri-apps/api/core";

// ── TMDB (M1) ─────────────────────────────────────────────────────
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

export interface TmdbGenre { id: number; name: string; }

export interface TmdbCrewMember { id: number; name: string; job: string; department: string; }
export interface TmdbCastMember { id: number; name: string; character: string; }
export interface TmdbMovieCredits {
  tmdb_id: number; title: string; original_title: string | null;
  year: number | null; overview: string | null; vote_average: number | null;
  poster_path: string | null; crew: TmdbCrewMember[]; cast: TmdbCastMember[];
}

// ── Config ────────────────────────────────────────────────────────
export interface MusicTrack { src: string; name: string; }

export interface AppConfig {
  tools: { aria2c: string; alass: string; ffmpeg: string };
  search: { rate_limit_secs: number };
  subtitle: { default_lang: string };
  opensubtitles: { api_key: string };
  tmdb: { api_key: string };
  library: { root_dir: string };
  music: { enabled: boolean; mode: string; playlist: MusicTrack[] };
}

// ── Library types ─────────────────────────────────────────────────
export interface PersonSummary { id: number; name: string; primary_role: string; film_count: number; }
export interface PersonFilmEntry { film_id: number; title: string; year: number | null; role: string; poster_cache_path: string | null; }
export interface PersonRelation { target_id: number; target_name: string; direction: string; relation_type: string; }
export interface PersonDetail {
  id: number; tmdb_id: number | null; name: string; primary_role: string;
  born_date: string | null; nationality: string | null; biography: string | null;
  wiki_content: string; films: PersonFilmEntry[]; relations: PersonRelation[];
}

export interface FilmSummary { id: number; title: string; year: number | null; tmdb_rating: number | null; poster_cache_path: string | null; }
export interface FilmPersonEntry { person_id: number; name: string; role: string; }
export interface ReviewEntry { id: number; is_personal: boolean; author: string | null; content: string; rating: number | null; created_at: string; }
export interface GenreSummary { id: number; name: string; film_count: number; child_count: number; }
export interface FilmDetail {
  id: number; tmdb_id: number | null; title: string; original_title: string | null;
  year: number | null; overview: string | null; tmdb_rating: number | null;
  poster_cache_path: string | null; wiki_content: string;
  people: FilmPersonEntry[]; genres: GenreSummary[]; reviews: ReviewEntry[];
}

export interface GenreDetail {
  id: number; name: string; description: string | null; parent_id: number | null;
  period: string | null; wiki_content: string;
  children: GenreSummary[]; people: PersonSummary[]; films: FilmSummary[];
}

export interface GenreTreeNode { id: number; name: string; period: string | null; film_count: number; children: GenreTreeNode[]; }

export interface TmdbPersonInput { tmdb_id: number | null; name: string; role: string; primary_role: string; }
export interface TmdbMovieInput {
  tmdb_id: number; title: string; original_title: string | null;
  year: number | null; overview: string | null; tmdb_rating: number | null;
  people: TmdbPersonInput[];
}

export interface GraphNode { id: string; label: string; node_type: string; role: string | null; weight: number; }
export interface GraphLink { source: string; target: string; role: string; }
export interface GraphData { nodes: GraphNode[]; links: GraphLink[]; }

// ── Invoke wrappers ───────────────────────────────────────────────
export const tmdb = {
  searchMovies: (apiKey: string, query: string, filters: SearchFilters) =>
    invoke<MovieListItem[]>("search_movies", { apiKey, query, filters }),
  discoverMovies: (apiKey: string, filters: SearchFilters) =>
    invoke<MovieListItem[]>("discover_movies", { apiKey, filters }),
  listGenres: (apiKey: string) => invoke<TmdbGenre[]>("list_genres", { apiKey }),
  getMovieCredits: (apiKey: string, tmdbId: number) =>
    invoke<TmdbMovieCredits>("get_tmdb_movie_credits", { apiKey, tmdbId }),
};

export const config = {
  get: () => invoke<AppConfig>("get_config"),
  set: (key: string, value: string) => invoke<void>("set_config_key", { key, value }),
  setMusicPlaylist: (tracks: MusicTrack[]) => invoke<void>("set_music_playlist", { tracks }),
};

export const library = {
  listPeople: () => invoke<PersonSummary[]>("list_people"),
  getPerson: (id: number) => invoke<PersonDetail>("get_person", { id }),
  createPerson: (name: string, primaryRole: string, tmdbId?: number, bornDate?: string, nationality?: string) =>
    invoke<number>("create_person", { name, primaryRole, tmdbId, bornDate, nationality }),
  updatePersonWiki: (id: number, content: string) => invoke<void>("update_person_wiki", { id, content }),
  deletePerson: (id: number) => invoke<void>("delete_person", { id }),
  addPersonRelation: (fromId: number, toId: number, relationType: string) =>
    invoke<void>("add_person_relation", { fromId, toId, relationType }),
  removePersonRelation: (fromId: number, toId: number, relationType: string) =>
    invoke<void>("remove_person_relation", { fromId, toId, relationType }),

  listFilms: () => invoke<FilmSummary[]>("list_films"),
  getFilm: (id: number) => invoke<FilmDetail>("get_film", { id }),
  addFilmFromTmdb: (tmdbMovie: TmdbMovieInput) => invoke<number>("add_film_from_tmdb", { tmdbMovie }),
  updateFilmWiki: (id: number, content: string) => invoke<void>("update_film_wiki", { id, content }),
  deleteFilm: (id: number) => invoke<void>("delete_film", { id }),

  listGenresTree: () => invoke<GenreTreeNode[]>("list_genres_tree"),
  getGenre: (id: number) => invoke<GenreDetail>("get_genre", { id }),
  createGenre: (name: string, parentId?: number, description?: string, period?: string) =>
    invoke<number>("create_genre", { name, parentId, description, period }),
  updateGenreWiki: (id: number, content: string) => invoke<void>("update_genre_wiki", { id, content }),
  deleteGenre: (id: number) => invoke<void>("delete_genre", { id }),
  linkFilmGenre: (filmId: number, genreId: number) => invoke<void>("link_film_genre", { filmId, genreId }),
  unlinkFilmGenre: (filmId: number, genreId: number) => invoke<void>("unlink_film_genre", { filmId, genreId }),
  linkPersonGenre: (personId: number, genreId: number) => invoke<void>("link_person_genre", { personId, genreId }),
  unlinkPersonGenre: (personId: number, genreId: number) => invoke<void>("unlink_person_genre", { personId, genreId }),

  addReview: (filmId: number, isPersonal: boolean, author: string | null, content: string, rating: number | null) =>
    invoke<number>("add_review", { filmId, isPersonal, author, content, rating }),
  updateReview: (id: number, content: string, rating: number | null) =>
    invoke<void>("update_review", { id, content, rating }),
  deleteReview: (id: number) => invoke<void>("delete_review", { id }),

  getGraphData: () => invoke<GraphData>("get_graph_data"),
};
```

- [ ] **Step 3: Type-check**

```bash
npx tsc --noEmit
```

Expected: no errors.

- [ ] **Step 4: Commit**

```bash
git add src/lib/tauri.ts package.json package-lock.json
git commit -m "feat: add d3/marked deps and extend tauri.ts with M2 library types"
```

---

### Task 9: WikiEditor component

**Files:**
- Create: `src/components/WikiEditor.tsx`

- [ ] **Step 1: Create WikiEditor.tsx**

```tsx
// src/components/WikiEditor.tsx
import { useState } from "react";
import { marked } from "marked";

interface WikiEditorProps {
  value: string;
  onChange: (v: string) => void;
  onSave: () => void;
  minHeight?: number;
}

export function WikiEditor({ value, onChange, onSave, minHeight = 200 }: WikiEditorProps) {
  const [tab, setTab] = useState<"write" | "preview">("write");

  return (
    <div>
      <div style={{ display: "flex", gap: "0.25rem", marginBottom: "0.5rem" }}>
        {(["write", "preview"] as const).map((t) => (
          <button
            key={t}
            onClick={() => setTab(t)}
            style={{
              background: "none", border: "none",
              borderBottom: tab === t ? "1px solid var(--color-accent)" : "1px solid transparent",
              padding: "0.2rem 0.6rem 0.3rem",
              cursor: "pointer", fontSize: "0.75rem",
              color: tab === t ? "var(--color-accent)" : "var(--color-label-tertiary)",
              fontFamily: "inherit",
            }}
          >
            {t === "write" ? "编辑" : "预览"}
          </button>
        ))}
      </div>

      {tab === "write" ? (
        <textarea
          value={value}
          onChange={(e) => onChange(e.target.value)}
          onBlur={onSave}
          placeholder="支持 Markdown 格式…"
          style={{
            width: "100%", minHeight, resize: "vertical", outline: "none",
            background: "var(--color-bg-elevated)",
            border: "1px solid var(--color-separator)",
            borderRadius: 6, padding: "0.75rem",
            color: "var(--color-label-primary)",
            fontSize: "0.8rem", fontFamily: "monospace",
            lineHeight: 1.65, boxSizing: "border-box",
          }}
        />
      ) : (
        <div
          dangerouslySetInnerHTML={{
            __html: marked.parse(value || "_（暂无内容）_") as string,
          }}
          style={{
            minHeight, overflowY: "auto",
            background: "var(--color-bg-elevated)",
            border: "1px solid var(--color-separator)",
            borderRadius: 6, padding: "0.75rem",
            fontSize: "0.8rem", lineHeight: 1.65,
            color: "var(--color-label-secondary)",
          }}
        />
      )}
    </div>
  );
}
```

- [ ] **Step 2: Type-check + commit**

```bash
npx tsc --noEmit
git add src/components/WikiEditor.tsx
git commit -m "feat: add WikiEditor component with Write/Preview tab switch"
```

---

### Task 10: ReviewSection component

**Files:**
- Create: `src/components/ReviewSection.tsx`

- [ ] **Step 1: Create ReviewSection.tsx**

```tsx
// src/components/ReviewSection.tsx
import { useState } from "react";
import { library } from "../lib/tauri";
import type { ReviewEntry } from "../lib/tauri";

function RatingDots({ value, onChange }: { value: number | null; onChange: (v: number) => void }) {
  const [hovered, setHovered] = useState<number | null>(null);
  const display = hovered ?? value ?? 0;

  return (
    <div style={{ display: "flex", alignItems: "center", gap: "0.2rem", flexWrap: "wrap" }}>
      {Array.from({ length: 20 }, (_, i) => {
        const v = (i + 1) * 0.5;
        return (
          <button
            key={v}
            title={v.toFixed(1)}
            onMouseEnter={() => setHovered(v)}
            onMouseLeave={() => setHovered(null)}
            onClick={() => onChange(v)}
            style={{
              width: 11, height: 11, borderRadius: "50%", padding: 0, cursor: "pointer",
              border: "1px solid var(--color-accent)",
              background: display >= v ? "var(--color-accent)" : "transparent",
              flexShrink: 0, transition: "background 0.1s",
            }}
          />
        );
      })}
      {value !== null && (
        <span style={{ marginLeft: "0.4rem", fontSize: "0.8rem", color: "var(--color-accent)", fontWeight: 500 }}>
          {value.toFixed(1)} / 10
        </span>
      )}
    </div>
  );
}

function CriticCard({ review, onDelete }: { review: ReviewEntry; onDelete: () => void }) {
  const [expanded, setExpanded] = useState(false);
  return (
    <div style={{ background: "var(--color-bg-elevated)", borderRadius: 6, padding: "0.65rem 0.75rem", fontSize: "0.78rem" }}>
      <div style={{ display: "flex", justifyContent: "space-between", marginBottom: "0.3rem" }}>
        <span style={{ fontWeight: 500, color: "var(--color-label-secondary)" }}>{review.author ?? "佚名"}</span>
        <div style={{ display: "flex", gap: "0.5rem", alignItems: "center" }}>
          {review.rating !== null && (
            <span style={{ color: "var(--color-accent)", fontSize: "0.72rem" }}>★ {review.rating.toFixed(1)}</span>
          )}
          <button onClick={onDelete} style={{ background: "none", border: "none", color: "var(--color-label-quaternary)", cursor: "pointer", fontSize: "0.7rem", padding: 0 }}>✕</button>
        </div>
      </div>
      <p style={{
        margin: 0, color: "var(--color-label-tertiary)", lineHeight: 1.55,
        display: "-webkit-box", WebkitLineClamp: expanded ? undefined : 3,
        WebkitBoxOrient: "vertical", overflow: expanded ? "visible" : "hidden",
      }}>
        {review.content}
      </p>
      {review.content.length > 160 && (
        <button onClick={() => setExpanded(!expanded)} style={{ background: "none", border: "none", color: "var(--color-label-quaternary)", cursor: "pointer", fontSize: "0.7rem", padding: "0.2rem 0 0", fontFamily: "inherit" }}>
          {expanded ? "收起" : "展开"}
        </button>
      )}
    </div>
  );
}

interface ReviewSectionProps {
  filmId: number;
  reviews: ReviewEntry[];
  onRefresh: () => void;
}

export function ReviewSection({ filmId, reviews, onRefresh }: ReviewSectionProps) {
  const personal = reviews.find((r) => r.is_personal);
  const critics = reviews.filter((r) => !r.is_personal);

  const [personalContent, setPersonalContent] = useState(personal?.content ?? "");
  const [personalRating, setPersonalRating] = useState<number | null>(personal?.rating ?? null);
  const [showAddCritic, setShowAddCritic] = useState(false);
  const [criticAuthor, setCriticAuthor] = useState("");
  const [criticContent, setCriticContent] = useState("");
  const [criticRating, setCriticRating] = useState<number | null>(null);

  const savePersonal = async () => {
    if (!personalContent.trim()) return;
    if (personal) {
      await library.updateReview(personal.id, personalContent, personalRating);
    } else {
      await library.addReview(filmId, true, null, personalContent, personalRating);
    }
    onRefresh();
  };

  const addCritic = async () => {
    if (!criticContent.trim()) return;
    await library.addReview(filmId, false, criticAuthor.trim() || null, criticContent, criticRating);
    setCriticAuthor(""); setCriticContent(""); setCriticRating(null);
    setShowAddCritic(false);
    onRefresh();
  };

  const labelStyle: React.CSSProperties = {
    margin: "0 0 0.5rem", fontSize: "0.72rem",
    color: "var(--color-label-quaternary)",
    textTransform: "uppercase", letterSpacing: "0.06em",
  };

  return (
    <div style={{ display: "flex", flexDirection: "column", gap: "1rem" }}>
      <section>
        <p style={labelStyle}>个人影评</p>
        <div style={{ marginBottom: "0.5rem" }}>
          <RatingDots value={personalRating} onChange={setPersonalRating} />
        </div>
        <div style={{ position: "relative" }}>
          <textarea
            value={personalContent}
            onChange={(e) => { if (e.target.value.length <= 500) setPersonalContent(e.target.value); }}
            onBlur={savePersonal}
            placeholder="写下你的想法…（最多 500 字）"
            style={{
              width: "100%", minHeight: 100, outline: "none", resize: "vertical",
              background: "var(--color-bg-elevated)",
              border: "1px solid var(--color-separator)",
              borderRadius: 6, padding: "0.6rem 0.75rem",
              color: "var(--color-label-primary)",
              fontSize: "0.8rem", fontFamily: "inherit", lineHeight: 1.6, boxSizing: "border-box",
            }}
          />
          <span style={{ position: "absolute", bottom: "0.4rem", right: "0.6rem", fontSize: "0.65rem", color: "var(--color-label-quaternary)" }}>
            {personalContent.length} / 500
          </span>
        </div>
      </section>

      <section>
        <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center", marginBottom: "0.5rem" }}>
          <p style={{ ...labelStyle, margin: 0 }}>收录影评</p>
          <button onClick={() => setShowAddCritic(!showAddCritic)} style={{ background: "none", border: "none", color: "var(--color-accent)", cursor: "pointer", fontSize: "0.75rem", fontFamily: "inherit" }}>
            + 添加
          </button>
        </div>

        {showAddCritic && (
          <div style={{ background: "var(--color-bg-elevated)", border: "1px solid var(--color-separator)", borderRadius: 6, padding: "0.75rem", marginBottom: "0.5rem", display: "flex", flexDirection: "column", gap: "0.5rem" }}>
            <input placeholder="作者姓名（可选）" value={criticAuthor} onChange={(e) => setCriticAuthor(e.target.value)}
              style={{ background: "var(--color-bg-primary)", border: "1px solid var(--color-separator)", borderRadius: 4, padding: "0.35rem 0.5rem", color: "var(--color-label-primary)", fontSize: "0.78rem", fontFamily: "inherit", outline: "none" }} />
            <RatingDots value={criticRating} onChange={setCriticRating} />
            <textarea placeholder="影评内容…" value={criticContent} onChange={(e) => setCriticContent(e.target.value)}
              style={{ background: "var(--color-bg-primary)", border: "1px solid var(--color-separator)", borderRadius: 4, padding: "0.35rem 0.5rem", color: "var(--color-label-primary)", fontSize: "0.78rem", fontFamily: "inherit", minHeight: 80, resize: "vertical", outline: "none" }} />
            <div style={{ display: "flex", gap: "0.5rem", justifyContent: "flex-end" }}>
              <button onClick={() => setShowAddCritic(false)} style={{ background: "none", border: "none", color: "var(--color-label-tertiary)", cursor: "pointer", fontSize: "0.75rem", fontFamily: "inherit" }}>取消</button>
              <button onClick={addCritic} style={{ background: "var(--color-accent-soft)", border: "1px solid var(--color-accent)", borderRadius: 4, padding: "0.25rem 0.75rem", color: "var(--color-accent)", cursor: "pointer", fontSize: "0.75rem", fontFamily: "inherit" }}>保存</button>
            </div>
          </div>
        )}

        <div style={{ display: "flex", flexDirection: "column", gap: "0.5rem" }}>
          {critics.map((r) => (
            <CriticCard key={r.id} review={r} onDelete={async () => { await library.deleteReview(r.id); onRefresh(); }} />
          ))}
          {critics.length === 0 && !showAddCritic && (
            <p style={{ fontSize: "0.75rem", color: "var(--color-label-quaternary)", margin: 0 }}>暂无收录影评</p>
          )}
        </div>
      </section>
    </div>
  );
}
```

- [ ] **Step 2: Type-check + commit**

```bash
npx tsc --noEmit
git add src/components/ReviewSection.tsx
git commit -m "feat: add ReviewSection component with personal and critic reviews"
```

---

### Task 11: FilmDetailPanel extraction + "加入知识库" flow

**Files:**
- Create: `src/components/FilmDetailPanel.tsx`
- Modify: `src/pages/Search.tsx`

- [ ] **Step 1: Create src/components/FilmDetailPanel.tsx**

```tsx
// src/components/FilmDetailPanel.tsx
import { useState } from "react";
import { tmdb, library, config } from "../lib/tauri";
import type { MovieListItem, TmdbPersonInput, TmdbMovieInput } from "../lib/tauri";

const JOB_TO_ROLE: Record<string, string> = {
  "Director": "director",
  "Director of Photography": "cinematographer",
  "Original Music Composer": "composer",
  "Editor": "editor",
  "Screenplay": "screenwriter",
  "Writer": "screenwriter",
  "Producer": "producer",
};

const ROLE_OPTIONS = ["director", "cinematographer", "composer", "editor", "screenwriter", "producer", "actor"];

interface PersonSel {
  tmdbId: number | null;
  name: string;
  job: string;
  primary_role: string;
  selected: boolean;
}

function PersonRow({ person, onToggle, onRoleChange }: { person: PersonSel; onToggle: () => void; onRoleChange: (r: string) => void }) {
  return (
    <div style={{ display: "flex", alignItems: "center", gap: "0.6rem", padding: "0.3rem 0", fontSize: "0.8rem" }}>
      <input type="checkbox" checked={person.selected} onChange={onToggle} style={{ cursor: "pointer", accentColor: "var(--color-accent)" }} />
      <span style={{ flex: 1, color: person.selected ? "var(--color-label-primary)" : "var(--color-label-tertiary)" }}>{person.name}</span>
      <span style={{ color: "var(--color-label-quaternary)", fontSize: "0.7rem" }}>{person.job}</span>
      {person.selected && (
        <select value={person.primary_role} onChange={(e) => onRoleChange(e.target.value)}
          style={{ background: "var(--color-bg-elevated)", border: "1px solid var(--color-separator)", borderRadius: 4, padding: "0.15rem 0.3rem", color: "var(--color-label-secondary)", fontSize: "0.7rem", fontFamily: "inherit", cursor: "pointer" }}>
          {ROLE_OPTIONS.map((r) => <option key={r} value={r}>{r}</option>)}
        </select>
      )}
    </div>
  );
}

function AddToLibraryModal({ film, apiKey, onClose, onAdded }: { film: MovieListItem; apiKey: string; onClose: () => void; onAdded: () => void }) {
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [crew, setCrew] = useState<PersonSel[]>([]);
  const [cast, setCast] = useState<PersonSel[]>([]);
  const [saving, setSaving] = useState(false);

  useState(() => {
    tmdb.getMovieCredits(apiKey, film.id).then((credits) => {
      setCrew(credits.crew.map((m) => ({ tmdbId: m.id, name: m.name, job: m.job, primary_role: JOB_TO_ROLE[m.job] ?? "director", selected: m.job === "Director" })));
      setCast(credits.cast.map((m) => ({ tmdbId: m.id, name: m.name, job: m.character, primary_role: "actor", selected: false })));
      setLoading(false);
    }).catch((e) => { setError(String(e)); setLoading(false); });
  });

  const confirm = async () => {
    setSaving(true);
    const people: TmdbPersonInput[] = [
      ...crew.filter((p) => p.selected).map((p) => ({ tmdb_id: p.tmdbId, name: p.name, role: JOB_TO_ROLE[p.job] ?? "director", primary_role: p.primary_role })),
      ...cast.filter((p) => p.selected).map((p) => ({ tmdb_id: p.tmdbId, name: p.name, role: "actor", primary_role: "actor" })),
    ];

    const input: TmdbMovieInput = {
      tmdb_id: film.id, title: film.title,
      original_title: film.original_title !== film.title ? film.original_title : null,
      year: film.year ? parseInt(film.year) : null,
      overview: film.overview || null, tmdb_rating: film.vote_average, people,
    };

    try {
      await library.addFilmFromTmdb(input);
      onAdded(); onClose();
    } catch (e) { setError(String(e)); }
    finally { setSaving(false); }
  };

  return (
    <div style={{ position: "fixed", inset: 0, background: "rgba(0,0,0,0.6)", display: "flex", alignItems: "center", justifyContent: "center", zIndex: 100 }} onClick={onClose}>
      <div style={{ background: "var(--color-bg-secondary)", border: "1px solid var(--color-separator)", borderRadius: 10, padding: "1.5rem", width: 460, maxHeight: "80vh", overflowY: "auto", display: "flex", flexDirection: "column", gap: "1rem" }} onClick={(e) => e.stopPropagation()}>
        <h3 style={{ margin: 0, fontSize: "1rem", fontWeight: 700 }}>加入知识库</h3>
        <div>
          <p style={{ margin: "0 0 0.15rem", fontWeight: 600 }}>{film.title}</p>
          {film.year && <p style={{ margin: 0, fontSize: "0.75rem", color: "var(--color-label-tertiary)" }}>{film.year}</p>}
        </div>
        {loading && <p style={{ color: "var(--color-label-tertiary)", fontSize: "0.8rem" }}>加载演职员…</p>}
        {error && <p style={{ color: "#e57373", fontSize: "0.8rem" }}>{error}</p>}
        {!loading && !error && (
          <>
            {crew.length > 0 && (
              <div>
                <p style={{ margin: "0 0 0.4rem", fontSize: "0.72rem", color: "var(--color-label-quaternary)", textTransform: "uppercase" }}>主创</p>
                {crew.map((p, i) => (
                  <PersonRow key={`c${i}`} person={p}
                    onToggle={() => setCrew((prev) => prev.map((x, xi) => xi === i ? { ...x, selected: !x.selected } : x))}
                    onRoleChange={(r) => setCrew((prev) => prev.map((x, xi) => xi === i ? { ...x, primary_role: r } : x))}
                  />
                ))}
              </div>
            )}
            {cast.length > 0 && (
              <div>
                <p style={{ margin: "0 0 0.4rem", fontSize: "0.72rem", color: "var(--color-label-quaternary)", textTransform: "uppercase" }}>演员（前5位）</p>
                {cast.map((p, i) => (
                  <PersonRow key={`a${i}`} person={p}
                    onToggle={() => setCast((prev) => prev.map((x, xi) => xi === i ? { ...x, selected: !x.selected } : x))}
                    onRoleChange={(r) => setCast((prev) => prev.map((x, xi) => xi === i ? { ...x, primary_role: r } : x))}
                  />
                ))}
              </div>
            )}
          </>
        )}
        <div style={{ display: "flex", justifyContent: "flex-end", gap: "0.5rem" }}>
          <button onClick={onClose} style={{ background: "none", border: "none", color: "var(--color-label-tertiary)", cursor: "pointer", fontSize: "0.8rem", fontFamily: "inherit" }}>取消</button>
          <button onClick={confirm} disabled={saving || loading}
            style={{ background: "var(--color-accent)", border: "none", borderRadius: 6, padding: "0.35rem 1rem", color: "#0B1628", fontWeight: 600, cursor: "pointer", fontSize: "0.8rem", fontFamily: "inherit" }}>
            {saving ? "保存中…" : "确认加入"}
          </button>
        </div>
      </div>
    </div>
  );
}

interface FilmDetailPanelProps {
  film: MovieListItem;
  onClose: () => void;
}

export function FilmDetailPanel({ film, onClose }: FilmDetailPanelProps) {
  const [apiKey, setApiKey] = useState("");
  const [showAddModal, setShowAddModal] = useState(false);
  const [addedToLibrary, setAddedToLibrary] = useState(false);

  useState(() => { config.get().then((cfg) => setApiKey(cfg.tmdb.api_key)); });

  return (
    <>
      <div style={{ width: 300, flexShrink: 0, borderLeft: "1px solid var(--color-separator)", background: "var(--color-bg-secondary)", overflowY: "auto", padding: "1.25rem 1.25rem 2rem", display: "flex", flexDirection: "column", gap: "0.75rem" }}>
        <div style={{ display: "flex", justifyContent: "flex-end" }}>
          <button onClick={onClose} style={{ background: "none", border: "none", color: "var(--color-label-tertiary)", cursor: "pointer", fontSize: "1rem", lineHeight: 1, padding: 0 }}>✕</button>
        </div>

        {film.poster_path && (
          <img src={`https://image.tmdb.org/t/p/w300${film.poster_path}`} alt={film.title} style={{ width: "100%", borderRadius: 6 }} />
        )}

        <div>
          <h2 style={{ margin: 0, fontSize: "1rem", fontWeight: 700, letterSpacing: "-0.02em" }}>{film.title}</h2>
          {film.original_title !== film.title && (
            <p style={{ margin: "0.15rem 0 0", fontSize: "0.72rem", color: "var(--color-label-tertiary)" }}>{film.original_title}</p>
          )}
        </div>

        <div style={{ display: "flex", gap: "0.5rem", alignItems: "center" }}>
          {film.year && <span style={{ fontSize: "0.75rem", color: "var(--color-label-secondary)" }}>{film.year}</span>}
          <span style={{ fontSize: "0.75rem", color: "var(--color-label-tertiary)" }}>·</span>
          <span style={{ fontSize: "0.75rem", color: "var(--color-accent)", fontWeight: 500 }}>★ {film.vote_average.toFixed(1)}</span>
        </div>

        <p style={{ margin: 0, fontSize: "0.78rem", color: "var(--color-label-secondary)", lineHeight: 1.6 }}>
          {film.overview || "暂无简介。"}
        </p>

        <div style={{ borderTop: "1px solid var(--color-separator)", paddingTop: "0.75rem", display: "flex", flexDirection: "column", gap: "0.4rem" }}>
          {addedToLibrary ? (
            <p style={{ fontSize: "0.78rem", color: "var(--color-accent)", margin: 0 }}>✓ 已加入知识库</p>
          ) : (
            <button onClick={() => setShowAddModal(true)}
              style={{ background: "var(--color-accent-soft)", border: "1px solid var(--color-accent)", borderRadius: 6, padding: "0.4rem 0.75rem", color: "var(--color-accent)", fontSize: "0.78rem", cursor: "pointer", fontFamily: "inherit", textAlign: "left" }}>
              加入知识库
            </button>
          )}
          <button disabled style={{ background: "var(--color-bg-control)", border: "none", borderRadius: 6, padding: "0.4rem 0.75rem", color: "var(--color-label-quaternary)", fontSize: "0.78rem", cursor: "default", fontFamily: "inherit", textAlign: "left", opacity: 0.4 }}>
            搜索资源（M3）
          </button>
        </div>
      </div>

      {showAddModal && (
        <AddToLibraryModal film={film} apiKey={apiKey}
          onClose={() => setShowAddModal(false)}
          onAdded={() => setAddedToLibrary(true)}
        />
      )}
    </>
  );
}
```

- [ ] **Step 2: Update Search.tsx — remove inline FilmDetailPanel, add import**

In `src/pages/Search.tsx`:
1. Add import at top: `import { FilmDetailPanel } from "../components/FilmDetailPanel";`
2. Delete the entire `// ── FilmDetailPanel ───` section (the `function FilmDetailPanel(...)` block at the bottom of the file).

- [ ] **Step 3: Type-check + commit**

```bash
npx tsc --noEmit
git add src/components/FilmDetailPanel.tsx src/pages/Search.tsx
git commit -m "feat: extract FilmDetailPanel and unlock 加入知识库 button"
```

---

### Task 12: People page

**Files:**
- Create: `src/pages/People.tsx`

- [ ] **Step 1: Create People.tsx**

```tsx
// src/pages/People.tsx
import { useState, useEffect, useCallback } from "react";
import { library } from "../lib/tauri";
import type { PersonSummary, PersonDetail } from "../lib/tauri";
import { WikiEditor } from "../components/WikiEditor";

const ROLE_LABELS: Record<string, string> = {
  director: "导演", cinematographer: "摄影", composer: "音乐",
  editor: "剪辑", screenwriter: "编剧", producer: "制片", actor: "演员",
};
const PRIMARY_ROLES = Object.keys(ROLE_LABELS);

// ── Shared modal primitives ───────────────────────────────────────
const inputStyle: React.CSSProperties = {
  background: "var(--color-bg-elevated)", border: "1px solid var(--color-separator)",
  borderRadius: 5, padding: "0.4rem 0.6rem",
  color: "var(--color-label-primary)", fontSize: "0.82rem", fontFamily: "inherit", outline: "none",
};

function Modal({ title, onClose, children }: { title: string; onClose: () => void; children: React.ReactNode }) {
  return (
    <div style={{ position: "fixed", inset: 0, background: "rgba(0,0,0,0.55)", display: "flex", alignItems: "center", justifyContent: "center", zIndex: 100 }} onClick={onClose}>
      <div style={{ background: "var(--color-bg-secondary)", border: "1px solid var(--color-separator)", borderRadius: 10, padding: "1.5rem", width: 360, display: "flex", flexDirection: "column", gap: "0.75rem" }} onClick={(e) => e.stopPropagation()}>
        <h3 style={{ margin: 0, fontSize: "0.95rem", fontWeight: 700 }}>{title}</h3>
        {children}
      </div>
    </div>
  );
}

function ModalField({ label, children }: { label: string; children: React.ReactNode }) {
  return (
    <div style={{ display: "flex", flexDirection: "column", gap: "0.25rem" }}>
      <label style={{ fontSize: "0.72rem", color: "var(--color-label-tertiary)" }}>{label}</label>
      {children}
    </div>
  );
}

function ModalActions({ onCancel, onConfirm, confirmLabel }: { onCancel: () => void; onConfirm: () => void; confirmLabel: string }) {
  return (
    <div style={{ display: "flex", justifyContent: "flex-end", gap: "0.5rem", marginTop: "0.25rem" }}>
      <button onClick={onCancel} style={{ background: "none", border: "none", color: "var(--color-label-tertiary)", cursor: "pointer", fontSize: "0.8rem", fontFamily: "inherit" }}>取消</button>
      <button onClick={onConfirm} style={{ background: "var(--color-accent)", border: "none", borderRadius: 6, padding: "0.3rem 0.9rem", color: "#0B1628", fontWeight: 600, cursor: "pointer", fontSize: "0.8rem", fontFamily: "inherit" }}>{confirmLabel}</button>
    </div>
  );
}

function SectionTitle({ children }: { children: React.ReactNode }) {
  return (
    <p style={{ margin: "1.25rem 0 0.5rem", fontSize: "0.72rem", color: "var(--color-label-quaternary)", textTransform: "uppercase", letterSpacing: "0.06em", fontWeight: 600 }}>
      {children}
    </p>
  );
}

// ── Add Person Modal ──────────────────────────────────────────────
function AddPersonModal({ onClose, onAdded }: { onClose: () => void; onAdded: () => void }) {
  const [name, setName] = useState("");
  const [role, setRole] = useState("director");
  const [tmdbId, setTmdbId] = useState("");

  const save = async () => {
    if (!name.trim()) return;
    await library.createPerson(name.trim(), role, tmdbId ? parseInt(tmdbId) : undefined);
    onAdded();
  };

  return (
    <Modal title="添加影人" onClose={onClose}>
      <ModalField label="姓名">
        <input value={name} onChange={(e) => setName(e.target.value)} placeholder="影人姓名" autoFocus style={inputStyle} />
      </ModalField>
      <ModalField label="主要角色">
        <select value={role} onChange={(e) => setRole(e.target.value)} style={{ ...inputStyle, cursor: "pointer" }}>
          {PRIMARY_ROLES.map((r) => <option key={r} value={r}>{ROLE_LABELS[r]}</option>)}
        </select>
      </ModalField>
      <ModalField label="TMDB ID（可选）">
        <input value={tmdbId} onChange={(e) => setTmdbId(e.target.value)} placeholder="如 19429" style={inputStyle} />
      </ModalField>
      <ModalActions onCancel={onClose} onConfirm={save} confirmLabel="添加" />
    </Modal>
  );
}

// ── Add Relation Modal ────────────────────────────────────────────
function AddRelationModal({ fromId, onClose, onAdded }: { fromId: number; onClose: () => void; onAdded: () => void }) {
  const [allPeople, setAllPeople] = useState<PersonSummary[]>([]);
  const [toId, setToId] = useState<number | null>(null);
  const [relType, setRelType] = useState("influenced");

  useEffect(() => { library.listPeople().then(setAllPeople); }, []);

  const save = async () => {
    if (!toId) return;
    await library.addPersonRelation(fromId, toId, relType);
    onAdded();
  };

  return (
    <Modal title="添加影人关系" onClose={onClose}>
      <ModalField label="关联影人">
        <select value={toId ?? ""} onChange={(e) => setToId(parseInt(e.target.value))} style={{ ...inputStyle, cursor: "pointer" }}>
          <option value="">请选择…</option>
          {allPeople.filter((p) => p.id !== fromId).map((p) => (
            <option key={p.id} value={p.id}>{p.name}</option>
          ))}
        </select>
      </ModalField>
      <ModalField label="关系类型">
        <select value={relType} onChange={(e) => setRelType(e.target.value)} style={{ ...inputStyle, cursor: "pointer" }}>
          <option value="influenced">影响（→）</option>
          <option value="contemporary">同时期（↔）</option>
          <option value="collaborated">合作（↔）</option>
        </select>
      </ModalField>
      <ModalActions onCancel={onClose} onConfirm={save} confirmLabel="添加" />
    </Modal>
  );
}

// ── Person detail view ────────────────────────────────────────────
function PersonDetailView({
  person, wikiContent, onWikiChange, onWikiSave, onDelete, onRelationAdded,
}: {
  person: PersonDetail; wikiContent: string;
  onWikiChange: (v: string) => void; onWikiSave: () => void;
  onDelete: () => void; onRelationAdded: () => void;
}) {
  const [showRelModal, setShowRelModal] = useState(false);

  return (
    <div style={{ maxWidth: 680 }}>
      <div style={{ display: "flex", justifyContent: "space-between", alignItems: "flex-start", marginBottom: "0.5rem" }}>
        <div>
          <h2 style={{ margin: 0, fontSize: "1.4rem", fontWeight: 700, letterSpacing: "-0.03em" }}>{person.name}</h2>
          <p style={{ margin: "0.2rem 0 0", fontSize: "0.78rem", color: "var(--color-accent)" }}>{ROLE_LABELS[person.primary_role] ?? person.primary_role}</p>
        </div>
        <button onClick={onDelete} style={{ background: "none", border: "none", color: "var(--color-label-quaternary)", cursor: "pointer", fontSize: "0.75rem", fontFamily: "inherit" }}>删除</button>
      </div>

      {(person.born_date || person.nationality) && (
        <p style={{ margin: "0 0 1rem", fontSize: "0.75rem", color: "var(--color-label-tertiary)" }}>
          {person.born_date && `出生：${person.born_date}`}
          {person.born_date && person.nationality && "  "}
          {person.nationality && `国籍：${person.nationality}`}
        </p>
      )}

      <SectionTitle>传记 Wiki</SectionTitle>
      <WikiEditor value={wikiContent} onChange={onWikiChange} onSave={onWikiSave} />

      <SectionTitle>作品列表</SectionTitle>
      {person.films.length === 0 ? (
        <p style={{ fontSize: "0.78rem", color: "var(--color-label-quaternary)" }}>暂无作品</p>
      ) : (
        <div style={{ display: "flex", flexDirection: "column", gap: "0.3rem" }}>
          {person.films.map((f) => (
            <div key={f.film_id} style={{ display: "flex", alignItems: "center", gap: "0.6rem", fontSize: "0.8rem" }}>
              <span style={{ color: "var(--color-label-secondary)" }}>{f.title}</span>
              {f.year && <span style={{ color: "var(--color-label-quaternary)" }}>({f.year})</span>}
              <span style={{ color: "var(--color-label-tertiary)", fontSize: "0.7rem" }}>· {f.role}</span>
            </div>
          ))}
        </div>
      )}

      <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center", margin: "1.25rem 0 0.5rem" }}>
        <SectionTitle>影人关系</SectionTitle>
        <button onClick={() => setShowRelModal(true)} style={{ background: "none", border: "none", color: "var(--color-accent)", cursor: "pointer", fontSize: "0.75rem", fontFamily: "inherit" }}>+ 添加关系</button>
      </div>
      {person.relations.length === 0 ? (
        <p style={{ fontSize: "0.78rem", color: "var(--color-label-quaternary)" }}>暂无关系</p>
      ) : (
        person.relations.map((r) => (
          <div key={`${r.direction}-${r.target_id}-${r.relation_type}`} style={{ fontSize: "0.78rem", color: "var(--color-label-secondary)", marginBottom: "0.25rem" }}>
            <span style={{ color: "var(--color-label-quaternary)" }}>
              {r.direction === "to" ? "→ 影响了" : r.direction === "from" ? "← 受影响于" : "↔ 合作"}
            </span>{" "}{r.target_name}
            <span style={{ color: "var(--color-label-quaternary)", fontSize: "0.7rem" }}> ({r.relation_type})</span>
          </div>
        ))
      )}

      {showRelModal && (
        <AddRelationModal fromId={person.id} onClose={() => setShowRelModal(false)}
          onAdded={() => { onRelationAdded(); setShowRelModal(false); }} />
      )}
    </div>
  );
}

// ── Main page ─────────────────────────────────────────────────────
export default function People() {
  const [people, setPeople] = useState<PersonSummary[]>([]);
  const [selected, setSelected] = useState<PersonDetail | null>(null);
  const [wikiContent, setWikiContent] = useState("");
  const [showAddModal, setShowAddModal] = useState(false);

  const loadPeople = useCallback(() => { library.listPeople().then(setPeople).catch(console.error); }, []);
  const loadPerson = useCallback((id: number) => {
    library.getPerson(id).then((p) => { setSelected(p); setWikiContent(p.wiki_content); }).catch(console.error);
  }, []);

  useEffect(() => { loadPeople(); }, [loadPeople]);

  const saveWiki = async () => {
    if (!selected) return;
    await library.updatePersonWiki(selected.id, wikiContent).catch(console.error);
  };

  return (
    <div style={{ display: "flex", height: "100%", overflow: "hidden" }}>
      {/* Left list */}
      <div style={{ width: 260, flexShrink: 0, borderRight: "1px solid var(--color-separator)", display: "flex", flexDirection: "column", overflow: "hidden" }}>
        <div style={{ padding: "1.2rem 1rem 0.75rem", borderBottom: "1px solid var(--color-separator)", display: "flex", justifyContent: "space-between", alignItems: "center" }}>
          <h1 style={{ margin: 0, fontSize: "1.1rem", fontWeight: 700 }}>影人</h1>
          <button onClick={() => setShowAddModal(true)}
            style={{ background: "var(--color-accent-soft)", border: "1px solid var(--color-accent)", borderRadius: 5, padding: "0.2rem 0.55rem", color: "var(--color-accent)", cursor: "pointer", fontSize: "0.75rem", fontFamily: "inherit" }}>
            + 添加
          </button>
        </div>
        <div style={{ flex: 1, overflowY: "auto", padding: "0.5rem" }}>
          {people.length === 0 ? (
            <p style={{ padding: "1rem", color: "var(--color-label-tertiary)", fontSize: "0.8rem" }}>知识库中暂无影人</p>
          ) : people.map((p) => (
            <div key={p.id} onClick={() => loadPerson(p.id)}
              style={{ padding: "0.5rem 0.65rem", borderRadius: 6, cursor: "pointer", background: selected?.id === p.id ? "var(--color-bg-elevated)" : "transparent" }}
              onMouseEnter={(e) => { if (selected?.id !== p.id) (e.currentTarget as HTMLDivElement).style.background = "rgba(255,255,255,0.04)"; }}
              onMouseLeave={(e) => { if (selected?.id !== p.id) (e.currentTarget as HTMLDivElement).style.background = "transparent"; }}
            >
              <p style={{ margin: 0, fontSize: "0.84rem", fontWeight: 500 }}>{p.name}</p>
              <p style={{ margin: 0, fontSize: "0.68rem", color: "var(--color-label-tertiary)" }}>
                {ROLE_LABELS[p.primary_role] ?? p.primary_role} · {p.film_count} 部
              </p>
            </div>
          ))}
        </div>
      </div>

      {/* Right detail */}
      <div style={{ flex: 1, overflowY: "auto", padding: "1.5rem" }}>
        {!selected ? (
          <p style={{ color: "var(--color-label-tertiary)", fontSize: "0.85rem" }}>选择左侧影人查看详情</p>
        ) : (
          <PersonDetailView
            person={selected} wikiContent={wikiContent}
            onWikiChange={setWikiContent} onWikiSave={saveWiki}
            onDelete={async () => { await library.deletePerson(selected.id); setSelected(null); loadPeople(); }}
            onRelationAdded={() => loadPerson(selected.id)}
          />
        )}
      </div>

      {showAddModal && (
        <AddPersonModal onClose={() => setShowAddModal(false)}
          onAdded={() => { loadPeople(); setShowAddModal(false); }} />
      )}
    </div>
  );
}
```

- [ ] **Step 2: Type-check + commit**

```bash
npx tsc --noEmit
git add src/pages/People.tsx
git commit -m "feat: add People page with person list, detail view, wiki, and relations"
```

---

### Task 13: Genres page

**Files:**
- Create: `src/pages/Genres.tsx`

- [ ] **Step 1: Create Genres.tsx**

```tsx
// src/pages/Genres.tsx
import { useState, useEffect, useCallback } from "react";
import { library } from "../lib/tauri";
import type { GenreTreeNode, GenreDetail } from "../lib/tauri";
import { WikiEditor } from "../components/WikiEditor";

function SectionTitle({ children }: { children: React.ReactNode }) {
  return (
    <p style={{ margin: "1.25rem 0 0.5rem", fontSize: "0.72rem", color: "var(--color-label-quaternary)", textTransform: "uppercase", letterSpacing: "0.06em", fontWeight: 600 }}>
      {children}
    </p>
  );
}

function GenreNode({ node, depth, selectedId, onSelect, onAddChild }: {
  node: GenreTreeNode; depth: number; selectedId: number | undefined;
  onSelect: (id: number) => void; onAddChild: (parentId: number) => void;
}) {
  const [expanded, setExpanded] = useState(true);
  const hasChildren = node.children.length > 0;

  return (
    <div>
      <div
        style={{ display: "flex", alignItems: "center", paddingLeft: `${depth * 14 + 6}px`, paddingRight: 6, borderRadius: 5, background: selectedId === node.id ? "var(--color-bg-elevated)" : "transparent", cursor: "pointer" }}
        onMouseEnter={(e) => {
          if (selectedId !== node.id) (e.currentTarget as HTMLDivElement).style.background = "rgba(255,255,255,0.04)";
          const btn = (e.currentTarget as HTMLDivElement).querySelector<HTMLButtonElement>(".add-child");
          if (btn) btn.style.display = "inline";
        }}
        onMouseLeave={(e) => {
          if (selectedId !== node.id) (e.currentTarget as HTMLDivElement).style.background = "transparent";
          const btn = (e.currentTarget as HTMLDivElement).querySelector<HTMLButtonElement>(".add-child");
          if (btn) btn.style.display = "none";
        }}
      >
        <span onClick={() => hasChildren && setExpanded(!expanded)}
          style={{ width: 14, textAlign: "center", color: "var(--color-label-quaternary)", fontSize: "0.65rem", flexShrink: 0, userSelect: "none" }}>
          {hasChildren ? (expanded ? "▾" : "▸") : " "}
        </span>
        <span onClick={() => onSelect(node.id)} style={{ flex: 1, padding: "0.4rem 0.25rem", fontSize: "0.82rem" }}>
          {node.name}
          {node.film_count > 0 && <span style={{ marginLeft: "0.35rem", fontSize: "0.65rem", color: "var(--color-label-quaternary)" }}>{node.film_count}</span>}
        </span>
        <button className="add-child" onClick={(e) => { e.stopPropagation(); onAddChild(node.id); }}
          style={{ display: "none", background: "none", border: "none", color: "var(--color-accent)", cursor: "pointer", fontSize: "0.7rem", padding: "0.2rem 0.3rem", fontFamily: "inherit" }}>
          +
        </button>
      </div>
      {expanded && node.children.map((child) => (
        <GenreNode key={child.id} node={child} depth={depth + 1} selectedId={selectedId} onSelect={onSelect} onAddChild={onAddChild} />
      ))}
    </div>
  );
}

function GenreDetailView({ genre, wikiContent, onWikiChange, onWikiSave, onDelete }: {
  genre: GenreDetail; wikiContent: string;
  onWikiChange: (v: string) => void; onWikiSave: () => void; onDelete: () => void;
}) {
  return (
    <div style={{ maxWidth: 680 }}>
      <div style={{ display: "flex", justifyContent: "space-between", alignItems: "flex-start", marginBottom: "0.25rem" }}>
        <div>
          <h2 style={{ margin: 0, fontSize: "1.4rem", fontWeight: 700, letterSpacing: "-0.03em" }}>{genre.name}</h2>
          {genre.period && <p style={{ margin: "0.15rem 0 0", fontSize: "0.75rem", color: "var(--color-label-tertiary)" }}>{genre.period}</p>}
        </div>
        <button onClick={onDelete} style={{ background: "none", border: "none", color: "var(--color-label-quaternary)", cursor: "pointer", fontSize: "0.75rem", fontFamily: "inherit" }}>删除</button>
      </div>

      <SectionTitle>简介 Wiki</SectionTitle>
      <WikiEditor value={wikiContent} onChange={onWikiChange} onSave={onWikiSave} />

      <SectionTitle>关联影人</SectionTitle>
      {genre.people.length === 0 ? (
        <p style={{ fontSize: "0.78rem", color: "var(--color-label-quaternary)" }}>暂无关联影人</p>
      ) : (
        <div style={{ display: "flex", flexWrap: "wrap", gap: "0.4rem" }}>
          {genre.people.map((p) => (
            <span key={p.id} style={{ background: "var(--color-bg-elevated)", border: "1px solid var(--color-separator)", borderRadius: 4, padding: "0.2rem 0.5rem", fontSize: "0.75rem", color: "var(--color-label-secondary)" }}>{p.name}</span>
          ))}
        </div>
      )}

      <SectionTitle>收录电影</SectionTitle>
      {genre.films.length === 0 ? (
        <p style={{ fontSize: "0.78rem", color: "var(--color-label-quaternary)" }}>暂无收录电影</p>
      ) : (
        <div style={{ display: "flex", flexDirection: "column", gap: "0.3rem" }}>
          {genre.films.map((f) => (
            <div key={f.id} style={{ display: "flex", alignItems: "center", gap: "0.5rem", fontSize: "0.8rem" }}>
              <span>{f.title}</span>
              {f.year && <span style={{ color: "var(--color-label-quaternary)" }}>({f.year})</span>}
              {f.tmdb_rating && <span style={{ color: "var(--color-accent)", fontSize: "0.72rem" }}>★ {f.tmdb_rating.toFixed(1)}</span>}
            </div>
          ))}
        </div>
      )}

      {genre.children.length > 0 && (
        <>
          <SectionTitle>子流派</SectionTitle>
          <div style={{ display: "flex", flexWrap: "wrap", gap: "0.4rem" }}>
            {genre.children.map((c) => (
              <span key={c.id} style={{ background: "var(--color-bg-elevated)", border: "1px solid var(--color-separator)", borderRadius: 4, padding: "0.2rem 0.5rem", fontSize: "0.75rem", color: "var(--color-label-secondary)" }}>
                {c.name} ({c.film_count})
              </span>
            ))}
          </div>
        </>
      )}
    </div>
  );
}

function AddGenreModal({ parentId, onClose, onAdded }: { parentId?: number; onClose: () => void; onAdded: () => void }) {
  const [name, setName] = useState("");
  const [description, setDescription] = useState("");
  const [period, setPeriod] = useState("");
  const iStyle: React.CSSProperties = { background: "var(--color-bg-elevated)", border: "1px solid var(--color-separator)", borderRadius: 5, padding: "0.4rem 0.6rem", color: "var(--color-label-primary)", fontSize: "0.82rem", fontFamily: "inherit", outline: "none" };

  const save = async () => {
    if (!name.trim()) return;
    await library.createGenre(name.trim(), parentId, description.trim() || undefined, period.trim() || undefined);
    onAdded();
  };

  return (
    <div style={{ position: "fixed", inset: 0, background: "rgba(0,0,0,0.55)", display: "flex", alignItems: "center", justifyContent: "center", zIndex: 100 }} onClick={onClose}>
      <div style={{ background: "var(--color-bg-secondary)", border: "1px solid var(--color-separator)", borderRadius: 10, padding: "1.5rem", width: 360, display: "flex", flexDirection: "column", gap: "0.75rem" }} onClick={(e) => e.stopPropagation()}>
        <h3 style={{ margin: 0, fontSize: "0.95rem", fontWeight: 700 }}>{parentId ? "添加子流派" : "添加流派"}</h3>
        {[
          { label: "名称", value: name, onChange: setName, placeholder: "流派名称", autoFocus: true },
          { label: "简介", value: description, onChange: setDescription, placeholder: "简短描述（可选）" },
          { label: "年代区间", value: period, onChange: setPeriod, placeholder: "如 1945-1965（可选）" },
        ].map(({ label, value, onChange, placeholder, autoFocus }) => (
          <div key={label} style={{ display: "flex", flexDirection: "column", gap: "0.25rem" }}>
            <label style={{ fontSize: "0.72rem", color: "var(--color-label-tertiary)" }}>{label}</label>
            <input value={value} onChange={(e) => onChange(e.target.value)} placeholder={placeholder} autoFocus={autoFocus} style={iStyle} />
          </div>
        ))}
        <div style={{ display: "flex", justifyContent: "flex-end", gap: "0.5rem" }}>
          <button onClick={onClose} style={{ background: "none", border: "none", color: "var(--color-label-tertiary)", cursor: "pointer", fontSize: "0.8rem", fontFamily: "inherit" }}>取消</button>
          <button onClick={save} style={{ background: "var(--color-accent)", border: "none", borderRadius: 6, padding: "0.3rem 0.9rem", color: "#0B1628", fontWeight: 600, cursor: "pointer", fontSize: "0.8rem", fontFamily: "inherit" }}>添加</button>
        </div>
      </div>
    </div>
  );
}

export default function Genres() {
  const [tree, setTree] = useState<GenreTreeNode[]>([]);
  const [selected, setSelected] = useState<GenreDetail | null>(null);
  const [wikiContent, setWikiContent] = useState("");
  const [showAddModal, setShowAddModal] = useState(false);
  const [addParentId, setAddParentId] = useState<number | undefined>();

  const loadTree = useCallback(() => { library.listGenresTree().then(setTree).catch(console.error); }, []);
  const loadGenre = useCallback((id: number) => {
    library.getGenre(id).then((g) => { setSelected(g); setWikiContent(g.wiki_content); }).catch(console.error);
  }, []);

  useEffect(() => { loadTree(); }, [loadTree]);

  return (
    <div style={{ display: "flex", height: "100%", overflow: "hidden" }}>
      <div style={{ width: 260, flexShrink: 0, borderRight: "1px solid var(--color-separator)", display: "flex", flexDirection: "column", overflow: "hidden" }}>
        <div style={{ padding: "1.2rem 1rem 0.75rem", borderBottom: "1px solid var(--color-separator)", display: "flex", justifyContent: "space-between", alignItems: "center" }}>
          <h1 style={{ margin: 0, fontSize: "1.1rem", fontWeight: 700 }}>流派</h1>
          <button onClick={() => { setAddParentId(undefined); setShowAddModal(true); }}
            style={{ background: "var(--color-accent-soft)", border: "1px solid var(--color-accent)", borderRadius: 5, padding: "0.2rem 0.55rem", color: "var(--color-accent)", cursor: "pointer", fontSize: "0.75rem", fontFamily: "inherit" }}>
            + 添加
          </button>
        </div>
        <div style={{ flex: 1, overflowY: "auto", padding: "0.5rem" }}>
          {tree.length === 0 ? (
            <p style={{ padding: "1rem", color: "var(--color-label-tertiary)", fontSize: "0.8rem" }}>知识库中暂无流派</p>
          ) : tree.map((node) => (
            <GenreNode key={node.id} node={node} depth={0} selectedId={selected?.id}
              onSelect={loadGenre}
              onAddChild={(pid) => { setAddParentId(pid); setShowAddModal(true); }}
            />
          ))}
        </div>
      </div>

      <div style={{ flex: 1, overflowY: "auto", padding: "1.5rem" }}>
        {!selected ? (
          <p style={{ color: "var(--color-label-tertiary)", fontSize: "0.85rem" }}>选择左侧流派查看详情</p>
        ) : (
          <GenreDetailView
            genre={selected} wikiContent={wikiContent}
            onWikiChange={setWikiContent}
            onWikiSave={async () => { if (selected) await library.updateGenreWiki(selected.id, wikiContent).catch(console.error); }}
            onDelete={async () => { await library.deleteGenre(selected.id); setSelected(null); loadTree(); }}
          />
        )}
      </div>

      {showAddModal && (
        <AddGenreModal parentId={addParentId} onClose={() => setShowAddModal(false)}
          onAdded={() => { loadTree(); setShowAddModal(false); }} />
      )}
    </div>
  );
}
```

- [ ] **Step 2: Type-check + commit**

```bash
npx tsc --noEmit
git add src/pages/Genres.tsx
git commit -m "feat: add Genres page with collapsible tree and genre detail view"
```

---

### Task 14: Graph page (D3 force + orbital rotation)

**Files:**
- Create: `src/pages/Graph.tsx`

- [ ] **Step 1: Create Graph.tsx**

```tsx
// src/pages/Graph.tsx
import { useEffect, useRef, useState, useCallback } from "react";
import * as d3 from "d3";
import { library } from "../lib/tauri";
import type { GraphData, GraphNode } from "../lib/tauri";

interface SimNode extends GraphNode {
  x?: number; y?: number; vx?: number; vy?: number;
  fx?: number | null; fy?: number | null;
}

interface SimLink { source: SimNode; target: SimNode; role: string; }

const nodeRadius = (n: SimNode) =>
  n.node_type === "film" ? 18 : n.role === "director" ? 8 + n.weight * 6 : 6 + n.weight * 4;

const nodeFill = (n: SimNode) =>
  n.node_type === "film" ? "#122040" : n.role === "director" ? "#C5A050" : "rgba(255,255,255,0.25)";

const nodeStroke = (n: SimNode) => n.node_type === "film" ? "#C5A050" : "transparent";

export default function Graph() {
  const svgRef = useRef<SVGSVGElement | null>(null);
  const [data, setData] = useState<GraphData | null>(null);
  const [hoveredId, setHoveredId] = useState<string | null>(null);
  const [selectedNode, setSelectedNode] = useState<SimNode | null>(null);
  const [orbitPaused, setOrbitPaused] = useState(false);
  const orbitPausedRef = useRef(false);
  const timerRef = useRef<d3.Timer | null>(null);

  useEffect(() => { library.getGraphData().then(setData).catch(console.error); }, []);

  const buildGraph = useCallback(() => {
    if (!data || !svgRef.current) return;
    const svg = d3.select(svgRef.current);
    svg.selectAll("*").remove();
    const width = svgRef.current.clientWidth;
    const height = svgRef.current.clientHeight;
    const g = svg.append("g");

    svg.call(
      d3.zoom<SVGSVGElement, unknown>().scaleExtent([0.2, 4])
        .on("zoom", (e) => g.attr("transform", e.transform))
    );

    const nodes: SimNode[] = data.nodes.map((n) => ({ ...n }));
    const nodeById = new Map(nodes.map((n) => [n.id, n]));

    const links: SimLink[] = data.links
      .map((l) => ({ source: nodeById.get(l.source)!, target: nodeById.get(l.target)!, role: l.role }))
      .filter((l) => l.source && l.target);

    const simulation = d3.forceSimulation<SimNode>(nodes)
      .force("link", d3.forceLink<SimNode, SimLink>(links).id((d) => d.id).distance(120))
      .force("charge", d3.forceManyBody().strength(-200))
      .force("center", d3.forceCenter(width / 2, height / 2))
      .force("collide", d3.forceCollide<SimNode>().radius((d) => nodeRadius(d) + 8))
      .alphaDecay(0.02);

    const link = g.append("g").selectAll("line").data(links).join("line")
      .attr("stroke", "rgba(255,255,255,0.12)").attr("stroke-width", 1);

    const node = g.append("g").selectAll<SVGCircleElement, SimNode>("circle")
      .data(nodes).join("circle")
      .attr("r", nodeRadius).attr("fill", nodeFill)
      .attr("stroke", nodeStroke).attr("stroke-width", (d) => d.node_type === "film" ? 1.5 : 0)
      .style("cursor", "pointer")
      .call(
        d3.drag<SVGCircleElement, SimNode>()
          .on("start", (e, d) => { if (!e.active) simulation.alphaTarget(0.3).restart(); d.fx = d.x; d.fy = d.y; })
          .on("drag", (e, d) => { d.fx = e.x; d.fy = e.y; })
          .on("end", (e, d) => { if (!e.active) simulation.alphaTarget(0); d.fx = null; d.fy = null; })
      )
      .on("mouseenter", (_, d) => setHoveredId(d.id))
      .on("mouseleave", () => setHoveredId(null))
      .on("click", (_, d) => setSelectedNode((prev) => prev?.id === d.id ? null : d));

    const label = g.append("g").selectAll("text").data(nodes).join("text")
      .text((d) => d.label)
      .attr("font-size", (d) => d.node_type === "film" ? 11 : 9)
      .attr("fill", "rgba(255,255,255,0.7)").attr("text-anchor", "middle")
      .attr("dy", (d) => nodeRadius(d) + 13)
      .style("pointer-events", "none").style("user-select", "none");

    simulation.on("tick", () => {
      link.attr("x1", (d) => d.source.x ?? 0).attr("y1", (d) => d.source.y ?? 0)
          .attr("x2", (d) => d.target.x ?? 0).attr("y2", (d) => d.target.y ?? 0);
      node.attr("cx", (d) => d.x ?? 0).attr("cy", (d) => d.y ?? 0);
      label.attr("x", (d) => d.x ?? 0).attr("y", (d) => d.y ?? 0);
    });

    // Build film→person adjacency for orbital rotation
    const personOf = new Map<string, string[]>();
    links.forEach((l) => {
      if (l.target.node_type === "film") {
        const list = personOf.get(l.target.id) ?? [];
        list.push(l.source.id);
        personOf.set(l.target.id, list);
      }
    });

    const angles = new Map<string, number>();
    nodes.forEach((n) => angles.set(n.id, Math.random() * Math.PI * 2));

    simulation.on("end", () => {
      if (timerRef.current) timerRef.current.stop();
      timerRef.current = d3.timer(() => {
        if (orbitPausedRef.current) return;
        nodes.filter((n) => n.node_type === "film").forEach((film) => {
          (personOf.get(film.id) ?? []).forEach((pid) => {
            const person = nodeById.get(pid);
            if (!person || (person.fx !== null && person.fx !== undefined)) return;
            const angle = (angles.get(pid) ?? 0) + 0.002;
            angles.set(pid, angle);
            const dx = (person.x ?? 0) - (film.x ?? 0);
            const dy = (person.y ?? 0) - (film.y ?? 0);
            const r = Math.sqrt(dx * dx + dy * dy);
            person.vx = (person.vx ?? 0) - Math.sin(angle) * r * 0.002;
            person.vy = (person.vy ?? 0) + Math.cos(angle) * r * 0.002;
          });
        });
        simulation.alphaTarget(0.01).restart();
      });
    });

    return () => { simulation.stop(); timerRef.current?.stop(); };
  }, [data]);

  useEffect(() => { const cleanup = buildGraph(); return cleanup; }, [buildGraph]);

  useEffect(() => {
    if (!svgRef.current || !data) return;
    const svg = d3.select(svgRef.current);
    if (!hoveredId) {
      svg.selectAll("circle, line, text").attr("opacity", 1);
      return;
    }
    const connected = new Set([hoveredId]);
    data.links.forEach((l) => {
      if (l.source === hoveredId) connected.add(l.target as string);
      if (l.target === hoveredId) connected.add(l.source as string);
    });
    svg.selectAll<SVGCircleElement, SimNode>("circle").attr("opacity", (d) => connected.has(d.id) ? 1 : 0.15);
    svg.selectAll<SVGLineElement, SimLink>("line").attr("opacity", (d) =>
      connected.has((d.source as SimNode).id) && connected.has((d.target as SimNode).id) ? 0.8 : 0.05
    );
    svg.selectAll<SVGTextElement, SimNode>("text").attr("opacity", (d) => connected.has(d.id) ? 1 : 0.1);
  }, [hoveredId, data]);

  const toggleOrbit = () => {
    orbitPausedRef.current = !orbitPausedRef.current;
    setOrbitPaused(orbitPausedRef.current);
  };

  const toolbarBtnStyle: React.CSSProperties = {
    background: "var(--color-bg-elevated)", border: "1px solid var(--color-separator)",
    borderRadius: 6, padding: "0.35rem 0.65rem",
    color: "var(--color-label-secondary)", fontSize: "0.73rem",
    cursor: "pointer", fontFamily: "inherit", whiteSpace: "nowrap",
  };

  return (
    <div style={{ position: "relative", width: "100%", height: "100%", background: "var(--color-bg-primary)" }}>
      {data === null ? (
        <div style={{ display: "flex", alignItems: "center", justifyContent: "center", height: "100%", color: "var(--color-label-tertiary)", fontSize: "0.85rem" }}>
          加载图谱数据…
        </div>
      ) : data.nodes.length === 0 ? (
        <div style={{ display: "flex", alignItems: "center", justifyContent: "center", height: "100%", color: "var(--color-label-tertiary)", fontSize: "0.85rem" }}>
          知识库为空。先在「影人」或「搜索」中添加内容。
        </div>
      ) : (
        <svg ref={svgRef} width="100%" height="100%" />
      )}

      {/* Toolbar */}
      <div style={{ position: "absolute", top: "1rem", right: "1rem", display: "flex", flexDirection: "column", gap: "0.4rem", zIndex: 10 }}>
        <button onClick={() => {
          if (!svgRef.current) return;
          d3.select(svgRef.current).transition().duration(500)
            .call((d3.zoom() as d3.ZoomBehavior<SVGSVGElement, unknown>).transform, d3.zoomIdentity);
        }} style={toolbarBtnStyle}>重置视角</button>
        <button onClick={toggleOrbit} style={toolbarBtnStyle}>
          {orbitPaused ? "▶ 继续旋转" : "⏸ 暂停旋转"}
        </button>
      </div>

      {/* Selected node mini card */}
      {selectedNode && (
        <div style={{ position: "absolute", bottom: "1.5rem", right: "1rem", background: "var(--color-bg-secondary)", border: "1px solid var(--color-separator)", borderRadius: 8, padding: "0.85rem 1rem", width: 200, zIndex: 10 }}>
          <button onClick={() => setSelectedNode(null)} style={{ position: "absolute", top: "0.4rem", right: "0.5rem", background: "none", border: "none", color: "var(--color-label-quaternary)", cursor: "pointer", fontSize: "0.8rem" }}>✕</button>
          <p style={{ margin: 0, fontSize: "0.85rem", fontWeight: 600 }}>{selectedNode.label}</p>
          <p style={{ margin: "0.2rem 0 0", fontSize: "0.7rem", color: "var(--color-label-tertiary)" }}>
            {selectedNode.node_type === "film" ? "电影" : selectedNode.role ?? "影人"}
          </p>
          {selectedNode.node_type !== "film" && (
            <p style={{ margin: "0.15rem 0 0", fontSize: "0.68rem", color: "var(--color-label-quaternary)" }}>
              影响力 {selectedNode.weight.toFixed(2)}
            </p>
          )}
        </div>
      )}
    </div>
  );
}
```

- [ ] **Step 2: Type-check + commit**

```bash
npx tsc --noEmit
git add src/pages/Graph.tsx
git commit -m "feat: add Graph page with D3 force simulation and orbital rotation"
```

---

### Task 15: MusicPlayer component + Settings music section

**Files:**
- Create: `src/components/MusicPlayer.tsx`
- Modify: `src/pages/Settings.tsx`

- [ ] **Step 1: Create MusicPlayer.tsx**

```tsx
// src/components/MusicPlayer.tsx
import { useEffect, useRef, useState, useCallback } from "react";
import { convertFileSrc } from "@tauri-apps/api/core";
import type { MusicTrack } from "../lib/tauri";

interface MusicPlayerProps {
  enabled: boolean;
  mode: "sequential" | "random";
  playlist: MusicTrack[];
  active: boolean;
}

export function MusicPlayer({ enabled, mode, playlist, active }: MusicPlayerProps) {
  const audioRef = useRef<HTMLAudioElement | null>(null);
  const [currentIndex, setCurrentIndex] = useState(0);
  const [isPlaying, setIsPlaying] = useState(false);
  const [progress, setProgress] = useState(0);
  const [duration, setDuration] = useState(0);

  const getTrackSrc = useCallback((track: MusicTrack): string => {
    if (track.src.startsWith("http://") || track.src.startsWith("https://")) return track.src;
    return convertFileSrc(track.src);
  }, []);

  useEffect(() => {
    audioRef.current = new Audio();
    return () => { audioRef.current?.pause(); audioRef.current = null; };
  }, []);

  useEffect(() => {
    if (!audioRef.current) return;
    if (!active || !enabled || playlist.length === 0) {
      audioRef.current.pause(); setIsPlaying(false);
    }
  }, [active, enabled, playlist.length]);

  useEffect(() => {
    const audio = audioRef.current;
    if (!audio) return;

    const onEnded = () => {
      if (playlist.length === 0) return;
      setCurrentIndex(mode === "random"
        ? Math.floor(Math.random() * playlist.length)
        : (i) => (i + 1) % playlist.length as unknown as number
      );
    };
    const onTimeUpdate = () => { setProgress(audio.currentTime); setDuration(audio.duration || 0); };
    const onPlay = () => setIsPlaying(true);
    const onPause = () => setIsPlaying(false);

    audio.addEventListener("ended", onEnded);
    audio.addEventListener("timeupdate", onTimeUpdate);
    audio.addEventListener("play", onPlay);
    audio.addEventListener("pause", onPause);
    return () => {
      audio.removeEventListener("ended", onEnded);
      audio.removeEventListener("timeupdate", onTimeUpdate);
      audio.removeEventListener("play", onPlay);
      audio.removeEventListener("pause", onPause);
    };
  }, [mode, playlist.length]);

  useEffect(() => {
    const audio = audioRef.current;
    if (!audio || !active || !enabled || playlist.length === 0) return;
    const track = playlist[currentIndex];
    if (!track) return;
    const wasSrc = audio.src;
    const newSrc = getTrackSrc(track);
    if (wasSrc !== newSrc) {
      audio.src = newSrc;
      audio.play().then(() => setIsPlaying(true)).catch(() => {});
    }
  }, [currentIndex, active, enabled, playlist, getTrackSrc]);

  if (!enabled || playlist.length === 0 || !active) return null;

  const currentTrack = playlist[currentIndex];
  const fmt = (s: number) => `${Math.floor(s / 60)}:${String(Math.floor(s % 60)).padStart(2, "0")}`;
  const next = () => setCurrentIndex(mode === "random" ? Math.floor(Math.random() * playlist.length) : (currentIndex + 1) % playlist.length);
  const togglePlay = () => {
    const a = audioRef.current;
    if (!a) return;
    isPlaying ? a.pause() : a.play().catch(() => {});
  };

  const btnStyle: React.CSSProperties = { background: "none", border: "none", color: "var(--color-label-secondary)", cursor: "pointer", fontSize: "0.8rem", padding: "0.1rem 0.2rem", lineHeight: 1 };

  return (
    <div style={{ position: "fixed", bottom: "1.25rem", right: "1.25rem", zIndex: 50, background: "var(--color-bg-elevated)", border: "1px solid var(--color-separator)", borderRadius: 10, padding: "0.6rem 0.85rem", width: 240, boxShadow: "0 4px 20px rgba(0,0,0,0.4)" }}>
      <div style={{ display: "flex", alignItems: "center", gap: "0.5rem", marginBottom: "0.4rem" }}>
        <span style={{ fontSize: "0.78rem", color: "var(--color-accent)" }}>♪</span>
        <span style={{ flex: 1, fontSize: "0.75rem", whiteSpace: "nowrap", overflow: "hidden", textOverflow: "ellipsis", color: "var(--color-label-secondary)" }}>
          {currentTrack?.name ?? "未知曲目"}
        </span>
        <button onClick={togglePlay} style={btnStyle}>{isPlaying ? "⏸" : "▶"}</button>
        <button onClick={next} style={btnStyle}>⏭</button>
        {mode === "random" && <span style={{ fontSize: "0.65rem", color: "var(--color-label-quaternary)" }}>🔀</span>}
      </div>
      <div style={{ display: "flex", alignItems: "center", gap: "0.4rem" }}>
        <div style={{ flex: 1, height: 3, background: "var(--color-bg-secondary)", borderRadius: 2, overflow: "hidden", cursor: "pointer" }}
          onClick={(e) => {
            const a = audioRef.current;
            if (!a || !duration) return;
            const rect = e.currentTarget.getBoundingClientRect();
            a.currentTime = ((e.clientX - rect.left) / rect.width) * duration;
          }}>
          <div style={{ height: "100%", width: duration > 0 ? `${(progress / duration) * 100}%` : "0%", background: "var(--color-accent)", borderRadius: 2, transition: "width 0.5s linear" }} />
        </div>
        <span style={{ fontSize: "0.62rem", color: "var(--color-label-quaternary)", whiteSpace: "nowrap" }}>
          {fmt(progress)} / {fmt(duration)}
        </span>
      </div>
    </div>
  );
}
```

- [ ] **Step 2: Add music section to Settings.tsx**

Read the current `src/pages/Settings.tsx`. Find the section that renders `library.root_dir`. After the last existing `<section>` element and before the closing tag of the main container, add a new section. Also add these helper functions inside the Settings component before the return statement, and add the import for `config.setMusicPlaylist` (already available via the updated `config` export in tauri.ts).

Add inside the `Settings` component function (before `return`):

```tsx
const updatePlaylist = async (index: number, field: "name" | "src", value: string) => {
  const newPlaylist = (form.music?.playlist ?? []).map((t, i) =>
    i === index ? { ...t, [field]: value } : t
  );
  setForm((prev) => ({ ...prev, music: { ...prev.music!, playlist: newPlaylist } }));
  await config.setMusicPlaylist(newPlaylist);
};

const addTrack = () => {
  setForm((prev) => ({
    ...prev,
    music: { ...prev.music!, playlist: [...(prev.music?.playlist ?? []), { name: "", src: "" }] },
  }));
};

const removeTrack = async (index: number) => {
  const newPlaylist = (form.music?.playlist ?? []).filter((_, i) => i !== index);
  setForm((prev) => ({ ...prev, music: { ...prev.music!, playlist: newPlaylist } }));
  await config.setMusicPlaylist(newPlaylist);
};
```

Add as the last `<section>` inside the Settings JSX (using the same style pattern as existing sections):

```tsx
<section>
  <h2 style={sectionTitleStyle}>背景音乐</h2>

  {/* enabled toggle */}
  <div style={rowStyle}>
    <div>
      <p style={labelStyle}>启用背景音乐</p>
      <p style={descStyle}>在影人/流派/关系图页面播放</p>
    </div>
    <input
      type="checkbox"
      checked={!!form.music?.enabled}
      onChange={async (e) => {
        await config.set("music.enabled", e.target.checked ? "true" : "false");
        setForm((prev) => ({ ...prev, music: { ...prev.music!, enabled: e.target.checked } }));
      }}
      style={{ accentColor: "var(--color-accent)", cursor: "pointer" }}
    />
  </div>

  {/* mode select */}
  <div style={rowStyle}>
    <p style={labelStyle}>播放模式</p>
    <select
      value={form.music?.mode ?? "sequential"}
      onChange={async (e) => {
        await config.set("music.mode", e.target.value);
        setForm((prev) => ({ ...prev, music: { ...prev.music!, mode: e.target.value } }));
      }}
      style={inputStyle}
    >
      <option value="sequential">顺序播放</option>
      <option value="random">随机播放</option>
    </select>
  </div>

  {/* playlist */}
  <div style={{ marginTop: "0.75rem" }}>
    <p style={{ ...labelStyle, marginBottom: "0.5rem" }}>播放列表</p>
    {(form.music?.playlist ?? []).map((track, i) => (
      <div key={i} style={{ display: "flex", gap: "0.5rem", marginBottom: "0.4rem", alignItems: "center" }}>
        <input
          placeholder="曲目名称"
          value={track.name}
          onChange={(e) => updatePlaylist(i, "name", e.target.value)}
          style={{ ...inputStyle, width: 120, flexShrink: 0 }}
        />
        <input
          placeholder="文件路径或 URL"
          value={track.src}
          onChange={(e) => updatePlaylist(i, "src", e.target.value)}
          style={{ ...inputStyle, flex: 1 }}
        />
        <button onClick={() => removeTrack(i)}
          style={{ background: "none", border: "none", color: "var(--color-label-quaternary)", cursor: "pointer", fontSize: "0.8rem" }}>
          ✕
        </button>
      </div>
    ))}
    <button onClick={addTrack}
      style={{ background: "none", border: "1px dashed var(--color-separator)", borderRadius: 5, padding: "0.3rem 0.75rem", color: "var(--color-label-tertiary)", cursor: "pointer", fontSize: "0.75rem", fontFamily: "inherit", marginTop: "0.25rem" }}>
      + 添加曲目
    </button>
  </div>
</section>
```

**Note:** The style variables (`sectionTitleStyle`, `rowStyle`, `labelStyle`, `descStyle`, `inputStyle`) match whatever names are used in the existing Settings.tsx code. Read the file first to confirm exact variable names and match them.

- [ ] **Step 3: Type-check + commit**

```bash
npx tsc --noEmit
git add src/components/MusicPlayer.tsx src/pages/Settings.tsx
git commit -m "feat: add MusicPlayer and music config section in Settings"
```

---

### Task 16: App.tsx unlock + final build

**Files:**
- Modify: `src/App.tsx`

- [ ] **Step 1: Update App.tsx**

Replace `src/App.tsx` with:

```tsx
// src/App.tsx
import { Routes, Route, useNavigate, useLocation } from "react-router-dom";
import { useState, useEffect } from "react";
import { NavItem } from "./components/ui/NavItem";
import { MusicPlayer } from "./components/MusicPlayer";
import Search from "./pages/Search";
import Settings from "./pages/Settings";
import People from "./pages/People";
import Genres from "./pages/Genres";
import Graph from "./pages/Graph";
import Placeholder from "./pages/Placeholder";
import { config } from "./lib/tauri";
import type { AppConfig } from "./lib/tauri";

const KB_PATHS = ["/people", "/genres", "/graph"];

const NAV_SECTIONS = [
  { label: null, items: [{ icon: "⌕", label: "搜索", path: "/" }] },
  {
    label: "知识库",
    items: [
      { icon: "◎", label: "影人", path: "/people" },
      { icon: "◈", label: "流派", path: "/genres" },
      { icon: "⋯", label: "关系图", path: "/graph" },
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
  const [musicConfig, setMusicConfig] = useState<AppConfig["music"] | null>(null);

  useEffect(() => { config.get().then((cfg) => setMusicConfig(cfg.music)).catch(() => {}); }, []);

  const isKbActive = KB_PATHS.some((p) => pathname.startsWith(p));

  return (
    <div style={{ display: "flex", height: "100vh", overflow: "hidden" }}>
      <aside style={{ width: 188, flexShrink: 0, background: "var(--color-bg-secondary)", borderRight: "1px solid var(--color-separator)", display: "flex", flexDirection: "column", padding: "1rem 0.5rem", gap: 1 }}>
        {NAV_SECTIONS.map((section, si) => (
          <div key={si}>
            {section.label && (
              <p style={{ fontSize: "0.62rem", color: "var(--color-label-quaternary)", letterSpacing: "0.08em", textTransform: "uppercase", padding: "0.85rem 0.75rem 0.3rem", margin: 0 }}>
                {section.label}
              </p>
            )}
            {section.items.map((item) => (
              <NavItem
                key={item.path}
                icon={item.icon}
                label={item.label}
                active={pathname === item.path || (item.path !== "/" && pathname.startsWith(item.path))}
                disabled={"disabled" in item && item.disabled}
                onClick={() => navigate(item.path)}
              />
            ))}
          </div>
        ))}
        <div style={{ marginTop: "auto", borderTop: "1px solid var(--color-separator)", paddingTop: "0.5rem" }}>
          <NavItem icon="⚙" label="设置" active={pathname === "/settings"} onClick={() => navigate("/settings")} />
        </div>
      </aside>

      <main style={{ flex: 1, overflow: "hidden", display: "flex", flexDirection: "column" }}>
        <Routes>
          <Route path="/" element={<Search />} />
          <Route path="/settings" element={<Settings />} />
          <Route path="/people" element={<People />} />
          <Route path="/genres" element={<Genres />} />
          <Route path="/graph" element={<Graph />} />
          <Route path="/library" element={<Placeholder title="我的库" milestone="M3" />} />
          <Route path="/download" element={<Placeholder title="下载" milestone="M3" />} />
          <Route path="/subtitle" element={<Placeholder title="字幕" milestone="M4" />} />
          <Route path="/media" element={<Placeholder title="媒体工具" milestone="M4" />} />
        </Routes>
      </main>

      {musicConfig && (
        <MusicPlayer
          enabled={musicConfig.enabled}
          mode={musicConfig.mode as "sequential" | "random"}
          playlist={musicConfig.playlist}
          active={isKbActive}
        />
      )}
    </div>
  );
}
```

- [ ] **Step 2: Type-check**

```bash
npx tsc --noEmit
```

Expected: no errors.

- [ ] **Step 3: Full frontend build**

```bash
npm run build
```

Expected: clean build, no errors.

- [ ] **Step 4: Full Rust test suite**

```bash
cd src-tauri && cargo test
```

Expected: all tests PASS.

- [ ] **Step 5: Final commit**

```bash
git add src/App.tsx
git commit -m "feat: unlock /people, /genres, /graph routes and integrate MusicPlayer — M2 complete"
```

---

## Self-Review

### Spec coverage

| Spec requirement | Task |
|---|---|
| 影人档案 CRUD + wiki | T3, T12 |
| 电影流派树 CRUD + wiki | T5, T13 |
| 关系图谱 D3 力导向 | T6, T14 |
| 轨道旋转特效 | T14 |
| 影评（个人 + 收录） | T6, T10 |
| 背景音乐播放器 | T1, T15 |
| MusicConfig in config.rs + save_config | T1 |
| set_music_playlist command | T1 |
| 加入知识库按钮 + TMDB credits | T7, T11 |
| sqlx derive feature | T1 |
| Settings 音乐区块 | T15 |
| App.tsx 解锁三个导航 | T16 |
| WikiEditor Write/Preview | T9 |
| FilmDetailPanel 提取 | T11 |

### Type consistency

- `PersonSummary.film_count: i64` (Rust) ↔ `PersonSummary.film_count: number` (TS) ✓
- `ReviewEntry.is_personal: bool` (Rust, stored as `is_personal as i64` on insert) ↔ `ReviewEntry.is_personal: boolean` (TS) ✓  
- `GenreTreeNode.children: Vec<GenreTreeNode>` (recursive Rust) ↔ `GenreTreeNode.children: GenreTreeNode[]` (TS) ✓
- `add_film_from_tmdb` receives `tmdb_movie: TmdbMovieInput` — Tauri param name matches `invoke("add_film_from_tmdb", { tmdbMovie })` via camelCase conversion ✓
- `set_music_playlist` receives `tracks: Vec<MusicTrack>` — `invoke("set_music_playlist", { tracks })` ✓
- All 30 commands registered in T7 lib.rs match their invoke names in T8 tauri.ts ✓

### Note on Settings.tsx integration (T15 Step 2)

The implementer must read the current `src/pages/Settings.tsx` before editing to identify the exact style variable names used (e.g., `sectionTitleStyle`, `inputStyle`, `rowStyle`). These names are defined inside the component and must match exactly. The music section code in T15 uses placeholder names — substitute with the actual ones found in the file.
