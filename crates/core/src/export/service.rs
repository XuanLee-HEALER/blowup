//! Knowledge-base and config export/import services.
//!
//! All the business logic (DB dump, DB restore, config file round-trip,
//! S3 put/get) lives here. Tauri wrappers in
//! `blowup-tauri/src/commands/export.rs` call these functions and
//! handle event emission.

use crate::config::{Config, SyncConfig};
use crate::export::s3;
use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;
use std::path::Path;

pub const S3_KEY_KB: &str = "knowledge-base.json";
pub const S3_KEY_CONFIG: &str = "config.toml";

// ── Export types ─────────────────────────────────────────────────

#[derive(Serialize, Deserialize)]
pub struct KnowledgeBaseExport {
    pub version: String,
    pub exported_at: String,
    pub entries: Vec<EntryRow>,
    pub entry_tags: Vec<EntryTagRow>,
    pub relations: Vec<RelationRow>,
}

#[derive(Serialize, Deserialize, sqlx::FromRow)]
pub struct EntryRow {
    pub id: i64,
    pub name: String,
    pub wiki: String,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Serialize, Deserialize, sqlx::FromRow)]
pub struct EntryTagRow {
    pub entry_id: i64,
    pub tag: String,
}

#[derive(Serialize, Deserialize, sqlx::FromRow)]
pub struct RelationRow {
    pub id: i64,
    pub from_id: i64,
    pub to_id: i64,
    pub relation_type: String,
}

// ── Knowledge-base serialization ─────────────────────────────────

pub async fn serialize_knowledge_base(pool: &SqlitePool) -> Result<String, String> {
    let entries =
        sqlx::query_as::<_, EntryRow>("SELECT id, name, wiki, created_at, updated_at FROM entries")
            .fetch_all(pool)
            .await
            .map_err(|e| e.to_string())?;

    let entry_tags = sqlx::query_as::<_, EntryTagRow>("SELECT entry_id, tag FROM entry_tags")
        .fetch_all(pool)
        .await
        .map_err(|e| e.to_string())?;

    let relations =
        sqlx::query_as::<_, RelationRow>("SELECT id, from_id, to_id, relation_type FROM relations")
            .fetch_all(pool)
            .await
            .map_err(|e| e.to_string())?;

    let export = KnowledgeBaseExport {
        version: "3.0.0".to_string(),
        exported_at: chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string(),
        entries,
        entry_tags,
        relations,
    };

    serde_json::to_string_pretty(&export).map_err(|e| e.to_string())
}

pub async fn import_knowledge_base_data(
    pool: &SqlitePool,
    json: &str,
) -> Result<String, String> {
    #[derive(Deserialize)]
    struct VersionProbe {
        #[serde(default)]
        version: String,
    }
    if let Ok(probe) = serde_json::from_str::<VersionProbe>(json)
        && (probe.version.starts_with("2.") || probe.version.starts_with("1."))
    {
        return Err("导入失败: 该文件为旧版知识库格式 (v2.x)，不兼容当前版本".to_string());
    }

    let data: KnowledgeBaseExport =
        serde_json::from_str(json).map_err(|e| format!("JSON 解析失败: {}", e))?;

    let mut imported_entries: i64 = 0;
    let mut imported_tags: i64 = 0;
    let mut imported_relations: i64 = 0;

    for e in &data.entries {
        let exists = sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM entries WHERE id = ?")
            .bind(e.id)
            .fetch_one(pool)
            .await
            .unwrap_or(0);
        if exists > 0 {
            continue;
        }
        sqlx::query(
            "INSERT INTO entries (id, name, wiki, created_at, updated_at) VALUES (?, ?, ?, ?, ?)",
        )
        .bind(e.id)
        .bind(&e.name)
        .bind(&e.wiki)
        .bind(&e.created_at)
        .bind(&e.updated_at)
        .execute(pool)
        .await
        .map_err(|e| e.to_string())?;
        imported_entries += 1;
    }

    for t in &data.entry_tags {
        if let Err(e) =
            sqlx::query("INSERT OR IGNORE INTO entry_tags (entry_id, tag) VALUES (?, ?)")
                .bind(t.entry_id)
                .bind(&t.tag)
                .execute(pool)
                .await
        {
            tracing::warn!(entry_id = t.entry_id, tag = %t.tag, error = %e, "failed to import tag");
            continue;
        }
        imported_tags += 1;
    }

    for r in &data.relations {
        let exists = sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM relations WHERE id = ?")
            .bind(r.id)
            .fetch_one(pool)
            .await
            .unwrap_or(0);
        if exists > 0 {
            continue;
        }
        if let Err(e) = sqlx::query(
            "INSERT INTO relations (id, from_id, to_id, relation_type) VALUES (?, ?, ?, ?)",
        )
        .bind(r.id)
        .bind(r.from_id)
        .bind(r.to_id)
        .bind(&r.relation_type)
        .execute(pool)
        .await
        {
            tracing::warn!(relation_id = r.id, error = %e, "failed to import relation");
            continue;
        }
        imported_relations += 1;
    }

    Ok(format!(
        "导入完成: {} 条目, {} 标签, {} 关系",
        imported_entries, imported_tags, imported_relations
    ))
}

