use blowup_core::subtitle::service::{
    self, AlignResult, SubEntry, SubtitleSearchResult, SubtitleStreamInfo,
};
use std::path::Path;

#[tauri::command]
pub fn parse_subtitle_cmd(path: String) -> Result<Vec<SubEntry>, String> {
    service::parse_subtitle_file(Path::new(&path))
}

#[tauri::command]
pub fn open_subtitle_viewer(app: tauri::AppHandle, file_path: String) -> Result<(), String> {
    let label = crate::common::unique_window_label("subtitle-viewer");
    let url = format!(
        "subtitle-viewer.html?file={}",
        urlencoding::encode(&file_path)
    );
    crate::common::open_child_window(
        &app,
        &label,
        &url,
        "字幕查看器",
        (720.0, 600.0),
        Some((400.0, 300.0)),
    )
}

#[tauri::command]
pub async fn fetch_subtitle_cmd(
    video: String,
    lang: String,
    _api_key: String,
) -> Result<(), String> {
    let cfg = blowup_core::config::load_config();
    service::fetch_subtitle(Path::new(&video), &lang, &cfg)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn align_subtitle_cmd(video: String, srt: String) -> Result<(), String> {
    service::align_subtitle(Path::new(&video), Path::new(&srt))
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn extract_subtitle_cmd(video: String, stream: Option<u32>) -> Result<(), String> {
    service::extract_sub_srt(Path::new(&video), stream)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn list_subtitle_streams_cmd(
    video: String,
) -> Result<Vec<SubtitleStreamInfo>, String> {
    service::list_all_subtitle_stream(Path::new(&video))
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn shift_subtitle_cmd(srt: String, offset_ms: i64) -> Result<(), String> {
    service::shift_srt(Path::new(&srt), offset_ms).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn align_to_audio_cmd(srt: String, audio: String) -> Result<AlignResult, String> {
    service::align_subtitle_to_audio(Path::new(&srt), Path::new(&audio))
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn search_subtitles_cmd(
    video: String,
    lang: String,
    title: Option<String>,
    year: Option<u32>,
    tmdb_id: Option<u64>,
) -> Result<Vec<SubtitleSearchResult>, String> {
    let cfg = blowup_core::config::load_config();
    service::search_with_priority(
        Path::new(&video),
        &lang,
        title.as_deref(),
        year,
        tmdb_id,
        &cfg,
    )
    .await
}

#[tauri::command]
pub async fn download_subtitle_cmd(
    video: String,
    lang: String,
    download_id: String,
) -> Result<(), String> {
    let cfg = blowup_core::config::load_config();
    service::download_by_id(Path::new(&video), &lang, &download_id, &cfg).await
}
