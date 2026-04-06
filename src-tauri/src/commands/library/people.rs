use super::{PersonDetail, PersonFilmEntry, PersonRelation, PersonSummary};
use sqlx::SqlitePool;

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
    .fetch_all(pool.inner())
    .await
    .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_person(
    id: i64,
    pool: tauri::State<'_, SqlitePool>,
) -> Result<PersonDetail, String> {
    let row = sqlx::query_as::<_, PersonRow>(
        "SELECT id, tmdb_id, name, primary_role, born_date, nationality, biography
         FROM people WHERE id = ?",
    )
    .bind(id)
    .fetch_one(pool.inner())
    .await
    .map_err(|e| e.to_string())?;

    let wiki_content = super::get_wiki_content(pool.inner(), "person", id).await?;

    let films = sqlx::query_as::<_, PersonFilmEntry>(
        "SELECT f.id as film_id, f.title, f.year, pf.role, f.poster_cache_path
         FROM person_films pf JOIN films f ON f.id = pf.film_id
         WHERE pf.person_id = ? ORDER BY f.year",
    )
    .bind(id)
    .fetch_all(pool.inner())
    .await
    .map_err(|e| e.to_string())?;

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
    .bind(id)
    .bind(id)
    .fetch_all(pool.inner())
    .await
    .map_err(|e| e.to_string())?;

    Ok(PersonDetail {
        id: row.id,
        tmdb_id: row.tmdb_id,
        name: row.name,
        primary_role: row.primary_role,
        born_date: row.born_date,
        nationality: row.nationality,
        biography: row.biography,
        wiki_content,
        films,
        relations,
    })
}

#[tauri::command]
pub async fn create_person(
    name: String,
    primary_role: String,
    tmdb_id: Option<i64>,
    born_date: Option<String>,
    nationality: Option<String>,
    pool: tauri::State<'_, SqlitePool>,
) -> Result<i64, String> {
    sqlx::query(
        "INSERT INTO people (name, primary_role, tmdb_id, born_date, nationality) VALUES (?, ?, ?, ?, ?)",
    )
    .bind(&name).bind(&primary_role).bind(tmdb_id).bind(born_date).bind(nationality)
    .execute(pool.inner()).await
    .map(|r| r.last_insert_rowid()).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn update_person_wiki(
    id: i64,
    content: String,
    pool: tauri::State<'_, SqlitePool>,
) -> Result<(), String> {
    super::upsert_wiki(pool.inner(), "person", id, &content).await
}

#[tauri::command]
pub async fn delete_person(id: i64, pool: tauri::State<'_, SqlitePool>) -> Result<(), String> {
    sqlx::query("DELETE FROM person_relations WHERE from_id = ? OR to_id = ?")
        .bind(id)
        .bind(id)
        .execute(pool.inner())
        .await
        .map_err(|e| e.to_string())?;
    sqlx::query("DELETE FROM person_films WHERE person_id = ?")
        .bind(id)
        .execute(pool.inner())
        .await
        .map_err(|e| e.to_string())?;
    sqlx::query("DELETE FROM person_genres WHERE person_id = ?")
        .bind(id)
        .execute(pool.inner())
        .await
        .map_err(|e| e.to_string())?;
    sqlx::query("DELETE FROM wiki_entries WHERE entity_type = 'person' AND entity_id = ?")
        .bind(id)
        .execute(pool.inner())
        .await
        .map_err(|e| e.to_string())?;
    sqlx::query("DELETE FROM people WHERE id = ?")
        .bind(id)
        .execute(pool.inner())
        .await
        .map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub async fn add_person_relation(
    from_id: i64,
    to_id: i64,
    relation_type: String,
    pool: tauri::State<'_, SqlitePool>,
) -> Result<(), String> {
    sqlx::query(
        "INSERT OR IGNORE INTO person_relations (from_id, to_id, relation_type) VALUES (?, ?, ?)",
    )
    .bind(from_id)
    .bind(to_id)
    .bind(&relation_type)
    .execute(pool.inner())
    .await
    .map(|_| ())
    .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn remove_person_relation(
    from_id: i64,
    to_id: i64,
    relation_type: String,
    pool: tauri::State<'_, SqlitePool>,
) -> Result<(), String> {
    sqlx::query(
        "DELETE FROM person_relations WHERE from_id = ? AND to_id = ? AND relation_type = ?",
    )
    .bind(from_id)
    .bind(to_id)
    .bind(&relation_type)
    .execute(pool.inner())
    .await
    .map(|_| ())
    .map_err(|e| e.to_string())
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
        .execute(&pool)
        .await
        .unwrap()
        .last_insert_rowid();

        let rows = sqlx::query_as::<_, PersonSummary>(
            "SELECT p.id, p.name, p.primary_role, COUNT(pf.film_id) as film_count
             FROM people p LEFT JOIN person_films pf ON pf.person_id = p.id
             GROUP BY p.id ORDER BY p.name",
        )
        .fetch_all(&pool)
        .await
        .unwrap();

        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].id, id);
        assert_eq!(rows[0].film_count, 0);
    }

    #[tokio::test]
    async fn wiki_upsert() {
        let pool = setup().await;
        let id = sqlx::query("INSERT INTO people (name, primary_role) VALUES ('Test', 'director')")
            .execute(&pool)
            .await
            .unwrap()
            .last_insert_rowid();

        for content in ["First", "Updated"] {
            sqlx::query(
                "INSERT INTO wiki_entries (entity_type, entity_id, content, updated_at)
                 VALUES ('person', ?, ?, datetime('now'))
                 ON CONFLICT(entity_type, entity_id)
                 DO UPDATE SET content = excluded.content, updated_at = excluded.updated_at",
            )
            .bind(id)
            .bind(content)
            .execute(&pool)
            .await
            .unwrap();
        }

        let saved: String = sqlx::query_scalar(
            "SELECT content FROM wiki_entries WHERE entity_type = 'person' AND entity_id = ?",
        )
        .bind(id)
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(saved, "Updated");
    }

    #[tokio::test]
    async fn person_relations() {
        let pool = setup().await;
        let a = sqlx::query("INSERT INTO people (name, primary_role) VALUES ('A', 'director')")
            .execute(&pool)
            .await
            .unwrap()
            .last_insert_rowid();
        let b = sqlx::query("INSERT INTO people (name, primary_role) VALUES ('B', 'director')")
            .execute(&pool)
            .await
            .unwrap()
            .last_insert_rowid();

        sqlx::query(
            "INSERT OR IGNORE INTO person_relations (from_id, to_id, relation_type) VALUES (?, ?, 'influenced')",
        )
        .bind(a).bind(b).execute(&pool).await.unwrap();

        let count: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM person_relations WHERE from_id = ? AND to_id = ?",
        )
        .bind(a)
        .bind(b)
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(count, 1);
    }
}
