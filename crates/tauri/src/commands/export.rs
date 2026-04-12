use blowup_core::config::{Config, config_path, load_config};
use blowup_core::export::service;
use sqlx::SqlitePool;
use std::path::Path;
use tauri::Emitter;

// Re-export for lib.rs generate_handler (types used by Tauri IPC).
pub use blowup_core::export::service::{EntryRow, EntryTagRow, KnowledgeBaseExport, RelationRow};

#[tauri::command]
pub async fn export_knowledge_base(
    path: String,
    pool: tauri::State<'_, SqlitePool>,
) -> Result<(), String> {
    service::export_knowledge_base_to_file(pool.inner(), Path::new(&path)).await
}

#[tauri::command]
pub async fn import_knowledge_base(
    app: tauri::AppHandle,
    path: String,
    pool: tauri::State<'_, SqlitePool>,
) -> Result<String, String> {
    let result = service::import_knowledge_base_from_file(pool.inner(), Path::new(&path)).await?;
    if let Err(e) = app.emit("entries:changed", ()) {
        tracing::warn!(error = %e, "failed to emit entries:changed");
    }
    Ok(result)
}

#[tauri::command]
pub fn export_config(path: String) -> Result<(), String> {
    let cfg: Config = load_config();
    service::export_config_to_file(&cfg, Path::new(&path))
}

#[tauri::command]
pub fn import_config(app: tauri::AppHandle, path: String) -> Result<(), String> {
    service::import_config_from_file(Path::new(&path), &config_path())?;
    if let Err(e) = app.emit("config:changed", ()) {
        tracing::warn!(error = %e, "failed to emit config:changed");
    }
    Ok(())
}

#[tauri::command]
pub async fn export_knowledge_base_s3(pool: tauri::State<'_, SqlitePool>) -> Result<(), String> {
    let cfg = load_config();
    service::export_knowledge_base_s3(pool.inner(), &cfg.sync).await
}

#[tauri::command]
pub async fn import_knowledge_base_s3(
    app: tauri::AppHandle,
    pool: tauri::State<'_, SqlitePool>,
) -> Result<String, String> {
    let cfg = load_config();
    let result = service::import_knowledge_base_s3(pool.inner(), &cfg.sync).await?;
    if let Err(e) = app.emit("entries:changed", ()) {
        tracing::warn!(error = %e, "failed to emit entries:changed");
    }
    Ok(result)
}

#[tauri::command]
pub async fn export_config_s3() -> Result<(), String> {
    let cfg = load_config();
    let sync = cfg.sync.clone();
    service::export_config_s3(&cfg, &sync).await
}

#[tauri::command]
pub async fn import_config_s3(app: tauri::AppHandle) -> Result<(), String> {
    let cfg = load_config();
    service::import_config_s3(&cfg.sync, &config_path()).await?;
    if let Err(e) = app.emit("config:changed", ()) {
        tracing::warn!(error = %e, "failed to emit config:changed");
    }
    Ok(())
}

#[tauri::command]
pub async fn test_s3_connection() -> Result<String, String> {
    let cfg = load_config();
    service::test_s3_connection(&cfg.sync).await
}
