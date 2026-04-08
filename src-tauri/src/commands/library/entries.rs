use super::{EntryDetail, EntryRow, EntrySummary, RelationEntry};
use sqlx::{QueryBuilder, SqlitePool};

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
pub async fn create_entry(name: String, pool: tauri::State<'_, SqlitePool>) -> Result<i64, String> {
    sqlx::query("INSERT INTO entries (name) VALUES (?)")
        .bind(&name)
        .execute(pool.inner())
        .await
        .map(|r| r.last_insert_rowid())
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn update_entry_name(
    id: i64,
    name: String,
    pool: tauri::State<'_, SqlitePool>,
) -> Result<(), String> {
    sqlx::query("UPDATE entries SET name = ?, updated_at = datetime('now') WHERE id = ?")
        .bind(&name)
        .bind(id)
        .execute(pool.inner())
        .await
        .map(|_| ())
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn update_entry_wiki(
    id: i64,
    wiki: String,
    pool: tauri::State<'_, SqlitePool>,
) -> Result<(), String> {
    sqlx::query("UPDATE entries SET wiki = ?, updated_at = datetime('now') WHERE id = ?")
        .bind(&wiki)
        .bind(id)
        .execute(pool.inner())
        .await
        .map(|_| ())
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn delete_entry(id: i64, pool: tauri::State<'_, SqlitePool>) -> Result<(), String> {
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
    Ok(())
}

// ── Tags ────────────────────────────────────────────────────────

#[tauri::command]
pub async fn add_entry_tag(
    entry_id: i64,
    tag: String,
    pool: tauri::State<'_, SqlitePool>,
) -> Result<(), String> {
    sqlx::query("INSERT OR IGNORE INTO entry_tags (entry_id, tag) VALUES (?, ?)")
        .bind(entry_id)
        .bind(&tag)
        .execute(pool.inner())
        .await
        .map(|_| ())
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn remove_entry_tag(
    entry_id: i64,
    tag: String,
    pool: tauri::State<'_, SqlitePool>,
) -> Result<(), String> {
    sqlx::query("DELETE FROM entry_tags WHERE entry_id = ? AND tag = ?")
        .bind(entry_id)
        .bind(&tag)
        .execute(pool.inner())
        .await
        .map(|_| ())
        .map_err(|e| e.to_string())
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
    from_id: i64,
    to_id: i64,
    relation_type: String,
    pool: tauri::State<'_, SqlitePool>,
) -> Result<i64, String> {
    sqlx::query("INSERT INTO relations (from_id, to_id, relation_type) VALUES (?, ?, ?)")
        .bind(from_id)
        .bind(to_id)
        .bind(&relation_type)
        .execute(pool.inner())
        .await
        .map(|r| r.last_insert_rowid())
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn remove_relation(id: i64, pool: tauri::State<'_, SqlitePool>) -> Result<(), String> {
    sqlx::query("DELETE FROM relations WHERE id = ?")
        .bind(id)
        .execute(pool.inner())
        .await
        .map(|_| ())
        .map_err(|e| e.to_string())
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

    #[tokio::test]
    async fn create_and_list_entries() {
        let pool = setup().await;
        let id = sqlx::query("INSERT INTO entries (name) VALUES ('Antonioni')")
            .execute(&pool)
            .await
            .unwrap()
            .last_insert_rowid();

        sqlx::query("INSERT INTO entry_tags (entry_id, tag) VALUES (?, '导演')")
            .bind(id)
            .execute(&pool)
            .await
            .unwrap();

        let rows = sqlx::query_as::<_, super::super::EntryRow>(
            "SELECT e.id, e.name, e.wiki,
                    COALESCE(GROUP_CONCAT(et.tag), '') AS tags_csv,
                    e.created_at, e.updated_at
             FROM entries e LEFT JOIN entry_tags et ON et.entry_id = e.id
             GROUP BY e.id ORDER BY e.updated_at DESC",
        )
        .fetch_all(&pool)
        .await
        .unwrap();

        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].name, "Antonioni");
        assert_eq!(rows[0].tags(), vec!["导演"]);
    }

    #[tokio::test]
    async fn entry_wiki_update() {
        let pool = setup().await;
        let id = sqlx::query("INSERT INTO entries (name) VALUES ('Test')")
            .execute(&pool)
            .await
            .unwrap()
            .last_insert_rowid();

        sqlx::query("UPDATE entries SET wiki = ?, updated_at = datetime('now') WHERE id = ?")
            .bind("# Hello")
            .bind(id)
            .execute(&pool)
            .await
            .unwrap();

        let wiki: String = sqlx::query_scalar("SELECT wiki FROM entries WHERE id = ?")
            .bind(id)
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(wiki, "# Hello");
    }

    #[tokio::test]
    async fn tags_crud() {
        let pool = setup().await;
        let id = sqlx::query("INSERT INTO entries (name) VALUES ('Test')")
            .execute(&pool)
            .await
            .unwrap()
            .last_insert_rowid();

        sqlx::query("INSERT OR IGNORE INTO entry_tags (entry_id, tag) VALUES (?, '导演')")
            .bind(id)
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query("INSERT OR IGNORE INTO entry_tags (entry_id, tag) VALUES (?, '意大利')")
            .bind(id)
            .execute(&pool)
            .await
            .unwrap();

        let tags: Vec<String> =
            sqlx::query_scalar("SELECT DISTINCT tag FROM entry_tags ORDER BY tag")
                .fetch_all(&pool)
                .await
                .unwrap();
        assert_eq!(tags, vec!["导演", "意大利"]);

        // Remove one
        sqlx::query("DELETE FROM entry_tags WHERE entry_id = ? AND tag = '意大利'")
            .bind(id)
            .execute(&pool)
            .await
            .unwrap();
        let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM entry_tags WHERE entry_id = ?")
            .bind(id)
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(count, 1);
    }

    #[tokio::test]
    async fn relations_crud() {
        let pool = setup().await;
        let a = sqlx::query("INSERT INTO entries (name) VALUES ('A')")
            .execute(&pool)
            .await
            .unwrap()
            .last_insert_rowid();
        let b = sqlx::query("INSERT INTO entries (name) VALUES ('B')")
            .execute(&pool)
            .await
            .unwrap()
            .last_insert_rowid();

        let rel_id = sqlx::query(
            "INSERT INTO relations (from_id, to_id, relation_type) VALUES (?, ?, 'influenced')",
        )
        .bind(a)
        .bind(b)
        .execute(&pool)
        .await
        .unwrap()
        .last_insert_rowid();

        let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM relations")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(count, 1);

        // Query relations for A
        let rels = sqlx::query_as::<_, super::super::RelationEntry>(
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
        .bind(a)
        .bind(a)
        .fetch_all(&pool)
        .await
        .unwrap();
        assert_eq!(rels.len(), 1);
        assert_eq!(rels[0].target_name, "B");
        assert_eq!(rels[0].direction, "to");

        // Delete relation
        sqlx::query("DELETE FROM relations WHERE id = ?")
            .bind(rel_id)
            .execute(&pool)
            .await
            .unwrap();
        let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM relations")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(count, 0);
    }

    #[tokio::test]
    async fn delete_entry_cascades() {
        let pool = setup().await;
        let a = sqlx::query("INSERT INTO entries (name) VALUES ('A')")
            .execute(&pool)
            .await
            .unwrap()
            .last_insert_rowid();
        let b = sqlx::query("INSERT INTO entries (name) VALUES ('B')")
            .execute(&pool)
            .await
            .unwrap()
            .last_insert_rowid();

        sqlx::query("INSERT INTO entry_tags (entry_id, tag) VALUES (?, '导演')")
            .bind(a)
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query("INSERT INTO relations (from_id, to_id, relation_type) VALUES (?, ?, 'x')")
            .bind(a)
            .bind(b)
            .execute(&pool)
            .await
            .unwrap();

        // Delete A
        sqlx::query("DELETE FROM relations WHERE from_id = ? OR to_id = ?")
            .bind(a)
            .bind(a)
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query("DELETE FROM entry_tags WHERE entry_id = ?")
            .bind(a)
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query("DELETE FROM entries WHERE id = ?")
            .bind(a)
            .execute(&pool)
            .await
            .unwrap();

        let entries: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM entries")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(entries, 1); // Only B remains
        let rels: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM relations")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(rels, 0);
        let tags: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM entry_tags")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(tags, 0);
    }

    #[tokio::test]
    async fn list_entries_filter_by_tag() {
        let pool = setup().await;
        let a = sqlx::query("INSERT INTO entries (name) VALUES ('Antonioni')")
            .execute(&pool)
            .await
            .unwrap()
            .last_insert_rowid();
        let b = sqlx::query("INSERT INTO entries (name) VALUES ('Blow-Up')")
            .execute(&pool)
            .await
            .unwrap()
            .last_insert_rowid();

        sqlx::query("INSERT INTO entry_tags (entry_id, tag) VALUES (?, '导演')")
            .bind(a)
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query("INSERT INTO entry_tags (entry_id, tag) VALUES (?, '电影')")
            .bind(b)
            .execute(&pool)
            .await
            .unwrap();

        // Filter by tag '导演'
        let rows = sqlx::query_as::<_, super::super::EntryRow>(
            "SELECT e.id, e.name, e.wiki,
                    COALESCE(GROUP_CONCAT(et.tag), '') AS tags_csv,
                    e.created_at, e.updated_at
             FROM entries e LEFT JOIN entry_tags et ON et.entry_id = e.id
             WHERE e.id IN (SELECT entry_id FROM entry_tags WHERE tag = '导演')
             GROUP BY e.id ORDER BY e.updated_at DESC",
        )
        .fetch_all(&pool)
        .await
        .unwrap();

        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].name, "Antonioni");
    }
}
