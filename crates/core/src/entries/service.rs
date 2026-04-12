//! Knowledge-base entries CRUD + tags + relations.
//!
//! All functions take `&SqlitePool` and return either data or `Result<_, String>`.
//! Event emission (`entries:changed`) is the responsibility of the caller.

use crate::entries::model::{EntryDetail, EntryRow, EntrySummary, RelationEntry};
use sqlx::{QueryBuilder, SqlitePool};

// ── Entry CRUD ──────────────────────────────────────────────────

pub async fn list_entries(
    pool: &SqlitePool,
    query: Option<&str>,
    tag: Option<&str>,
) -> Result<Vec<EntrySummary>, String> {
    let mut qb = QueryBuilder::<sqlx::Sqlite>::new(
        "SELECT e.id, e.name, e.wiki, \
         COALESCE(GROUP_CONCAT(et.tag), '') AS tags_csv, \
         e.created_at, e.updated_at \
         FROM entries e \
         LEFT JOIN entry_tags et ON et.entry_id = e.id \
         WHERE 1=1",
    );

    if let Some(t) = tag {
        qb.push(" AND e.id IN (SELECT entry_id FROM entry_tags WHERE tag = ");
        qb.push_bind(t.to_string());
        qb.push(")");
    }
    if let Some(q) = query {
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
        .fetch_all(pool)
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

pub async fn get_entry(pool: &SqlitePool, id: i64) -> Result<EntryDetail, String> {
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
    .fetch_one(pool)
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
    .fetch_all(pool)
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

pub async fn create_entry(pool: &SqlitePool, name: &str) -> Result<i64, String> {
    sqlx::query("INSERT INTO entries (name) VALUES (?)")
        .bind(name)
        .execute(pool)
        .await
        .map(|r| r.last_insert_rowid())
        .map_err(|e| e.to_string())
}

pub async fn update_entry_name(pool: &SqlitePool, id: i64, name: &str) -> Result<(), String> {
    sqlx::query("UPDATE entries SET name = ?, updated_at = datetime('now') WHERE id = ?")
        .bind(name)
        .bind(id)
        .execute(pool)
        .await
        .map_err(|e| e.to_string())
        .map(|_| ())
}

pub async fn update_entry_wiki(pool: &SqlitePool, id: i64, wiki: &str) -> Result<(), String> {
    sqlx::query("UPDATE entries SET wiki = ?, updated_at = datetime('now') WHERE id = ?")
        .bind(wiki)
        .bind(id)
        .execute(pool)
        .await
        .map_err(|e| e.to_string())
        .map(|_| ())
}

pub async fn delete_entry(pool: &SqlitePool, id: i64) -> Result<(), String> {
    sqlx::query("DELETE FROM relations WHERE from_id = ? OR to_id = ?")
        .bind(id)
        .bind(id)
        .execute(pool)
        .await
        .map_err(|e| e.to_string())?;
    sqlx::query("DELETE FROM entry_tags WHERE entry_id = ?")
        .bind(id)
        .execute(pool)
        .await
        .map_err(|e| e.to_string())?;
    sqlx::query("DELETE FROM entries WHERE id = ?")
        .bind(id)
        .execute(pool)
        .await
        .map_err(|e| e.to_string())?;
    Ok(())
}

// ── Tags ────────────────────────────────────────────────────────

pub async fn add_entry_tag(pool: &SqlitePool, entry_id: i64, tag: &str) -> Result<(), String> {
    sqlx::query("INSERT OR IGNORE INTO entry_tags (entry_id, tag) VALUES (?, ?)")
        .bind(entry_id)
        .bind(tag)
        .execute(pool)
        .await
        .map_err(|e| e.to_string())
        .map(|_| ())
}

pub async fn remove_entry_tag(pool: &SqlitePool, entry_id: i64, tag: &str) -> Result<(), String> {
    sqlx::query("DELETE FROM entry_tags WHERE entry_id = ? AND tag = ?")
        .bind(entry_id)
        .bind(tag)
        .execute(pool)
        .await
        .map_err(|e| e.to_string())
        .map(|_| ())
}

pub async fn list_all_tags(pool: &SqlitePool) -> Result<Vec<String>, String> {
    sqlx::query_scalar::<_, String>("SELECT DISTINCT tag FROM entry_tags ORDER BY tag")
        .fetch_all(pool)
        .await
        .map_err(|e| e.to_string())
}

// ── Relations ───────────────────────────────────────────────────

pub async fn add_relation(
    pool: &SqlitePool,
    from_id: i64,
    to_id: i64,
    relation_type: &str,
) -> Result<i64, String> {
    sqlx::query("INSERT INTO relations (from_id, to_id, relation_type) VALUES (?, ?, ?)")
        .bind(from_id)
        .bind(to_id)
        .bind(relation_type)
        .execute(pool)
        .await
        .map(|r| r.last_insert_rowid())
        .map_err(|e| e.to_string())
}

pub async fn remove_relation(pool: &SqlitePool, id: i64) -> Result<(), String> {
    sqlx::query("DELETE FROM relations WHERE id = ?")
        .bind(id)
        .execute(pool)
        .await
        .map_err(|e| e.to_string())
        .map(|_| ())
}

pub async fn list_relation_types(pool: &SqlitePool) -> Result<Vec<String>, String> {
    sqlx::query_scalar::<_, String>(
        "SELECT DISTINCT relation_type FROM relations ORDER BY relation_type",
    )
    .fetch_all(pool)
    .await
    .map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[sqlx::test]
    async fn create_and_list_entries(pool: SqlitePool) {
        crate::infra::db::MIGRATOR.run(&pool).await.unwrap();
        create_entry(&pool, "Test Entry").await.unwrap();
        let rows = list_entries(&pool, None, None).await.unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].name, "Test Entry");
    }

    #[sqlx::test]
    async fn entry_wiki_update(pool: SqlitePool) {
        crate::infra::db::MIGRATOR.run(&pool).await.unwrap();
        let id = create_entry(&pool, "A").await.unwrap();
        update_entry_wiki(&pool, id, "hello").await.unwrap();
        let detail = get_entry(&pool, id).await.unwrap();
        assert_eq!(detail.wiki, "hello");
    }

    #[sqlx::test]
    async fn list_entries_filter_by_tag(pool: SqlitePool) {
        crate::infra::db::MIGRATOR.run(&pool).await.unwrap();
        let a = create_entry(&pool, "A").await.unwrap();
        let _b = create_entry(&pool, "B").await.unwrap();
        add_entry_tag(&pool, a, "x").await.unwrap();

        let rows = list_entries(&pool, None, Some("x")).await.unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].id, a);
    }

    #[sqlx::test]
    async fn tags_crud(pool: SqlitePool) {
        crate::infra::db::MIGRATOR.run(&pool).await.unwrap();
        let id = create_entry(&pool, "E").await.unwrap();
        add_entry_tag(&pool, id, "a").await.unwrap();
        add_entry_tag(&pool, id, "b").await.unwrap();

        let tags = list_all_tags(&pool).await.unwrap();
        assert_eq!(tags, vec!["a".to_string(), "b".to_string()]);

        remove_entry_tag(&pool, id, "a").await.unwrap();
        let tags = list_all_tags(&pool).await.unwrap();
        assert_eq!(tags, vec!["b".to_string()]);
    }

    #[sqlx::test]
    async fn delete_entry_cascades(pool: SqlitePool) {
        crate::infra::db::MIGRATOR.run(&pool).await.unwrap();
        let a = create_entry(&pool, "A").await.unwrap();
        let b = create_entry(&pool, "B").await.unwrap();
        add_entry_tag(&pool, a, "t").await.unwrap();
        add_relation(&pool, a, b, "r").await.unwrap();

        delete_entry(&pool, a).await.unwrap();

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
        crate::infra::db::MIGRATOR.run(&pool).await.unwrap();
        let a = create_entry(&pool, "A").await.unwrap();
        let b = create_entry(&pool, "B").await.unwrap();

        let id = add_relation(&pool, a, b, "directed").await.unwrap();

        let rows: Vec<(i64, i64, i64, String)> =
            sqlx::query_as("SELECT id, from_id, to_id, relation_type FROM relations")
                .fetch_all(&pool)
                .await
                .unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].0, id);

        remove_relation(&pool, id).await.unwrap();
        let count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM relations")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(count.0, 0);
    }
}
