use axum::extract::{Path, Query, State};
use axum::{Json, Router, routing::delete, routing::get, routing::post, routing::put};
use blowup_core::entries::graph;
use blowup_core::entries::model::{EntryDetail, EntrySummary, GraphData};
use blowup_core::entries::service;
use blowup_core::infra::events::DomainEvent;
use serde::Deserialize;

use crate::error::ApiResult;
use crate::state::AppState;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/entries", get(list_entries).post(create_entry))
        .route("/entries/tags", get(list_all_tags))
        .route("/entries/relation-types", get(list_relation_types))
        .route("/entries/graph", get(get_graph))
        .route("/entries/relations", post(add_relation))
        .route("/entries/relations/{id}", delete(remove_relation))
        .route("/entries/{id}", get(get_entry).delete(delete_entry))
        .route("/entries/{id}/name", put(update_name))
        .route("/entries/{id}/wiki", put(update_wiki))
        .route(
            "/entries/{id}/tags",
            post(add_tag).delete(remove_tag_query),
        )
}

#[derive(Deserialize)]
pub struct ListQuery {
    pub query: Option<String>,
    pub tag: Option<String>,
}

async fn list_entries(
    State(state): State<AppState>,
    Query(q): Query<ListQuery>,
) -> ApiResult<Json<Vec<EntrySummary>>> {
    let entries = service::list_entries(&state.db, q.query.as_deref(), q.tag.as_deref())
        .await
        .map_err(crate::error::ApiError::Internal)?;
    Ok(Json(entries))
}

async fn get_entry(
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> ApiResult<Json<EntryDetail>> {
    let detail = service::get_entry(&state.db, id)
        .await
        .map_err(crate::error::ApiError::from)?;
    Ok(Json(detail))
}

#[derive(Deserialize)]
pub struct NameBody {
    pub name: String,
}

async fn create_entry(
    State(state): State<AppState>,
    Json(req): Json<NameBody>,
) -> ApiResult<Json<i64>> {
    let id = service::create_entry(&state.db, &req.name)
        .await
        .map_err(crate::error::ApiError::Internal)?;
    state.events.publish(DomainEvent::EntriesChanged);
    Ok(Json(id))
}

async fn update_name(
    State(state): State<AppState>,
    Path(id): Path<i64>,
    Json(req): Json<NameBody>,
) -> ApiResult<()> {
    service::update_entry_name(&state.db, id, &req.name)
        .await
        .map_err(crate::error::ApiError::Internal)?;
    state.events.publish(DomainEvent::EntriesChanged);
    Ok(())
}

#[derive(Deserialize)]
pub struct WikiBody {
    pub wiki: String,
}

async fn update_wiki(
    State(state): State<AppState>,
    Path(id): Path<i64>,
    Json(req): Json<WikiBody>,
) -> ApiResult<()> {
    service::update_entry_wiki(&state.db, id, &req.wiki)
        .await
        .map_err(crate::error::ApiError::Internal)?;
    state.events.publish(DomainEvent::EntriesChanged);
    Ok(())
}

async fn delete_entry(State(state): State<AppState>, Path(id): Path<i64>) -> ApiResult<()> {
    service::delete_entry(&state.db, id)
        .await
        .map_err(crate::error::ApiError::Internal)?;
    state.events.publish(DomainEvent::EntriesChanged);
    Ok(())
}

#[derive(Deserialize)]
pub struct TagBody {
    pub tag: String,
}

async fn add_tag(
    State(state): State<AppState>,
    Path(entry_id): Path<i64>,
    Json(req): Json<TagBody>,
) -> ApiResult<()> {
    service::add_entry_tag(&state.db, entry_id, &req.tag)
        .await
        .map_err(crate::error::ApiError::Internal)?;
    state.events.publish(DomainEvent::EntriesChanged);
    Ok(())
}

/// Delete a tag via a query string (`?tag=...`) on DELETE /entries/{id}/tags.
async fn remove_tag_query(
    State(state): State<AppState>,
    Path(entry_id): Path<i64>,
    Query(q): Query<TagBody>,
) -> ApiResult<()> {
    service::remove_entry_tag(&state.db, entry_id, &q.tag)
        .await
        .map_err(crate::error::ApiError::Internal)?;
    state.events.publish(DomainEvent::EntriesChanged);
    Ok(())
}

async fn list_all_tags(State(state): State<AppState>) -> ApiResult<Json<Vec<String>>> {
    let tags = service::list_all_tags(&state.db)
        .await
        .map_err(crate::error::ApiError::Internal)?;
    Ok(Json(tags))
}

#[derive(Deserialize)]
pub struct RelationBody {
    pub from_id: i64,
    pub to_id: i64,
    pub relation_type: String,
}

async fn add_relation(
    State(state): State<AppState>,
    Json(req): Json<RelationBody>,
) -> ApiResult<Json<i64>> {
    let id = service::add_relation(&state.db, req.from_id, req.to_id, &req.relation_type)
        .await
        .map_err(crate::error::ApiError::Internal)?;
    state.events.publish(DomainEvent::EntriesChanged);
    Ok(Json(id))
}

async fn remove_relation(State(state): State<AppState>, Path(id): Path<i64>) -> ApiResult<()> {
    service::remove_relation(&state.db, id)
        .await
        .map_err(crate::error::ApiError::Internal)?;
    state.events.publish(DomainEvent::EntriesChanged);
    Ok(())
}

async fn list_relation_types(State(state): State<AppState>) -> ApiResult<Json<Vec<String>>> {
    let types = service::list_relation_types(&state.db)
        .await
        .map_err(crate::error::ApiError::Internal)?;
    Ok(Json(types))
}

#[derive(Deserialize)]
pub struct GraphQuery {
    pub center_id: Option<i64>,
}

async fn get_graph(
    State(state): State<AppState>,
    Query(q): Query<GraphQuery>,
) -> ApiResult<Json<GraphData>> {
    let data = graph::get_graph_data(&state.db, q.center_id)
        .await
        .map_err(crate::error::ApiError::Internal)?;
    Ok(Json(data))
}
