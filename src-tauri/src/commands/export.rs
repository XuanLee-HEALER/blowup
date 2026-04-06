use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;

// ── Export types ─────────────────────────────────────────────────

#[derive(Serialize, Deserialize)]
pub struct KnowledgeBaseExport {
    pub version: String,
    pub exported_at: String,
    pub people: Vec<PersonRow>,
    pub genres: Vec<GenreRow>,
    pub films: Vec<FilmRow>,
    pub person_films: Vec<PersonFilmRow>,
    pub film_genres: Vec<FilmGenreRow>,
    pub person_genres: Vec<PersonGenreRow>,
    pub person_relations: Vec<PersonRelationRow>,
    pub wiki_entries: Vec<WikiEntryRow>,
    pub reviews: Vec<ReviewRow>,
}

#[derive(Serialize, Deserialize, sqlx::FromRow)]
pub struct PersonRow {
    pub id: i64,
    pub tmdb_id: Option<i64>,
    pub name: String,
    pub born_date: Option<String>,
    pub biography: Option<String>,
    pub nationality: Option<String>,
    pub primary_role: String,
}

#[derive(Serialize, Deserialize, sqlx::FromRow)]
pub struct GenreRow {
    pub id: i64,
    pub name: String,
    pub description: Option<String>,
    pub parent_id: Option<i64>,
    pub period: Option<String>,
}

#[derive(Serialize, Deserialize, sqlx::FromRow)]
pub struct FilmRow {
    pub id: i64,
    pub tmdb_id: Option<i64>,
    pub title: String,
    pub original_title: Option<String>,
    pub year: Option<i64>,
    pub overview: Option<String>,
    pub tmdb_rating: Option<f64>,
    pub poster_cache_path: Option<String>,
}

#[derive(Serialize, Deserialize, sqlx::FromRow)]
pub struct PersonFilmRow {
    pub person_id: i64,
    pub film_id: i64,
    pub role: String,
}

#[derive(Serialize, Deserialize, sqlx::FromRow)]
pub struct FilmGenreRow {
    pub film_id: i64,
    pub genre_id: i64,
}

#[derive(Serialize, Deserialize, sqlx::FromRow)]
pub struct PersonGenreRow {
    pub person_id: i64,
    pub genre_id: i64,
}

#[derive(Serialize, Deserialize, sqlx::FromRow)]
pub struct PersonRelationRow {
    pub from_id: i64,
    pub to_id: i64,
    pub relation_type: String,
}

#[derive(Serialize, Deserialize, sqlx::FromRow)]
pub struct WikiEntryRow {
    pub id: i64,
    pub entity_type: String,
    pub entity_id: i64,
    pub content: String,
}

#[derive(Serialize, Deserialize, sqlx::FromRow)]
pub struct ReviewRow {
    pub id: i64,
    pub film_id: i64,
    pub is_personal: i64,
    pub author: Option<String>,
    pub content: String,
    pub rating: Option<f64>,
}

// ── Export command ────────────────────────────────────────────────

