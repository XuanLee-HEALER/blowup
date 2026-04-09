use super::{EntryDetail, EntryRow, EntrySummary, RelationEntry};
use sqlx::{QueryBuilder, SqlitePool};
use tauri::Emitter;

const EVENT: &str = "entries:changed";

fn emit_kb(app: &tauri::AppHandle) {
    if let Err(e) = app.emit(EVENT, ()) {
        tracing::warn!(error = %e, "failed to emit {}", EVENT);
    }
}

// ── Entry CRUD ──────────────────────────────────────────────────

#[tauri::command]
pub async fn list_entries(
    query: Option<String>,
    tag: Option<String>,
    pool: tauri::State<'_, SqlitePool>,
) -> Result<Vec<EntrySummary>, String> {
    let mut qb = QueryBuilder::<sqlx::Sqlite>::new(
        "SELECT e.id, e.name, e.wiki, \
         COALESCE(GROUP_CONCAT(et.tag), '') AS tags_csv, \
         e.created_at, e.updated_at \
         FROM entries e \
         LEFT JOIN entry_tags et ON et.entry_id = e.id \
         WHERE 1=1",
    );

    if let Some(t) = &tag {
        qb.push(" AND e.id IN (SELECT entry_id FROM entry_tags WHERE tag = ");
        qb.push_bind(t.clone());
        qb.push(")");
    }
    if let Some(q) = &query {
        let pattern = format!("%{q}%");
        qb.push(" AND (e.name LIKE ");
        qb.push_bind(pattern.clone());
        qb.push(" OR e.wiki LIKE ");
        qb.push_bind(pattern);
        qb.push(")");
    }
    qb.push(" GROUP BY e.id ORDER BY e.updated_at DESC");

    let rows: Vec<EntryRow> = qb
        .build_query_as()
        .fetch_all(pool.inner())
        .await
        .map_err(|e| e.to_string())?;

    Ok(rows
        .into_iter()
        .map(|r| {
            let tags = r.tags();
            EntrySummary {
                id: r.id,
                name: r.name,
                tags,
                updated_at: r.updated_at,
            }
        })
        .collect())
}

#[tauri::command]
pub async fn get_entry(id: i64, pool: tauri::State<'_, SqlitePool>) -> Result<EntryDetail, String> {
    let row = sqlx::query_as::<_, EntryRow>(
        "SELECT e.id, e.name, e.wiki,
                COALESCE(GROUP_CONCAT(et.tag), '') AS tags_csv,
                e.created_at, e.updated_at
         FROM entries e
         LEFT JOIN entry_tags et ON et.entry_id = e.id
         WHERE e.id = ?
         GROUP BY e.id",
    )
    .bind(id)
    .fetch_one(pool.inner())
    .await
    .map_err(|e| e.to_string())?;

    let relations = sqlx::query_as::<_, RelationEntry>(
        "SELECT r.id, r.to_id AS target_id, e.name AS target_name,
                'to' AS direction, r.relation_type
         FROM relations r JOIN entries e ON e.id = r.to_id
         WHERE r.from_id = ?
         UNION ALL
         SELECT r.id, r.from_id AS target_id, e.name AS target_name,
                'from' AS direction, r.relation_type
         FROM relations r JOIN entries e ON e.id = r.from_id
         WHERE r.to_id = ?",
    )
    .bind(id)
    .bind(id)
    .fetch_all(pool.inner())
    .await
    .map_err(|e| e.to_string())?;

    let tags = row.tags();
    Ok(EntryDetail {
        id: row.id,
        name: row.name,
        wiki: row.wiki,
        tags,
        relations,
        created_at: row.created_at,
        updated_at: row.updated_at,
    })
}

#[tauri::command]
pub async fn create_entry(
    app: tauri::AppHandle,
    name: String,
    pool: tauri::State<'_, SqlitePool>,
) -> Result<i64, String> {
    let id = sqlx::query("INSERT INTO entries (name) VALUES (?)")
        .bind(&name)
        .execute(pool.inner())
        .await
        .map(|r| r.last_insert_rowid())
        .map_err(|e| e.to_string())?;
    emit_kb(&app);
    Ok(id)
}

#[tauri::command]
pub async fn update_entry_name(
    app: tauri::AppHandle,
    id: i64,
    name: String,
    pool: tauri::State<'_, SqlitePool>,
) -> Result<(), String> {
    sqlx::query("UPDATE entries SET name = ?, updated_at = datetime('now') WHERE id = ?")
        .bind(&name)
        .bind(id)
        .execute(pool.inner())
        .await
        .map_err(|e| e.to_string())?;
    emit_kb(&app);
    Ok(())
}

