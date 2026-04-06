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
    .fetch_all(pool.inner()).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_film(id: i64, pool: tauri::State<'_, SqlitePool>) -> Result<FilmDetail, String> {
    let row = sqlx::query_as::<_, FilmRow>(
        "SELECT id, tmdb_id, title, original_title, year, overview, tmdb_rating, poster_cache_path
         FROM films WHERE id = ?",
    )
    .bind(id).fetch_one(pool.inner()).await.map_err(|e| e.to_string())?;

    let wiki_content: String = sqlx::query_scalar(
        "SELECT content FROM wiki_entries WHERE entity_type = 'film' AND entity_id = ?",
    )
    .bind(id).fetch_optional(pool.inner()).await.map_err(|e| e.to_string())?
    .unwrap_or_default();

    let people = sqlx::query_as::<_, FilmPersonEntry>(
        "SELECT p.id as person_id, p.name, pf.role
         FROM person_films pf JOIN people p ON p.id = pf.person_id
         WHERE pf.film_id = ? ORDER BY p.primary_role",
    )
    .bind(id).fetch_all(pool.inner()).await.map_err(|e| e.to_string())?;

    let genres = sqlx::query_as::<_, GenreSummary>(
        "SELECT g.id, g.name,
                (SELECT COUNT(*) FROM film_genres WHERE genre_id = g.id) as film_count,
                (SELECT COUNT(*) FROM genres WHERE parent_id = g.id) as child_count
         FROM film_genres fg JOIN genres g ON g.id = fg.genre_id
         WHERE fg.film_id = ?",
    )
    .bind(id).fetch_all(pool.inner()).await.map_err(|e| e.to_string())?;

    let reviews = sqlx::query_as::<_, ReviewEntry>(
        "SELECT id, is_personal, author, content, rating, created_at
         FROM reviews WHERE film_id = ? ORDER BY is_personal DESC, created_at DESC",
    )
    .bind(id).fetch_all(pool.inner()).await.map_err(|e| e.to_string())?;

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
    .bind(tmdb_movie.tmdb_id).fetch_optional(pool.inner()).await.map_err(|e| e.to_string())?
    {
        return Ok(existing_id);
    }

    let film_id = sqlx::query(
        "INSERT INTO films (tmdb_id, title, original_title, year, overview, tmdb_rating) VALUES (?, ?, ?, ?, ?, ?)",
    )
    .bind(tmdb_movie.tmdb_id).bind(&tmdb_movie.title).bind(&tmdb_movie.original_title)
    .bind(tmdb_movie.year).bind(&tmdb_movie.overview).bind(tmdb_movie.tmdb_rating)
    .execute(pool.inner()).await.map_err(|e| e.to_string())?.last_insert_rowid();

    for person in &tmdb_movie.people {
        let person_id: i64 = if let Some(tmdb_id) = person.tmdb_id {
            if let Some(existing) = sqlx::query_scalar::<_, i64>(
                "SELECT id FROM people WHERE tmdb_id = ?",
            )
            .bind(tmdb_id).fetch_optional(pool.inner()).await.map_err(|e| e.to_string())?
            {
                existing
            } else {
                sqlx::query("INSERT INTO people (name, primary_role, tmdb_id) VALUES (?, ?, ?)")
                    .bind(&person.name).bind(&person.primary_role).bind(tmdb_id)
                    .execute(pool.inner()).await.map_err(|e| e.to_string())?.last_insert_rowid()
            }
        } else {
            if let Some(existing) = sqlx::query_scalar::<_, i64>(
                "SELECT id FROM people WHERE name = ?",
            )
            .bind(&person.name).fetch_optional(pool.inner()).await.map_err(|e| e.to_string())?
            {
                existing
            } else {
                sqlx::query("INSERT INTO people (name, primary_role) VALUES (?, ?)")
                    .bind(&person.name).bind(&person.primary_role)
                    .execute(pool.inner()).await.map_err(|e| e.to_string())?.last_insert_rowid()
            }
        };

        sqlx::query(
            "INSERT OR IGNORE INTO person_films (person_id, film_id, role) VALUES (?, ?, ?)",
        )
        .bind(person_id).bind(film_id).bind(&person.role)
        .execute(pool.inner()).await.map_err(|e| e.to_string())?;
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
    .bind(id).bind(&content).execute(pool.inner()).await
    .map(|_| ()).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn delete_film(id: i64, pool: tauri::State<'_, SqlitePool>) -> Result<(), String> {
    sqlx::query("DELETE FROM film_genres WHERE film_id = ?").bind(id).execute(pool.inner()).await.map_err(|e| e.to_string())?;
    sqlx::query("DELETE FROM person_films WHERE film_id = ?").bind(id).execute(pool.inner()).await.map_err(|e| e.to_string())?;
    sqlx::query("DELETE FROM reviews WHERE film_id = ?").bind(id).execute(pool.inner()).await.map_err(|e| e.to_string())?;
    sqlx::query("DELETE FROM library_items WHERE film_id = ?").bind(id).execute(pool.inner()).await.map_err(|e| e.to_string())?;
    sqlx::query("DELETE FROM wiki_entries WHERE entity_type = 'film' AND entity_id = ?").bind(id).execute(pool.inner()).await.map_err(|e| e.to_string())?;
    sqlx::query("DELETE FROM films WHERE id = ?").bind(id).execute(pool.inner()).await.map_err(|e| e.to_string())?;
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
