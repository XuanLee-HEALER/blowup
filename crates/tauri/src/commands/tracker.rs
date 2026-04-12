// Re-export so `commands::tracker::TrackerManager` keeps resolving in
// lib.rs for the state registration and background refresh task.
pub use blowup_core::torrent::tracker::{TrackerManager, TrackerStatus};

#[tauri::command]
pub async fn get_tracker_status(
    tm: tauri::State<'_, TrackerManager>,
) -> Result<TrackerStatus, String> {
    Ok(tm.get_status().await)
}

#[tauri::command]
pub async fn refresh_trackers(
    tm: tauri::State<'_, TrackerManager>,
) -> Result<TrackerStatus, String> {
    tm.refresh_auto().await
}

#[tauri::command]
pub async fn add_user_trackers(
    tm: tauri::State<'_, TrackerManager>,
    raw: String,
) -> Result<TrackerStatus, String> {
    tm.add_user_trackers(raw).await
}
