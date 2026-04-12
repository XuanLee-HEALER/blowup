use blowup_core::audio::service::{self, AudioStreamInfo};
use std::path::Path;

#[tauri::command]
pub async fn list_audio_streams_cmd(video: String) -> Result<Vec<AudioStreamInfo>, String> {
    service::list_audio_streams(Path::new(&video)).await
}

#[tauri::command]
pub async fn extract_audio_cmd(
    video: String,
    stream: u32,
    format: String,
) -> Result<String, String> {
    service::extract_audio(Path::new(&video), stream, &format).await
}

/// Must be `async` — sync Tauri commands run on the main thread in v2,
/// and calling `run_on_main_thread` from the main thread deadlocks: the
/// posted closure is queued but never processed because the main thread
/// is busy running this command, so the IPC response never comes back
/// and the waveform window is built too late (or never).
#[tauri::command]
pub async fn open_waveform_window(app: tauri::AppHandle, file_path: String) -> Result<(), String> {
    let label = crate::common::unique_window_label("waveform");
    let url = format!("waveform.html?file={}", urlencoding::encode(&file_path));
    crate::common::open_child_window(&app, &label, &url, "音频波形", (800.0, 300.0), None)
}
