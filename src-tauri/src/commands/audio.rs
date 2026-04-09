use crate::ffmpeg::FfmpegTool;
use serde::{Deserialize, Serialize};
use std::path::Path;

// ── Types ────────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
struct FfprobeOutput {
    streams: Vec<FfprobeStream>,
}

#[derive(Debug, Deserialize)]
struct FfprobeStream {
    index: u32,
    codec_name: String,
    channels: Option<u32>,
    sample_rate: Option<String>,
    bit_rate: Option<String>,
    tags: Option<FfprobeTags>,
}

#[derive(Debug, Deserialize)]
struct FfprobeTags {
    language: Option<String>,
    title: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct AudioStreamInfo {
    pub index: u32,
    pub codec_name: String,
    pub channels: Option<u32>,
    pub sample_rate: Option<String>,
    pub bit_rate: Option<String>,
    pub language: Option<String>,
    pub title: Option<String>,
}

// ── Core functions ───────────────────────────────────────────────

pub async fn list_audio_streams(file: &Path) -> Result<Vec<AudioStreamInfo>, String> {
    if !file.exists() {
        return Err(format!("文件不存在: {}", file.display()));
    }

    let args: Vec<String> = vec![
        "-v",
        "quiet",
        "-print_format",
        "json",
        "-show_streams",
        "-select_streams",
        "a",
        "--",
    ]
    .into_iter()
    .map(String::from)
    .chain(std::iter::once(file.to_string_lossy().to_string()))
    .collect();

    let (stdout, _) = FfmpegTool::Ffprobe
        .exec_with_options(None::<&str>, Some(args))
        .await
        .map_err(|e| e.to_string())?;

    if stdout.is_empty() {
        return Ok(vec![]);
    }

    let output: FfprobeOutput =
        serde_json::from_str(&stdout).map_err(|e| format!("解析 ffprobe 输出失败: {e}"))?;

    Ok(output
        .streams
        .into_iter()
        .map(|s| AudioStreamInfo {
            index: s.index,
            codec_name: s.codec_name,
            channels: s.channels,
            sample_rate: s.sample_rate,
            bit_rate: s.bit_rate,
            language: s.tags.as_ref().and_then(|t| t.language.clone()),
            title: s.tags.as_ref().and_then(|t| t.title.clone()),
        })
        .collect())
}

/// Map audio codec to a suitable container extension when using `-c copy`.
fn codec_to_ext(codec: &str) -> &str {
    match codec {
        "aac" => "m4a",
        "mp3" | "libmp3lame" => "mp3",
        "flac" => "flac",
        "opus" | "libopus" => "ogg",
        "vorbis" | "libvorbis" => "ogg",
        "pcm_s16le" | "pcm_s24le" | "pcm_f32le" => "wav",
        "ac3" | "eac3" => "ac3",
        "dts" => "dts",
        _ => "mka", // Matroska audio as fallback
    }
}

/// Map user-selected format to ffmpeg output extension.
fn format_to_ext(format: &str) -> &str {
    match format {
        "mp3" => "mp3",
        "aac" => "m4a",
        "flac" => "flac",
        "opus" => "ogg",
        "wav" => "wav",
        _ => "mka",
    }
}

/// Extract an audio stream from a video file.
///
/// - `stream`: audio stream index (relative, 0-based among audio streams)
/// - `format`: "copy" for original codec, or "mp3"/"aac"/"flac"/"opus"/"wav"
///
/// Returns the output file path.
pub async fn extract_audio(file: &Path, stream: u32, format: &str) -> Result<String, String> {
    if !file.exists() {
        return Err(format!("文件不存在: {}", file.display()));
    }

    let file_str = file.to_string_lossy().to_string();
    let stem = file.file_stem().unwrap_or_default().to_string_lossy();
    let map_spec = format!("0:a:{stream}");

    // Determine output extension and whether to copy codec
    let (ext, copy_codec) = if format == "copy" {
        let streams = list_audio_streams(file).await?;
        let codec = streams
            .get(stream as usize)
            .map(|s| s.codec_name.as_str())
            .unwrap_or("unknown");
        (codec_to_ext(codec).to_string(), true)
    } else {
        (format_to_ext(format).to_string(), false)
    };

    let out_name = format!("{stem}_audio_{stream}.{ext}");
    let out_path = file.parent().unwrap_or(Path::new(".")).join(&out_name);
    let out_str = out_path.to_string_lossy().to_string();

    let mut args: Vec<String> = vec!["-i".into(), file_str, "-map".into(), map_spec];
    if copy_codec {
        args.extend(["-c".into(), "copy".into()]);
    }
    args.extend(["-y".into(), out_str.clone()]);

    let arg_refs: Vec<&str> = args.iter().map(|s| s.as_str()).collect();
    FfmpegTool::Ffmpeg
        .exec_with_options(None::<&str>, Some(arg_refs))
        .await
        .map_err(|e| format!("音频提取失败: {e}"))?;

    Ok(out_str)
}

// ── Tauri commands ───────────────────────────────────────────────

#[tauri::command]
pub async fn list_audio_streams_cmd(video: String) -> Result<Vec<AudioStreamInfo>, String> {
    list_audio_streams(Path::new(&video)).await
}

#[tauri::command]
pub async fn extract_audio_cmd(
    video: String,
    stream: u32,
    format: String,
) -> Result<String, String> {
    extract_audio(Path::new(&video), stream, &format).await
}

#[tauri::command]
pub fn open_waveform_window(app: tauri::AppHandle, file_path: String) -> Result<(), String> {
    let label = crate::common::unique_window_label("waveform");
    let url = format!("waveform.html?file={}", urlencoding::encode(&file_path));
    crate::common::open_child_window(&app, &label, &url, "音频波形", (800.0, 300.0), None)
}
