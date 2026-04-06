use super::{FilmSummary, GenreDetail, GenreSummary, GenreTreeNode, PersonSummary};
use sqlx::SqlitePool;
use std::collections::HashMap;

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
    nodes
        .iter()
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
pub async fn list_genres_tree(
    pool: tauri::State<'_, SqlitePool>,
) -> Result<Vec<GenreTreeNode>, String> {
    let rows = sqlx::query_as::<_, GenreRow>(
        "SELECT id, name, description, parent_id, period FROM genres",
    )
    .fetch_all(pool.inner())
    .await
    .map_err(|e| e.to_string())?;

    let counts = sqlx::query_as::<_, FilmCountRow>(
        "SELECT genre_id, COUNT(*) as count FROM film_genres GROUP BY genre_id",
    )
    .fetch_all(pool.inner())
    .await
    .map_err(|e| e.to_string())?;

    let count_map: HashMap<i64, i64> = counts.into_iter().map(|r| (r.genre_id, r.count)).collect();
    Ok(build_tree(&rows, &count_map, None))
}

#[tauri::command]
pub async fn get_genre(id: i64, pool: tauri::State<'_, SqlitePool>) -> Result<GenreDetail, String> {
    let row = sqlx::query_as::<_, GenreRow>(
        "SELECT id, name, description, parent_id, period FROM genres WHERE id = ?",
    )
    .bind(id)
    .fetch_one(pool.inner())
    .await
    .map_err(|e| e.to_string())?;

    let wiki_content = super::get_wiki_content(pool.inner(), "genre", id).await?;

    let children = sqlx::query_as::<_, GenreSummary>(
        "SELECT g.id, g.name,
                (SELECT COUNT(*) FROM film_genres WHERE genre_id = g.id) as film_count,
                (SELECT COUNT(*) FROM genres WHERE parent_id = g.id) as child_count
         FROM genres g WHERE g.parent_id = ?",
    )
    .bind(id)
    .fetch_all(pool.inner())
    .await
    .map_err(|e| e.to_string())?;

    let people = sqlx::query_as::<_, PersonSummary>(
        "SELECT p.id, p.name, p.primary_role, COUNT(pf.film_id) as film_count
         FROM person_genres pg
         JOIN people p ON p.id = pg.person_id
         LEFT JOIN person_films pf ON pf.person_id = p.id
         WHERE pg.genre_id = ?
         GROUP BY p.id ORDER BY p.name",
    )
    .bind(id)
    .fetch_all(pool.inner())
    .await
    .map_err(|e| e.to_string())?;

    let films = sqlx::query_as::<_, FilmSummary>(
        "SELECT f.id, f.title, f.year, f.tmdb_rating, f.poster_cache_path
         FROM film_genres fg JOIN films f ON f.id = fg.film_id
         WHERE fg.genre_id = ? ORDER BY f.year DESC",
    )
    .bind(id)
    .fetch_all(pool.inner())
    .await
    .map_err(|e| e.to_string())?;

    Ok(GenreDetail {
        id: row.id,
        name: row.name,
        description: row.description,
        parent_id: row.parent_id,
        period: row.period,
        wiki_content,
        children,
        people,
        films,
    })
}