#[tauri::command]
pub async fn export_knowledge_base(
    path: String,
    pool: tauri::State<'_, SqlitePool>,
) -> Result<(), String> {
    let people = sqlx::query_as::<_, PersonRow>(
        "SELECT id, tmdb_id, name, born_date, biography, nationality, primary_role FROM people",
    )
    .fetch_all(pool.inner())
    .await
    .map_err(|e| e.to_string())?;

    let genres = sqlx::query_as::<_, GenreRow>(
        "SELECT id, name, description, parent_id, period FROM genres",
    )
    .fetch_all(pool.inner())
    .await
    .map_err(|e| e.to_string())?;

    let films = sqlx::query_as::<_, FilmRow>(
        "SELECT id, tmdb_id, title, original_title, year, overview, tmdb_rating, poster_cache_path FROM films",
    )
    .fetch_all(pool.inner())
    .await
    .map_err(|e| e.to_string())?;

    let person_films =
        sqlx::query_as::<_, PersonFilmRow>("SELECT person_id, film_id, role FROM person_films")
            .fetch_all(pool.inner())
            .await
            .map_err(|e| e.to_string())?;

    let film_genres =
        sqlx::query_as::<_, FilmGenreRow>("SELECT film_id, genre_id FROM film_genres")
            .fetch_all(pool.inner())
            .await
            .map_err(|e| e.to_string())?;

    let person_genres =
        sqlx::query_as::<_, PersonGenreRow>("SELECT person_id, genre_id FROM person_genres")
            .fetch_all(pool.inner())
            .await
            .map_err(|e| e.to_string())?;

    let person_relations = sqlx::query_as::<_, PersonRelationRow>(
        "SELECT from_id, to_id, relation_type FROM person_relations",
    )
    .fetch_all(pool.inner())
    .await
    .map_err(|e| e.to_string())?;

    let wiki_entries = sqlx::query_as::<_, WikiEntryRow>(
        "SELECT id, entity_type, entity_id, content FROM wiki_entries",
    )
    .fetch_all(pool.inner())
    .await
    .map_err(|e| e.to_string())?;

    let reviews = sqlx::query_as::<_, ReviewRow>(
        "SELECT id, film_id, is_personal, author, content, rating FROM reviews",
    )
    .fetch_all(pool.inner())
    .await
    .map_err(|e| e.to_string())?;

    let export = KnowledgeBaseExport {
        version: "2.0.0".to_string(),
        exported_at: chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string(),
        people,
        genres,
        films,
        person_films,
        film_genres,
        person_genres,
        person_relations,
        wiki_entries,
        reviews,
    };

    let json = serde_json::to_string_pretty(&export).map_err(|e| e.to_string())?;
    std::fs::write(&path, json).map_err(|e| e.to_string())?;

    Ok(())
}

// ── Import command ───────────────────────────────────────────────

