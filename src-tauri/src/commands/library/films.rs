use sqlx::{SqlitePool, QueryBuilder};
use super::{FilmDetail, FilmFilterResult, FilmListEntry, FilmPersonEntry, FilmSummary, GenreSummary, ReviewEntry, TmdbMovieInput};

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
    sqlx::query("UPDATE library_items SET film_id = NULL WHERE film_id = ?").bind(id).execute(pool.inner()).await.map_err(|e| e.to_string())?;
    sqlx::query("DELETE FROM wiki_entries WHERE entity_type = 'film' AND entity_id = ?").bind(id).execute(pool.inner()).await.map_err(|e| e.to_string())?;
    sqlx::query("DELETE FROM films WHERE id = ?").bind(id).execute(pool.inner()).await.map_err(|e| e.to_string())?;
    Ok(())
}

fn apply_film_filters(
    qb: &mut QueryBuilder<'_, sqlx::Sqlite>,
    query: &Option<String>,
    genre_id: Option<i64>,
    year_from: Option<i64>,
    year_to: Option<i64>,
    min_rating: Option<f64>,
    has_file: Option<bool>,
) {
    if let Some(q) = query {
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

    #[tokio::test]
    async fn test_list_films_filtered_by_query() {
        let pool = setup().await;
        sqlx::query("INSERT INTO films (title, year) VALUES (?, ?)")
            .bind("Blow-Up").bind(1966_i64).execute(&pool).await.unwrap();
        sqlx::query("INSERT INTO films (title, year) VALUES (?, ?)")
            .bind("Stalker").bind(1979_i64).execute(&pool).await.unwrap();
        sqlx::query("INSERT INTO films (title, year) VALUES (?, ?)")
            .bind("Blowout").bind(1981_i64).execute(&pool).await.unwrap();

        let result = super::list_films_filtered_inner(
            &pool, Some("Blow".to_string()), None, None, None, None, None, None, None, None, None,
        ).await.unwrap();
        assert_eq!(result.films.len(), 2);
        assert_eq!(result.total, 2);
    }

    #[tokio::test]
    async fn test_list_films_filtered_by_year_range() {
        let pool = setup().await;
        sqlx::query("INSERT INTO films (title, year) VALUES (?, ?)")
            .bind("Film A").bind(1960_i64).execute(&pool).await.unwrap();
        sqlx::query("INSERT INTO films (title, year) VALUES (?, ?)")
            .bind("Film B").bind(1975_i64).execute(&pool).await.unwrap();
        sqlx::query("INSERT INTO films (title, year) VALUES (?, ?)")
            .bind("Film C").bind(1990_i64).execute(&pool).await.unwrap();

        let result = super::list_films_filtered_inner(
            &pool, None, None, Some(1970), Some(1980), None, None, None, None, None, None,
        ).await.unwrap();
        assert_eq!(result.films.len(), 1);
        assert_eq!(result.films[0].title, "Film B");
    }

    #[tokio::test]
    async fn test_list_films_filtered_has_file() {
        let pool = setup().await;
        sqlx::query("INSERT INTO films (title) VALUES (?)").bind("Film A").execute(&pool).await.unwrap();
        sqlx::query("INSERT INTO films (title) VALUES (?)").bind("Film B").execute(&pool).await.unwrap();
        sqlx::query("INSERT INTO library_items (film_id, file_path) VALUES (?, ?)")
            .bind(1_i64).bind("/a.mp4").execute(&pool).await.unwrap();

        let with_file = super::list_films_filtered_inner(
            &pool, None, None, None, None, None, Some(true), None, None, None, None,
        ).await.unwrap();
        assert_eq!(with_file.films.len(), 1);
        assert_eq!(with_file.films[0].title, "Film A");

        let without_file = super::list_films_filtered_inner(
            &pool, None, None, None, None, None, Some(false), None, None, None, None,
        ).await.unwrap();
        assert_eq!(without_file.films.len(), 1);
        assert_eq!(without_file.films[0].title, "Film B");
    }

    #[tokio::test]
    async fn test_list_films_filtered_pagination() {
        let pool = setup().await;
        for i in 1..=25 {
            sqlx::query("INSERT INTO films (title, year) VALUES (?, ?)")
                .bind(format!("Film {}", i)).bind(2000_i64 + i)
                .execute(&pool).await.unwrap();
        }

        let page1 = super::list_films_filtered_inner(
            &pool, None, None, None, None, None, None, None, None, Some(1), Some(10),
        ).await.unwrap();
        assert_eq!(page1.films.len(), 10);
        assert_eq!(page1.total, 25);
        assert_eq!(page1.page, 1);
        assert_eq!(page1.page_size, 10);

        let page3 = super::list_films_filtered_inner(
            &pool, None, None, None, None, None, None, None, None, Some(3), Some(10),
        ).await.unwrap();
        assert_eq!(page3.films.len(), 5);
    }
}