#[tauri::command]
pub async fn update_entry_wiki(
    app: tauri::AppHandle,
    id: i64,
    wiki: String,
    pool: tauri::State<'_, SqlitePool>,
) -> Result<(), String> {
    sqlx::query("UPDATE entries SET wiki = ?, updated_at = datetime('now') WHERE id = ?")
        .bind(&wiki)
        .bind(id)
        .execute(pool.inner())
        .await
        .map_err(|e| e.to_string())?;
    emit_kb(&app);
    Ok(())
}

#[tauri::command]
pub async fn delete_entry(
    app: tauri::AppHandle,
    id: i64,
    pool: tauri::State<'_, SqlitePool>,
) -> Result<(), String> {
    sqlx::query("DELETE FROM relations WHERE from_id = ? OR to_id = ?")
        .bind(id)
        .bind(id)
        .execute(pool.inner())
        .await
        .map_err(|e| e.to_string())?;
    sqlx::query("DELETE FROM entry_tags WHERE entry_id = ?")
        .bind(id)
        .execute(pool.inner())
        .await
        .map_err(|e| e.to_string())?;
    sqlx::query("DELETE FROM entries WHERE id = ?")
        .bind(id)
        .execute(pool.inner())
        .await
        .map_err(|e| e.to_string())?;
    emit_kb(&app);
    Ok(())
}

// ── Tags ────────────────────────────────────────────────────────

#[tauri::command]
pub async fn add_entry_tag(
    app: tauri::AppHandle,
    entry_id: i64,
    tag: String,
    pool: tauri::State<'_, SqlitePool>,
) -> Result<(), String> {
    sqlx::query("INSERT OR IGNORE INTO entry_tags (entry_id, tag) VALUES (?, ?)")
        .bind(entry_id)
        .bind(&tag)
        .execute(pool.inner())
        .await
        .map_err(|e| e.to_string())?;
    emit_kb(&app);
    Ok(())
}

#[tauri::command]
pub async fn remove_entry_tag(
    app: tauri::AppHandle,
    entry_id: i64,
    tag: String,
    pool: tauri::State<'_, SqlitePool>,
) -> Result<(), String> {
    sqlx::query("DELETE FROM entry_tags WHERE entry_id = ? AND tag = ?")
        .bind(entry_id)
        .bind(&tag)
        .execute(pool.inner())
        .await
        .map_err(|e| e.to_string())?;
    emit_kb(&app);
    Ok(())
}

#[tauri::command]
pub async fn list_all_tags(pool: tauri::State<'_, SqlitePool>) -> Result<Vec<String>, String> {
    sqlx::query_scalar::<_, String>("SELECT DISTINCT tag FROM entry_tags ORDER BY tag")
        .fetch_all(pool.inner())
        .await
        .map_err(|e| e.to_string())
}

// ── Relations ───────────────────────────────────────────────────

#[tauri::command]
pub async fn add_relation(
    app: tauri::AppHandle,
    from_id: i64,
    to_id: i64,
    relation_type: String,
    pool: tauri::State<'_, SqlitePool>,
) -> Result<i64, String> {
    let id = sqlx::query("INSERT INTO relations (from_id, to_id, relation_type) VALUES (?, ?, ?)")
        .bind(from_id)
        .bind(to_id)
        .bind(&relation_type)
        .execute(pool.inner())
        .await
        .map(|r| r.last_insert_rowid())
        .map_err(|e| e.to_string())?;
    emit_kb(&app);
    Ok(id)
}

#[tauri::command]
pub async fn remove_relation(
    app: tauri::AppHandle,
    id: i64,
    pool: tauri::State<'_, SqlitePool>,
) -> Result<(), String> {
    sqlx::query("DELETE FROM relations WHERE id = ?")
        .bind(id)
        .execute(pool.inner())
        .await
        .map_err(|e| e.to_string())?;
    emit_kb(&app);
    Ok(())
}

#[tauri::command]
pub async fn list_relation_types(
    pool: tauri::State<'_, SqlitePool>,
) -> Result<Vec<String>, String> {
    sqlx::query_scalar::<_, String>(
        "SELECT DISTINCT relation_type FROM relations ORDER BY relation_type",
    )
    .fetch_all(pool.inner())
    .await
    .map_err(|e| e.to_string())
}