#[tauri::command]
pub async fn import_knowledge_base(
    path: String,
    pool: tauri::State<'_, SqlitePool>,
) -> Result<String, String> {
    let json = std::fs::read_to_string(&path).map_err(|e| e.to_string())?;
    let data: KnowledgeBaseExport =
        serde_json::from_str(&json).map_err(|e| format!("JSON 解析失败: {}", e))?;

    let mut imported = ImportStats::default();

    // Insert in dependency order: people, genres, films → junctions → wiki, reviews
    for p in &data.people {
        let exists = sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM people WHERE id = ?")
            .bind(p.id)
            .fetch_one(pool.inner())
            .await
            .unwrap_or(0);
        if exists > 0 {
            continue;
        }
        sqlx::query(
            "INSERT INTO people (id, tmdb_id, name, born_date, biography, nationality, primary_role) VALUES (?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(p.id)
        .bind(p.tmdb_id)
        .bind(&p.name)
        .bind(&p.born_date)
        .bind(&p.biography)
        .bind(&p.nationality)
        .bind(&p.primary_role)
        .execute(pool.inner())
        .await
        .map_err(|e| e.to_string())?;
        imported.people += 1;
    }

    for g in &data.genres {
        let exists = sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM genres WHERE id = ?")
            .bind(g.id)
            .fetch_one(pool.inner())
            .await
            .unwrap_or(0);
        if exists > 0 {
            continue;
        }
        sqlx::query(
            "INSERT INTO genres (id, name, description, parent_id, period) VALUES (?, ?, ?, ?, ?)",
        )
        .bind(g.id)
        .bind(&g.name)
        .bind(&g.description)
        .bind(g.parent_id)
        .bind(&g.period)
        .execute(pool.inner())
        .await
        .map_err(|e| e.to_string())?;
        imported.genres += 1;
    }

    for f in &data.films {
        let exists = sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM films WHERE id = ?")
            .bind(f.id)
            .fetch_one(pool.inner())
            .await
            .unwrap_or(0);
        if exists > 0 {
            continue;
        }
        sqlx::query(
            "INSERT INTO films (id, tmdb_id, title, original_title, year, overview, tmdb_rating, poster_cache_path) VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(f.id)
        .bind(f.tmdb_id)
        .bind(&f.title)
        .bind(&f.original_title)
        .bind(f.year)
        .bind(&f.overview)
        .bind(f.tmdb_rating)
        .bind(&f.poster_cache_path)
        .execute(pool.inner())
        .await
        .map_err(|e| e.to_string())?;
        imported.films += 1;
    }

    // Junction tables (skip duplicates silently)
    for pf in &data.person_films {
        sqlx::query(
            "INSERT OR IGNORE INTO person_films (person_id, film_id, role) VALUES (?, ?, ?)",
        )
        .bind(pf.person_id)
        .bind(pf.film_id)
        .bind(&pf.role)
        .execute(pool.inner())
        .await
        .ok();
    }
    for fg in &data.film_genres {
        sqlx::query("INSERT OR IGNORE INTO film_genres (film_id, genre_id) VALUES (?, ?)")
            .bind(fg.film_id)
            .bind(fg.genre_id)
            .execute(pool.inner())
            .await
            .ok();
    }
    for pg in &data.person_genres {
        sqlx::query("INSERT OR IGNORE INTO person_genres (person_id, genre_id) VALUES (?, ?)")
            .bind(pg.person_id)
            .bind(pg.genre_id)
            .execute(pool.inner())
            .await
            .ok();
    }
    for pr in &data.person_relations {
        sqlx::query("INSERT OR IGNORE INTO person_relations (from_id, to_id, relation_type) VALUES (?, ?, ?)")
            .bind(pr.from_id).bind(pr.to_id).bind(&pr.relation_type)
            .execute(pool.inner()).await.ok();
    }

    // Wiki entries (upsert)
    for w in &data.wiki_entries {
        sqlx::query(
            "INSERT INTO wiki_entries (entity_type, entity_id, content, updated_at)
             VALUES (?, ?, ?, datetime('now'))
             ON CONFLICT(entity_type, entity_id)
             DO UPDATE SET content = excluded.content, updated_at = excluded.updated_at",
        )
        .bind(&w.entity_type)
        .bind(w.entity_id)
        .bind(&w.content)
        .execute(pool.inner())
        .await
        .ok();
        imported.wiki += 1;
    }

    // Reviews (skip by id)
    for r in &data.reviews {
        let exists = sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM reviews WHERE id = ?")
            .bind(r.id)
            .fetch_one(pool.inner())
            .await
            .unwrap_or(0);
        if exists > 0 {
            continue;
        }
        sqlx::query(
            "INSERT INTO reviews (id, film_id, is_personal, author, content, rating) VALUES (?, ?, ?, ?, ?, ?)",
        )
        .bind(r.id)
        .bind(r.film_id)
        .bind(r.is_personal)
        .bind(&r.author)
        .bind(&r.content)
        .bind(r.rating)
        .execute(pool.inner())
        .await
        .ok();
        imported.reviews += 1;
    }

    Ok(format!(
        "导入完成: {} 影人, {} 流派, {} 影片, {} 条 Wiki, {} 条评论",
        imported.people, imported.genres, imported.films, imported.wiki, imported.reviews
    ))
}

#[derive(Default)]
struct ImportStats {
    people: i64,
    genres: i64,
    films: i64,
    wiki: i64,
    reviews: i64,
}

// ── Config export/import ─────────────────────────────────────────

#[tauri::command]
pub fn export_config(path: String) -> Result<(), String> {
    let config_path = crate::config::config_path();
    if !config_path.exists() {
        // Write default config
        let cfg = crate::config::Config::default();
        crate::config::save_config(&cfg)?;
    }
    std::fs::copy(&config_path, &path).map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub fn import_config(path: String) -> Result<(), String> {
    let content = std::fs::read_to_string(&path).map_err(|e| e.to_string())?;
    // Validate it's valid TOML config
    let _: crate::config::Config =
        toml::from_str(&content).map_err(|e| format!("配置文件格式错误: {}", e))?;
    let config_path = crate::config::config_path();
    if let Some(parent) = config_path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    std::fs::write(&config_path, content).map_err(|e| e.to_string())?;
    Ok(())
}
