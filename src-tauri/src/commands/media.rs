// src-tauri/src/commands/media.rs
use crate::ffmpeg::FfmpegTool;
use serde::Serialize;
use crate::config::load_config;

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

#[derive(Serialize)]
pub struct MediaInfo {
    pub file_path: String,
    pub file_size: Option<i64>,
    pub duration_secs: Option<f64>,
    pub format_name: Option<String>,
    pub bit_rate: Option<i64>,
    pub streams: Vec<StreamInfo>,
}

#[derive(Serialize)]
pub struct StreamInfo {
    pub index: i64,
    pub codec_type: String,
    pub codec_name: String,
    pub width: Option<i64>,
    pub height: Option<i64>,
    pub frame_rate: Option<String>,
    pub bit_rate: Option<i64>,
    pub channels: Option<i64>,
    pub sample_rate: Option<String>,
    pub language: Option<String>,
    pub title: Option<String>,
}

#[tauri::command]
pub async fn probe_media_detail(file_path: String) -> Result<MediaInfo, String> {
    let args: Vec<String> = vec![
        "-v", "quiet", "-print_format", "json", "-show_format", "-show_streams", "--", &file_path,
    ]
    .iter()
    .map(|s| s.to_string())
    .collect();

    let (stdout, _) = FfmpegTool::Ffprobe
        .exec_with_options(None::<&str>, Some(args))
        .await
        .map_err(|e| e.to_string())?;

    let json: serde_json::Value =
        serde_json::from_str(&stdout).map_err(|e| format!("ffprobe parse error: {}", e))?;

    let format = &json["format"];
    let file_size = format["size"].as_str().and_then(|s| s.parse().ok());
    let duration_secs = format["duration"].as_str().and_then(|s| s.parse().ok());
    let format_name = format["format_name"].as_str().map(String::from);
    let bit_rate = format["bit_rate"].as_str().and_then(|s| s.parse().ok());

    let mut streams = Vec::new();
    if let Some(arr) = json["streams"].as_array() {
        for s in arr {
            streams.push(StreamInfo {
                index: s["index"].as_i64().unwrap_or(0),
                codec_type: s["codec_type"].as_str().unwrap_or("unknown").to_string(),
                codec_name: s["codec_name"].as_str().unwrap_or("unknown").to_string(),
                width: s["width"].as_i64(),
                height: s["height"].as_i64(),
                frame_rate: s["r_frame_rate"].as_str().map(String::from),
                bit_rate: s["bit_rate"].as_str().and_then(|s| s.parse().ok()),
                channels: s["channels"].as_i64(),
                sample_rate: s["sample_rate"].as_str().map(String::from),
                language: s["tags"]["language"].as_str().map(String::from),
                title: s["tags"]["title"].as_str().map(String::from),
            });
        }
    }

    Ok(MediaInfo {
        file_path,
        file_size,
        duration_secs,
        format_name,
        bit_rate,
        streams,
    })
}

#[tauri::command]
pub async fn open_in_player(file_path: String) -> Result<(), String> {
    let config = load_config();
    let player = config.tools.player.clone();

    if !player.is_empty() && which::which(&player).is_ok() {
        std::process::Command::new(&player)
            .arg(&file_path)
            .spawn()
            .map_err(|e| format!("启动播放器失败: {}", e))?;
        return Ok(());
    }

    open_with_system_default(&file_path)
}

fn open_with_system_default(file_path: &str) -> Result<(), String> {
    #[cfg(target_family = "windows")]
    {
        std::process::Command::new("cmd")
            .args(["/c", "start", "", file_path])
            .spawn()
            .map_err(|e| format!("打开文件失败: {}", e))?;
    }
    #[cfg(target_os = "macos")]
    {
        std::process::Command::new("open")
            .arg(file_path)
            .spawn()
            .map_err(|e| format!("打开文件失败: {}", e))?;
    }
    #[cfg(all(target_family = "unix", not(target_os = "macos")))]
    {
        std::process::Command::new("xdg-open")
            .arg(file_path)
            .spawn()
            .map_err(|e| format!("打开文件失败: {}", e))?;
    }
    Ok(())
}
