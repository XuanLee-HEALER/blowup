//! Audio stream listing and extraction via ffprobe/ffmpeg.
//! The waveform rendering window is Tauri-bound and stays in blowup-tauri.

use crate::infra::ffmpeg::FfmpegTool;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// Sample rate for pre-computed waveform peaks (100 samples/sec is
/// plenty for 800-2000 px wide visualizations and keeps the payload
/// tiny — a 2-hour track is ~2.8 MB of f32le).
pub const PEAKS_SAMPLE_RATE: u32 = 100;

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
        _ => "mka",
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

/// Sidecar filename for cached waveform peaks.
fn peaks_cache_path(audio: &Path) -> PathBuf {
    let mut p = audio.to_path_buf();
    let mut name = audio
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| "audio".to_string());
    name.push_str(".peaks.bin");
    p.set_file_name(name);
    p
}

/// Cache is fresh iff both files exist and the sidecar's mtime is at
/// least as recent as the source's. Any missing mtime or metadata
/// failure counts as stale so the caller re-generates.
fn peaks_cache_is_fresh(cache: &Path, source: &Path) -> bool {
    let Ok(c_meta) = std::fs::metadata(cache) else {
        return false;
    };
    let Ok(s_meta) = std::fs::metadata(source) else {
        return false;
    };
    match (c_meta.modified(), s_meta.modified()) {
        (Ok(c), Ok(s)) => c >= s,
        _ => false,
    }
}

/// Write bytes to `path` via a unique tmp file + rename, so two
/// concurrent callers never truncate each other's output mid-read.
fn atomic_write(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    let unique = format!(
        ".tmp.{}.{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0)
    );
    let mut tmp = path.to_path_buf();
    let name = path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| "cache.bin".to_string());
    tmp.set_file_name(format!("{name}{unique}"));
    std::fs::write(&tmp, bytes)?;
    std::fs::rename(&tmp, path)
}

/// Return pre-computed waveform peak samples for `file` as raw
/// little-endian float32 bytes (100 samples/sec, mono, normalized
/// to [-1, 1] by ffmpeg).
///
/// First call per audio file runs ffmpeg to decode + downsample +
/// write a `{audio}.peaks.bin` sidecar (one-time cost, seconds).
/// Subsequent calls read the sidecar from disk (milliseconds).
///
/// The frontend passes these bytes to `WaveSurfer` via its `peaks`
/// option so WaveSurfer never invokes the browser's `decodeAudioData`
/// on the full audio file — which was the source of the
/// waveform-window-stuck-at-loading bug for 342 MB AAC 5.1 streams.
pub async fn extract_audio_peaks(file: &Path) -> Result<Vec<u8>, String> {
    if !file.exists() {
        return Err(format!("文件不存在: {}", file.display()));
    }

    let cache = peaks_cache_path(file);
    if peaks_cache_is_fresh(&cache, file) {
        return std::fs::read(&cache).map_err(|e| format!("读取峰值缓存失败: {e}"));
    }

    let file_str = file.to_string_lossy().to_string();
    let sample_rate = PEAKS_SAMPLE_RATE.to_string();
    let args: Vec<&str> = vec![
        "-v",
        "error",
        "-i",
        &file_str,
        "-ac",
        "1", // mono
        "-ar",
        &sample_rate, // downsample to 100 Hz
        "-f",
        "f32le", // raw float32 little-endian
        "-",     // stdout
    ];

    let bytes = FfmpegTool::Ffmpeg
        .exec_binary_output(&args)
        .await
        .map_err(|e| format!("生成波形峰值失败: {e}"))?;

    // Persist cache via atomic write (tmp + rename) so parallel
    // callers never observe a partially-written file. Log on write
    // failure — we still have the bytes in hand.
    if let Err(e) = atomic_write(&cache, &bytes) {
        tracing::warn!(error = %e, path = %cache.display(), "failed to write peaks cache");
    }

    Ok(bytes)
}

/// Extract an audio stream from a video file.
///
/// - `stream`: audio stream index (0-based among audio streams)
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
