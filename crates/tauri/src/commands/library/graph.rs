use blowup_core::entries::graph;
use blowup_core::entries::model::GraphData;
use sqlx::SqlitePool;

#[tauri::command]
pub async fn get_graph_data(
    center_id: Option<i64>,
    pool: tauri::State<'_, SqlitePool>,
) -> Result<GraphData, String> {
    graph::get_graph_data(pool.inner(), center_id).await
}