#[tauri::command]
pub async fn create_genre(
    name: String,
    parent_id: Option<i64>,
    description: Option<String>,
    period: Option<String>,
    pool: tauri::State<'_, SqlitePool>,
) -> Result<i64, String> {
    sqlx::query("INSERT INTO genres (name, parent_id, description, period) VALUES (?, ?, ?, ?)")
        .bind(&name)
        .bind(parent_id)
        .bind(description)
        .bind(period)
        .execute(pool.inner())
        .await
        .map(|r| r.last_insert_rowid())
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn update_genre_wiki(
    id: i64,
    content: String,
    pool: tauri::State<'_, SqlitePool>,
) -> Result<(), String> {
    super::upsert_wiki(pool.inner(), "genre", id, &content).await
}

#[tauri::command]
pub async fn delete_genre(id: i64, pool: tauri::State<'_, SqlitePool>) -> Result<(), String> {
    // Reparent children to this genre's parent
    sqlx::query(
        "UPDATE genres SET parent_id = (SELECT parent_id FROM genres WHERE id = ?) WHERE parent_id = ?",
    )
    .bind(id).bind(id).execute(pool.inner()).await.map_err(|e| e.to_string())?;

    sqlx::query("DELETE FROM film_genres WHERE genre_id = ?")
        .bind(id)
        .execute(pool.inner())
        .await
        .map_err(|e| e.to_string())?;
    sqlx::query("DELETE FROM person_genres WHERE genre_id = ?")
        .bind(id)
        .execute(pool.inner())
        .await
        .map_err(|e| e.to_string())?;
    sqlx::query("DELETE FROM wiki_entries WHERE entity_type = 'genre' AND entity_id = ?")
        .bind(id)
        .execute(pool.inner())
        .await
        .map_err(|e| e.to_string())?;
    sqlx::query("DELETE FROM genres WHERE id = ?")
        .bind(id)
        .execute(pool.inner())
        .await
        .map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub async fn link_film_genre(
    film_id: i64,
    genre_id: i64,
    pool: tauri::State<'_, SqlitePool>,
) -> Result<(), String> {
    sqlx::query("INSERT OR IGNORE INTO film_genres (film_id, genre_id) VALUES (?, ?)")
        .bind(film_id)
        .bind(genre_id)
        .execute(pool.inner())
        .await
        .map(|_| ())
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn unlink_film_genre(
    film_id: i64,
    genre_id: i64,
    pool: tauri::State<'_, SqlitePool>,
) -> Result<(), String> {
    sqlx::query("DELETE FROM film_genres WHERE film_id = ? AND genre_id = ?")
        .bind(film_id)
        .bind(genre_id)
        .execute(pool.inner())
        .await
        .map(|_| ())
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn link_person_genre(
    person_id: i64,
    genre_id: i64,
    pool: tauri::State<'_, SqlitePool>,
) -> Result<(), String> {
    sqlx::query("INSERT OR IGNORE INTO person_genres (person_id, genre_id) VALUES (?, ?)")
        .bind(person_id)
        .bind(genre_id)
        .execute(pool.inner())
        .await
        .map(|_| ())
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn unlink_person_genre(
    person_id: i64,
    genre_id: i64,
    pool: tauri::State<'_, SqlitePool>,
) -> Result<(), String> {
    sqlx::query("DELETE FROM person_genres WHERE person_id = ? AND genre_id = ?")
        .bind(person_id)
        .bind(genre_id)
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
    async fn build_genre_tree_empty() {
        let pool = setup().await;
        let rows = sqlx::query_as::<_, GenreRow>(
            "SELECT id, name, description, parent_id, period FROM genres",
        )
        .fetch_all(&pool)
        .await
        .unwrap();
        let tree = build_tree(&rows, &HashMap::new(), None);
        assert!(tree.is_empty());
    }

    #[tokio::test]
    async fn build_genre_tree_with_children() {
        let pool = setup().await;
        let parent = sqlx::query("INSERT INTO genres (name) VALUES ('Drama')")
            .execute(&pool)
            .await
            .unwrap()
            .last_insert_rowid();
        sqlx::query("INSERT INTO genres (name, parent_id) VALUES ('Neorealism', ?)")
            .bind(parent)
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query("INSERT INTO genres (name, parent_id) VALUES ('New Wave', ?)")
            .bind(parent)
            .execute(&pool)
            .await
            .unwrap();

        let rows = sqlx::query_as::<_, GenreRow>(
            "SELECT id, name, description, parent_id, period FROM genres",
        )
        .fetch_all(&pool)
        .await
        .unwrap();
        let tree = build_tree(&rows, &HashMap::new(), None);

        assert_eq!(tree.len(), 1);
        assert_eq!(tree[0].name, "Drama");
        assert_eq!(tree[0].children.len(), 2);
    }
}
