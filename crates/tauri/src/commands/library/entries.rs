use blowup_core::entries::model::{EntryDetail, EntrySummary};
use blowup_core::entries::service;
use blowup_core::infra::events::{DomainEvent, EventBus};
use sqlx::SqlitePool;

#[tauri::command]
pub async fn list_entries(
    query: Option<String>,
    tag: Option<String>,
    pool: tauri::State<'_, SqlitePool>,
) -> Result<Vec<EntrySummary>, String> {
    service::list_entries(pool.inner(), query.as_deref(), tag.as_deref()).await
}

#[tauri::command]
pub async fn get_entry(id: i64, pool: tauri::State<'_, SqlitePool>) -> Result<EntryDetail, String> {
    service::get_entry(pool.inner(), id).await
}

#[tauri::command]
pub async fn create_entry(
    events: tauri::State<'_, EventBus>,
    name: String,
    pool: tauri::State<'_, SqlitePool>,
) -> Result<i64, String> {
    let id = service::create_entry(pool.inner(), &name).await?;
    events.publish(DomainEvent::EntriesChanged);
    Ok(id)
}

#[tauri::command]
pub async fn update_entry_name(
    events: tauri::State<'_, EventBus>,
    id: i64,
    name: String,
    pool: tauri::State<'_, SqlitePool>,
) -> Result<(), String> {
    service::update_entry_name(pool.inner(), id, &name).await?;
    events.publish(DomainEvent::EntriesChanged);
    Ok(())
}

#[tauri::command]
pub async fn update_entry_wiki(
    events: tauri::State<'_, EventBus>,
    id: i64,
    wiki: String,
    pool: tauri::State<'_, SqlitePool>,
) -> Result<(), String> {
    service::update_entry_wiki(pool.inner(), id, &wiki).await?;
    events.publish(DomainEvent::EntriesChanged);
    Ok(())
}

#[tauri::command]
pub async fn delete_entry(
    events: tauri::State<'_, EventBus>,
    id: i64,
    pool: tauri::State<'_, SqlitePool>,
) -> Result<(), String> {
    service::delete_entry(pool.inner(), id).await?;
    events.publish(DomainEvent::EntriesChanged);
    Ok(())
}

#[tauri::command]
pub async fn add_entry_tag(
    events: tauri::State<'_, EventBus>,
    entry_id: i64,
    tag: String,
    pool: tauri::State<'_, SqlitePool>,
) -> Result<(), String> {
    service::add_entry_tag(pool.inner(), entry_id, &tag).await?;
    events.publish(DomainEvent::EntriesChanged);
    Ok(())
}

#[tauri::command]
pub async fn remove_entry_tag(
    events: tauri::State<'_, EventBus>,
    entry_id: i64,
    tag: String,
    pool: tauri::State<'_, SqlitePool>,
) -> Result<(), String> {
    service::remove_entry_tag(pool.inner(), entry_id, &tag).await?;
    events.publish(DomainEvent::EntriesChanged);
    Ok(())
}

#[tauri::command]
pub async fn list_all_tags(pool: tauri::State<'_, SqlitePool>) -> Result<Vec<String>, String> {
    service::list_all_tags(pool.inner()).await
}

#[tauri::command]
pub async fn add_relation(
    events: tauri::State<'_, EventBus>,
    from_id: i64,
    to_id: i64,
    relation_type: String,
    pool: tauri::State<'_, SqlitePool>,
) -> Result<i64, String> {
    let id = service::add_relation(pool.inner(), from_id, to_id, &relation_type).await?;
    events.publish(DomainEvent::EntriesChanged);
    Ok(id)
}

#[tauri::command]
pub async fn remove_relation(
    events: tauri::State<'_, EventBus>,
    id: i64,
    pool: tauri::State<'_, SqlitePool>,
) -> Result<(), String> {
    service::remove_relation(pool.inner(), id).await?;
    events.publish(DomainEvent::EntriesChanged);
    Ok(())
}

#[tauri::command]
pub async fn list_relation_types(
    pool: tauri::State<'_, SqlitePool>,
) -> Result<Vec<String>, String> {
    service::list_relation_types(pool.inner()).await
}
