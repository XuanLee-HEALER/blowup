//! Media probing via ffprobe + cached results in `LibraryIndex`.
//! HLS transmux will land here later (see docs/REFACTOR.md D11).

use crate::infra::ffmpeg::FfmpegTool;
use crate::library::index::{FileMediaInfo, FileStreamInfo, LibraryIndex};
use serde::Serialize;

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

/// Returns raw JSON output from `ffprobe -show_streams` for the given file.
pub async fn probe_media(file: &str) -> Result<String, String> {
    let args = vec![
        "-v".to_string(),
        "quiet".to_string(),
        "-print_format".to_string(),
        "json".to_string(),
        "-show_streams".to_string(),
        "--".to_string(),
        file.to_string(),
    ];
    let (stdout, _) = FfmpegTool::Ffprobe
        .exec_with_options(None::<&str>, Some(args))
        .await
        .map_err(|e| e.to_string())?;
    Ok(stdout)
}

/// Probe with `-show_format -show_streams` and parse into a typed `MediaInfo`.
pub async fn probe_media_detail(file_path: &str) -> Result<MediaInfo, String> {
    let args: Vec<String> = [
        "-v",
        "quiet",
        "-print_format",
        "json",
        "-show_format",
        "-show_streams",
        "--",
        file_path,
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
        file_path: file_path.to_string(),
        file_size,
        duration_secs,
        format_name,
        bit_rate,
        streams,
    })
}

/// Probe a video file under a `LibraryIndex` entry and cache the result.
/// Returns the cached `FileMediaInfo`. If the file already has cached info
/// and `force` is false, returns it without re-probing.
pub async fn probe_and_cache(
    index: &LibraryIndex,
    tmdb_id: u64,
    filename: &str,
) -> Result<FileMediaInfo, String> {
    if let Some(entry) = index.get_entry(tmdb_id)
        && let Some(cached) = entry.media_info.get(filename)
    {
        return Ok(cached.clone());
    }

    let entry = index.get_entry(tmdb_id).ok_or("影片条目未找到")?;
    let full_path = index.root().join(&entry.path).join(filename);
    let full_path_str = full_path.to_string_lossy().to_string();

    let detail = probe_media_detail(&full_path_str).await?;

    let info = FileMediaInfo {
        file_size: detail.file_size,
        duration_secs: detail.duration_secs,
        format_name: detail.format_name,
        bit_rate: detail.bit_rate,
        streams: detail
            .streams
            .into_iter()
            .map(|s| FileStreamInfo {
                index: s.index,
                codec_type: s.codec_type,
                codec_name: s.codec_name,
                width: s.width,
                height: s.height,
                frame_rate: s.frame_rate,
                bit_rate: s.bit_rate,
                channels: s.channels,
                sample_rate: s.sample_rate,
                language: s.language,
                title: s.title,
            })
            .collect(),
    };

    index
        .set_file_media_info(tmdb_id, filename, info.clone())
        .ok_or("影片条目未找到")?;

    Ok(info)
}
