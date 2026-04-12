use blowup_core::library::index::{FileMediaInfo, LibraryIndex};
use blowup_core::media::service::{self, MediaInfo};

#[tauri::command]
pub async fn probe_media(file: String) -> Result<String, String> {
    service::probe_media(&file).await
}

#[tauri::command]
pub async fn probe_media_detail(file_path: String) -> Result<MediaInfo, String> {
    service::probe_media_detail(&file_path).await
}

#[tauri::command]
pub async fn probe_and_cache(
    index: tauri::State<'_, LibraryIndex>,
    tmdb_id: u64,
    filename: String,
) -> Result<FileMediaInfo, String> {
    service::probe_and_cache(index.inner(), tmdb_id, &filename).await
}