// ── Tests ───────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use sqlx::SqlitePool;

    async fn setup() -> SqlitePool {
        let pool = SqlitePool::connect(":memory:").await.unwrap();
        sqlx::migrate!("./migrations").run(&pool).await.unwrap();
        pool
    }

    #[sqlx::test]
    async fn create_and_list_entries(pool: SqlitePool) {
        sqlx::migrate!("./migrations").run(&pool).await.unwrap();
        sqlx::query("INSERT INTO entries (name) VALUES ('Test Entry')")
            .execute(&pool)
            .await
            .unwrap();
        let rows: Vec<(i64, String)> = sqlx::query_as("SELECT id, name FROM entries")
            .fetch_all(&pool)
            .await
            .unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].1, "Test Entry");
    }

    #[sqlx::test]
    async fn entry_wiki_update(pool: SqlitePool) {
        sqlx::migrate!("./migrations").run(&pool).await.unwrap();
        sqlx::query("INSERT INTO entries (name) VALUES ('A')")
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query("UPDATE entries SET wiki = 'hello' WHERE id = 1")
            .execute(&pool)
            .await
            .unwrap();
        let wiki: String = sqlx::query_scalar("SELECT wiki FROM entries WHERE id = 1")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(wiki, "hello");
    }

    #[sqlx::test]
    async fn list_entries_filter_by_tag(pool: SqlitePool) {
        sqlx::migrate!("./migrations").run(&pool).await.unwrap();
        sqlx::query("INSERT INTO entries (name) VALUES ('A')")
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query("INSERT INTO entries (name) VALUES ('B')")
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query("INSERT INTO entry_tags (entry_id, tag) VALUES (1, 'x')")
            .execute(&pool)
            .await
            .unwrap();

        // Only entry 1 has tag 'x'
        let rows: Vec<(i64,)> = sqlx::query_as(
            "SELECT e.id FROM entries e \
             JOIN entry_tags et ON et.entry_id = e.id \
             WHERE et.tag = 'x'",
        )
        .fetch_all(&pool)
        .await
        .unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].0, 1);
    }

    #[sqlx::test]
    async fn tags_crud(pool: SqlitePool) {
        sqlx::migrate!("./migrations").run(&pool).await.unwrap();
        sqlx::query("INSERT INTO entries (name) VALUES ('E')")
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query("INSERT INTO entry_tags (entry_id, tag) VALUES (1, 'a')")
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query("INSERT INTO entry_tags (entry_id, tag) VALUES (1, 'b')")
            .execute(&pool)
            .await
            .unwrap();
        let tags: Vec<String> =
            sqlx::query_scalar("SELECT tag FROM entry_tags WHERE entry_id = 1 ORDER BY tag")
                .fetch_all(&pool)
                .await
                .unwrap();
        assert_eq!(tags, vec!["a", "b"]);

        sqlx::query("DELETE FROM entry_tags WHERE entry_id = 1 AND tag = 'a'")
            .execute(&pool)
            .await
            .unwrap();
        let tags: Vec<String> =
            sqlx::query_scalar("SELECT tag FROM entry_tags WHERE entry_id = 1 ORDER BY tag")
                .fetch_all(&pool)
                .await
                .unwrap();
        assert_eq!(tags, vec!["b"]);
    }

    #[sqlx::test]
    async fn delete_entry_cascades(pool: SqlitePool) {
        sqlx::migrate!("./migrations").run(&pool).await.unwrap();
        sqlx::query("INSERT INTO entries (name) VALUES ('A')")
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query("INSERT INTO entries (name) VALUES ('B')")
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query("INSERT INTO entry_tags (entry_id, tag) VALUES (1, 't')")
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query("INSERT INTO relations (from_id, to_id, relation_type) VALUES (1, 2, 'r')")
            .execute(&pool)
            .await
            .unwrap();

        // Delete entry 1
        sqlx::query("DELETE FROM relations WHERE from_id = 1 OR to_id = 1")
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query("DELETE FROM entry_tags WHERE entry_id = 1")
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query("DELETE FROM entries WHERE id = 1")
            .execute(&pool)
            .await
            .unwrap();

        let count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM entries")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(count.0, 1);
        let rel_count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM relations")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(rel_count.0, 0);
    }

    #[sqlx::test]
    async fn relations_crud(pool: SqlitePool) {
        sqlx::migrate!("./migrations").run(&pool).await.unwrap();
        sqlx::query("INSERT INTO entries (name) VALUES ('A')")
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query("INSERT INTO entries (name) VALUES ('B')")
            .execute(&pool)
            .await
            .unwrap();

        let id = sqlx::query(
            "INSERT INTO relations (from_id, to_id, relation_type) VALUES (1, 2, 'directed')",
        )
        .execute(&pool)
        .await
        .unwrap()
        .last_insert_rowid();

        let rows: Vec<(i64, i64, i64, String)> =
            sqlx::query_as("SELECT id, from_id, to_id, relation_type FROM relations")
                .fetch_all(&pool)
                .await
                .unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].0, id);

        sqlx::query("DELETE FROM relations WHERE id = ?")
            .bind(id)
            .execute(&pool)
            .await
            .unwrap();
        let count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM relations")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(count.0, 0);
    }
}
