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
    .execute(pool.inner()).await
    .map(|r| r.last_insert_rowid()).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn update_review(
    id: i64, content: String, rating: Option<f64>,
    pool: tauri::State<'_, SqlitePool>,
) -> Result<(), String> {
    sqlx::query("UPDATE reviews SET content = ?, rating = ? WHERE id = ?")
        .bind(&content).bind(rating).bind(id).execute(pool.inner()).await
        .map(|_| ()).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn delete_review(id: i64, pool: tauri::State<'_, SqlitePool>) -> Result<(), String> {
    sqlx::query("DELETE FROM reviews WHERE id = ?").bind(id).execute(pool.inner()).await
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
