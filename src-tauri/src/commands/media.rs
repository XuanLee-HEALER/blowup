// src-tauri/src/commands/media.rs
use crate::ffmpeg::FfmpegTool;

/// Returns JSON output from ffprobe for the given file (streams info).
#[tauri::command]
pub async fn probe_media(file: String) -> std::result::Result<String, String> {
    let args = vec![
        "-v".to_string(),
        "quiet".to_string(),
        "-print_format".to_string(),
        "json".to_string(),
        "-show_streams".to_string(),
        "--".to_string(),
        file,
    ];
    let (stdout, _) = FfmpegTool::Ffprobe
        .exec_with_options(None::<&str>, Some(args))
        .await
        .map_err(|e| e.to_string())?;
    Ok(stdout)
}