/// Export the knowledge base to a local file as JSON.
pub async fn export_knowledge_base_to_file(pool: &SqlitePool, path: &Path) -> Result<(), String> {
    let json = serialize_knowledge_base(pool).await?;
    std::fs::write(path, json).map_err(|e| e.to_string())?;
    Ok(())
}

/// Import the knowledge base from a local JSON file.
pub async fn import_knowledge_base_from_file(
    pool: &SqlitePool,
    path: &Path,
) -> Result<String, String> {
    let json = std::fs::read_to_string(path).map_err(|e| e.to_string())?;
    import_knowledge_base_data(pool, &json).await
}

// ── Config export/import ─────────────────────────────────────────

pub fn strip_library_root_dir(config: &mut Config) {
    config.library.root_dir = String::new();
}

/// Export config to a local TOML file (strips library root dir).
pub fn export_config_to_file(config: &Config, path: &Path) -> Result<(), String> {
    let mut cfg = config.clone();
    strip_library_root_dir(&mut cfg);
    let content = toml::to_string_pretty(&cfg).map_err(|e| e.to_string())?;
    std::fs::write(path, content).map_err(|e| e.to_string())?;
    Ok(())
}

/// Validate and copy a TOML file into the app's config path.
pub fn import_config_from_file(src: &Path, dst: &Path) -> Result<(), String> {
    let content = std::fs::read_to_string(src).map_err(|e| e.to_string())?;
    let _: Config =
        toml::from_str(&content).map_err(|e| format!("配置文件格式错误: {}", e))?;
    if let Some(parent) = dst.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    std::fs::write(dst, content).map_err(|e| e.to_string())?;
    Ok(())
}

// ── S3 variants ──────────────────────────────────────────────────

pub async fn export_knowledge_base_s3(
    pool: &SqlitePool,
    sync: &SyncConfig,
) -> Result<(), String> {
    let json = serialize_knowledge_base(pool).await?;
    s3::s3_put(sync, S3_KEY_KB, json.as_bytes()).await
}

pub async fn import_knowledge_base_s3(
    pool: &SqlitePool,
    sync: &SyncConfig,
) -> Result<String, String> {
    let bytes = s3::s3_get(sync, S3_KEY_KB).await?;
    let json = String::from_utf8(bytes).map_err(|e| format!("数据编码错误: {}", e))?;
    import_knowledge_base_data(pool, &json).await
}

pub async fn export_config_s3(config: &Config, sync: &SyncConfig) -> Result<(), String> {
    let mut cfg = config.clone();
    strip_library_root_dir(&mut cfg);
    let content = toml::to_string_pretty(&cfg).map_err(|e| e.to_string())?;
    s3::s3_put(sync, S3_KEY_CONFIG, content.as_bytes()).await
}

pub async fn import_config_s3(sync: &SyncConfig, config_path: &Path) -> Result<(), String> {
    let bytes = s3::s3_get(sync, S3_KEY_CONFIG).await?;
    let content = String::from_utf8(bytes).map_err(|e| format!("数据编码错误: {}", e))?;
    let _: Config =
        toml::from_str(&content).map_err(|e| format!("配置文件格式错误: {}", e))?;
    if let Some(parent) = config_path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    std::fs::write(config_path, content).map_err(|e| e.to_string())?;
    Ok(())
}

pub async fn test_s3_connection(sync: &SyncConfig) -> Result<String, String> {
    match s3::s3_head(sync, S3_KEY_KB).await {
        Ok(true) => Ok("连接成功，云端已有知识库数据".to_string()),
        Ok(false) => Ok("连接成功，云端暂无数据".to_string()),
        Err(e) => Err(e),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    async fn setup() -> SqlitePool {
        let pool = SqlitePool::connect(":memory:").await.unwrap();
        crate::infra::db::MIGRATOR.run(&pool).await.unwrap();
        pool
    }

    #[tokio::test]
    async fn export_empty_db() {
        let pool = setup().await;
        let json = serialize_knowledge_base(&pool).await.unwrap();
        let data: KnowledgeBaseExport = serde_json::from_str(&json).unwrap();
        assert_eq!(data.version, "3.0.0");
        assert!(data.entries.is_empty());
        assert!(data.entry_tags.is_empty());
        assert!(data.relations.is_empty());
    }

    #[tokio::test]
    async fn export_roundtrip() {
        let pool = setup().await;

        sqlx::query("INSERT INTO entries (id, name, wiki) VALUES (1, 'Antonioni', '# Bio')")
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query("INSERT INTO entries (id, name) VALUES (2, 'Blow-Up')")
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query("INSERT INTO entry_tags (entry_id, tag) VALUES (1, '导演')")
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query(
            "INSERT INTO relations (id, from_id, to_id, relation_type) VALUES (1, 1, 2, '执导')",
        )
        .execute(&pool)
        .await
        .unwrap();

        let json = serialize_knowledge_base(&pool).await.unwrap();

        let pool2 = setup().await;
        let result = import_knowledge_base_data(&pool2, &json).await.unwrap();
        assert!(result.contains("2 条目"));
        assert!(result.contains("1 标签"));
        assert!(result.contains("1 关系"));

        let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM entries")
            .fetch_one(&pool2)
            .await
            .unwrap();
        assert_eq!(count, 2);

        let tag_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM entry_tags")
            .fetch_one(&pool2)
            .await
            .unwrap();
        assert_eq!(tag_count, 1);

        let rel_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM relations")
            .fetch_one(&pool2)
            .await
            .unwrap();
        assert_eq!(rel_count, 1);
    }

    #[tokio::test]
    async fn import_skips_duplicates() {
        let pool = setup().await;
        sqlx::query("INSERT INTO entries (id, name) VALUES (1, 'Existing')")
            .execute(&pool)
            .await
            .unwrap();

        let json = r#"{
            "version": "3.0.0",
            "exported_at": "2026-01-01",
            "entries": [{"id": 1, "name": "Override", "wiki": "", "created_at": "2026-01-01", "updated_at": "2026-01-01"}],
            "entry_tags": [],
            "relations": []
        }"#;

        let result = import_knowledge_base_data(&pool, json).await.unwrap();
        assert!(result.contains("0 条目"));

        let name: String = sqlx::query_scalar("SELECT name FROM entries WHERE id = 1")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(name, "Existing");
    }

    #[tokio::test]
    async fn import_rejects_old_format() {
        let pool = setup().await;
        let json =
            r#"{"version": "2.0.1", "exported_at": "", "people": [], "genres": [], "films": []}"#;

        let result = import_knowledge_base_data(&pool, json).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("旧版"));
    }

    #[tokio::test]
    async fn import_rejects_invalid_json() {
        let pool = setup().await;
        let result = import_knowledge_base_data(&pool, "not json").await;
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("JSON"));
    }
}
