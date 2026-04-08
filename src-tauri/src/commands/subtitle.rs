// src-tauri/src/commands/subtitle.rs
//
// OpenSubtitles.com REST API + alass alignment + SRT shift + ffmpeg extraction

use crate::config::Config;
use crate::error::SubError;
use crate::ffmpeg::{FfmpegError, FfmpegTool};
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;
use std::process::Command;
use std::sync::Mutex;
use std::time::Instant;
use which::which;

// ── OpenSubtitles REST API ────────────────────────────────────────────────

const API_BASE: &str = "https://api.opensubtitles.com/api/v1";
const USER_AGENT: &str = "blowup v2.0.2";
/// JWT token cache: (token, created_at). Token is valid for 24h,
/// we refresh after 23h to avoid edge-case expiry during a request.
const TOKEN_TTL_SECS: u64 = 23 * 3600;
static TOKEN_CACHE: Mutex<Option<(String, Instant)>> = Mutex::new(None);

// -- Response types --

#[derive(Debug, Deserialize)]
struct OsSearchResponse {
    data: Vec<OsSearchResult>,
}

#[derive(Debug, Deserialize)]
struct OsSearchResult {
    attributes: OsSubAttributes,
}

#[derive(Debug, Deserialize)]
struct OsSubAttributes {
    release: Option<String>,
    download_count: i64,
    files: Vec<OsSubFile>,
}

#[derive(Debug, Deserialize)]
struct OsSubFile {
    file_id: i64,
    file_name: String,
}

#[derive(Debug, Deserialize)]
struct OsLoginResponse {
    token: String,
}

#[derive(Debug, Serialize)]
struct OsDownloadRequest {
    file_id: i64,
}

#[derive(Debug, Deserialize)]
struct OsDownloadResponse {
    link: String,
    remaining: i64,
}

/// Build a reqwest client with the API key and User-Agent headers.
fn os_client(api_key: &str) -> Result<reqwest::Client, SubError> {
    use reqwest::header::{HeaderMap, HeaderValue};
    let mut headers = HeaderMap::new();
    headers.insert(
        "Api-Key",
        HeaderValue::from_str(api_key)
            .map_err(|e| SubError::InvalidSrt(format!("invalid API key header: {e}")))?,
    );
    headers.insert("Content-Type", HeaderValue::from_static("application/json"));
    headers.insert("Accept", HeaderValue::from_static("application/json"));
    reqwest::Client::builder()
        .user_agent(USER_AGENT)
        .default_headers(headers)
        .build()
        .map_err(SubError::HttpFailed)
}

/// Search subtitles via REST API.
async fn os_search(
    client: &reqwest::Client,
    query: &str,
    lang: &str,
) -> Result<Vec<OsSearchResult>, SubError> {
    let url = format!("{API_BASE}/subtitles");
    let resp = client
        .get(&url)
        .query(&[("query", query), ("languages", lang)])
        .send()
        .await?;

    if !resp.status().is_success() {
        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        return Err(SubError::InvalidSrt(format!(
            "OpenSubtitles search failed ({status}): {body}"
        )));
    }

    let search: OsSearchResponse = resp.json().await?;
    Ok(search.data)
}

/// Login to get JWT token (needed for downloads).
async fn os_login(
    client: &reqwest::Client,
    username: &str,
    password: &str,
) -> Result<String, SubError> {
    let url = format!("{API_BASE}/login");
    let body = serde_json::json!({
        "username": username,
        "password": password,
    });

    let resp = client.post(&url).json(&body).send().await?;

    if !resp.status().is_success() {
        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        return Err(SubError::InvalidSrt(format!(
            "OpenSubtitles login failed ({status}): {body}"
        )));
    }

    let login: OsLoginResponse = resp.json().await?;
    Ok(login.token)
}

/// Request a download link for a subtitle file.
/// If `token` is provided, uses authenticated download (higher quota).
/// Otherwise tries unauthenticated download (5 per IP per day).
async fn os_download(
    client: &reqwest::Client,
    token: Option<&str>,
    file_id: i64,
) -> Result<OsDownloadResponse, SubError> {
    let url = format!("{API_BASE}/download");
    let body = OsDownloadRequest { file_id };

    let mut req = client.post(&url);
    if let Some(t) = token {
        req = req.header("Authorization", format!("Bearer {t}"));
    }
    let resp = req.json(&body).send().await?;

    if !resp.status().is_success() {
        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        return Err(SubError::InvalidSrt(format!(
            "OpenSubtitles download failed ({status}): {body}"
        )));
    }

    resp.json().await.map_err(SubError::HttpFailed)
}

/// Fetch and save a subtitle file from OpenSubtitles.
pub async fn fetch_subtitle(video: &Path, lang: &str, cfg: &Config) -> Result<(), SubError> {
    let api_key = &cfg.opensubtitles.api_key;
    if api_key.is_empty() {
        return Err(SubError::InvalidSrt(
            "OpenSubtitles API key not configured. Set it in Settings → OpenSubtitles.".into(),
        ));
    }

    let client = os_client(api_key)?;

    // Build search query from filename
    let stem = video
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("")
        .to_string();
    let query = clean_query(&stem);

    // Search
    let results = os_search(&client, &query, lang).await?;
    if results.is_empty() {
        return Err(SubError::NoSubtitleFound);
    }

    // Pick best result (first file of first result — sorted by relevance)
    let best = &results[0];
    let file = best
        .attributes
        .files
        .first()
        .ok_or(SubError::NoSubtitleFound)?;

    // Login if credentials are configured (higher download quota).
    // Token is cached in memory for ~23h (JWT valid for 24h).
    let username = &cfg.opensubtitles.username;
    let password = &cfg.opensubtitles.password;
    let token = if !username.is_empty() && !password.is_empty() {
        // Check cache first
        let cached = {
            let guard = TOKEN_CACHE.lock().unwrap();
            guard
                .as_ref()
                .filter(|(_, created)| created.elapsed().as_secs() < TOKEN_TTL_SECS)
                .map(|(t, _)| t.clone())
        };
        if let Some(t) = cached {
            tracing::debug!("OpenSubtitles: using cached token");
            Some(t)
        } else {
            match os_login(&client, username, password).await {
                Ok(t) => {
                    tracing::info!("OpenSubtitles: logged in as {username}");
                    *TOKEN_CACHE.lock().unwrap() = Some((t.clone(), Instant::now()));
                    Some(t)
                }
                Err(e) => {
                    tracing::warn!("OpenSubtitles login failed, trying without auth: {e}");
                    None
                }
            }
        }
    } else {
        None
    };

    // Request download link
    let dl = os_download(&client, token.as_deref(), file.file_id).await?;
    tracing::info!(
        file_name = file.file_name,
        remaining = dl.remaining,
        "subtitle download link obtained"
    );

    // Download the subtitle file (temporary URL, no auth needed)
    let out_path = video.with_extension(format!("{lang}.srt"));
    let resp = reqwest::get(&dl.link).await?;
    if !resp.status().is_success() {
        return Err(SubError::NoSubtitleFound);
    }
    let bytes = resp.bytes().await?;
    std::fs::write(&out_path, &bytes).map_err(SubError::Io)?;

    tracing::info!(
        path = %out_path.display(),
        release = best.attributes.release.as_deref().unwrap_or(""),
        downloads = best.attributes.download_count,
        "subtitle saved"
    );
    Ok(())
}

fn clean_query(stem: &str) -> String {
    let s: String = stem
        .chars()
        .map(|c| {
            if matches!(c, '.' | '-' | '[' | ']') {
                ' '
            } else {
                c
            }
        })
        .collect();
    let mut out = Vec::new();
    for t in s.split_whitespace() {
        if matches!(
            t,
            "1080p" | "720p" | "2160p" | "BluRay" | "WEB" | "x264" | "x265" | "AAC"
        ) {
            break;
        }
        out.push(t);
    }
    if out.is_empty() {
        s.trim().to_string()
    } else {
        out.join(" ")
    }
}

// ── align_subtitle (alass) ───────────────────────────────────────────────

pub fn align_subtitle(video: &Path, srt: &Path, alass_path: Option<&str>) -> Result<(), SubError> {
    let alass = if let Some(p) = alass_path.filter(|s| !s.is_empty()) {
        std::path::PathBuf::from(p)
    } else {
        which("alass")
            .or_else(|_| which("alass-cli"))
            .map_err(|_| SubError::AlassNotFound)?
    };
    align_with_binary(&alass, video, srt)
}

fn align_with_binary(alass: &Path, video: &Path, srt: &Path) -> Result<(), SubError> {
    let backup = srt.with_extension("bak.srt");

    let run_result = Command::new(alass)
        .arg(video)
        .arg(srt)
        .arg(&backup)
        .output()
        .map_err(|e| SubError::AlassFailed(e.to_string()))?;

    if !run_result.status.success() {
        let stderr = String::from_utf8_lossy(&run_result.stderr).to_string();
        return Err(SubError::AlassFailed(stderr));
    }

    std::fs::copy(&backup, srt).map_err(SubError::Io)?;
    Ok(())
}

// ── shift_srt ────────────────────────────────────────────────────────────

pub fn shift_srt(srt_path: &Path, offset_ms: i64) -> Result<(), SubError> {
    let content = fs::read_to_string(srt_path).map_err(SubError::Io)?;
    let shifted = apply_offset(&content, offset_ms)?;
    fs::write(srt_path, shifted).map_err(SubError::Io)?;
    Ok(())
}

fn apply_offset(content: &str, offset_ms: i64) -> Result<String, SubError> {
    let re = Regex::new(r"(\d{2}):(\d{2}):(\d{2}),(\d{3}) --> (\d{2}):(\d{2}):(\d{2}),(\d{3})")
        .expect("valid regex");

    let result = re.replace_all(content, |caps: &regex::Captures| {
        let start = parse_ts(caps, 1) + offset_ms;
        let end = parse_ts(caps, 5) + offset_ms;
        format!("{} --> {}", format_ts(start.max(0)), format_ts(end.max(0)))
    });
    Ok(result.into_owned())
}

fn parse_ts(caps: &regex::Captures, offset: usize) -> i64 {
    let h: i64 = caps[offset].parse().unwrap_or(0);
    let m: i64 = caps[offset + 1].parse().unwrap_or(0);
    let s: i64 = caps[offset + 2].parse().unwrap_or(0);
    let ms: i64 = caps[offset + 3].parse().unwrap_or(0);
    h * 3_600_000 + m * 60_000 + s * 1_000 + ms
}

fn format_ts(total_ms: i64) -> String {
    let h = total_ms / 3_600_000;
    let m = (total_ms % 3_600_000) / 60_000;
    let s = (total_ms % 60_000) / 1_000;
    let ms = total_ms % 1_000;
    format!("{:02}:{:02}:{:02},{:03}", h, m, s, ms)
}

// ── extract / list subtitle streams (ffmpeg) ─────────────────────────────

pub async fn extract_sub_srt(
    file: impl AsRef<Path>,
    stream: Option<u32>,
) -> Result<(), FfmpegError> {
    let stream_idx = stream.unwrap_or(0);
    let map_spec = format!("0:s:{}", stream_idx);
    let file_str = file.as_ref().to_str().unwrap_or("");
    let out = file
        .as_ref()
        .with_extension("srt")
        .to_str()
        .unwrap_or("")
        .to_string();
    let options = vec!["-i", file_str, "-map", &map_spec, "-c", "copy", &out];
    FfmpegTool::Ffmpeg
        .exec_with_options(None::<&'static str>, Some(options))
        .await?;
    Ok(())
}

#[derive(Debug, Deserialize, Serialize)]
struct FfprobeOutput {
    streams: Vec<FfprobeStream>,
}

#[derive(Debug, Deserialize, Serialize)]
struct FfprobeStream {
    index: u32,
    codec_type: String,
    codec_name: String,
    start_time: String,
    duration_ts: u32,
    tags: Option<FfprobeTags>,
}

#[derive(Debug, Deserialize, Serialize)]
struct FfprobeTags {
    language: Option<String>,
    title: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct SubtitleStreamInfo {
    pub index: u32,
    pub codec_name: String,
    pub duration: u32,
    pub language: Option<String>,
    pub title: Option<String>,
}

pub async fn list_all_subtitle_stream(
    file: impl AsRef<Path>,
) -> anyhow::Result<Vec<SubtitleStreamInfo>> {
    let file_path = file.as_ref();
    if !file_path.exists() {
        anyhow::bail!("文件不存在: {}", file_path.display());
    }

    let mut args: Vec<String> = vec![
        "-v".to_string(),
        "quiet".to_string(),
        "-print_format".to_string(),
        "json".to_string(),
        "-show_streams".to_string(),
        "-select_streams".to_string(),
        "s".to_string(),
        "--".to_string(),
    ];
    args.push(file_path.to_string_lossy().to_string());

    let (stdout, _) = FfmpegTool::Ffprobe
        .exec_with_options(None::<&'static str>, Some(args))
        .await?;

    if stdout.is_empty() {
        return Ok(vec![]);
    }

    let output: FfprobeOutput = serde_json::from_str(&stdout)?;
    let streams = output
        .streams
        .into_iter()
        .map(|stream| SubtitleStreamInfo {
            index: stream.index,
            codec_name: stream.codec_name,
            duration: stream.duration_ts,
            language: stream.tags.as_ref().and_then(|t| t.language.clone()),
            title: stream.tags.as_ref().and_then(|t| t.title.clone()),
        })
        .collect();
    Ok(streams)
}

// ── Tauri commands ───────────────────────────────────────────────────────

#[tauri::command]
pub async fn fetch_subtitle_cmd(
    video: String,
    lang: String,
    _api_key: String,
) -> std::result::Result<(), String> {
    let cfg = crate::config::load_config();
    let video_path = std::path::Path::new(&video);
    fetch_subtitle(video_path, &lang, &cfg)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn align_subtitle_cmd(video: String, srt: String) -> std::result::Result<(), String> {
    let cfg = crate::config::load_config();
    align_subtitle(
        std::path::Path::new(&video),
        std::path::Path::new(&srt),
        Some(&cfg.tools.alass),
    )
    .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn extract_subtitle_cmd(
    video: String,
    stream: Option<u32>,
) -> std::result::Result<(), String> {
    extract_sub_srt(std::path::Path::new(&video), stream)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn list_subtitle_streams_cmd(
    video: String,
) -> std::result::Result<Vec<SubtitleStreamInfo>, String> {
    list_all_subtitle_stream(std::path::Path::new(&video))
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn shift_subtitle_cmd(srt: String, offset_ms: i64) -> std::result::Result<(), String> {
    shift_srt(std::path::Path::new(&srt), offset_ms).map_err(|e| e.to_string())
}

// ── Tests ────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clean_query_strips_quality_tags() {
        let q = clean_query("Blow-Up.1966.1080p.BluRay.x264.AAC-YTS.MX");
        assert_eq!(q, "Blow Up 1966");
    }

    #[test]
    fn clean_query_keeps_title_and_year() {
        let q = clean_query("The.Godfather.1972.1080p.BluRay");
        assert_eq!(q, "The Godfather 1972");
    }

    #[test]
    fn align_returns_error_when_alass_missing() {
        let result = align_with_binary(
            Path::new("nonexistent_alass_binary_xyz"),
            Path::new("video.mp4"),
            Path::new("sub.srt"),
        );
        assert!(matches!(result, Err(SubError::AlassFailed(_))));
    }

    #[test]
    fn offset_positive() {
        let srt = "1\n00:01:00,000 --> 00:01:05,000\nHello\n";
        let result = apply_offset(srt, 5000).unwrap();
        assert!(result.contains("00:01:05,000 --> 00:01:10,000"));
    }

    #[test]
    fn offset_negative() {
        let srt = "1\n00:01:00,000 --> 00:01:05,000\nHello\n";
        let result = apply_offset(srt, -10000).unwrap();
        assert!(result.contains("00:00:50,000 --> 00:00:55,000"));
    }

    #[test]
    fn clamp_at_zero() {
        let srt = "1\n00:00:01,000 --> 00:00:02,000\nHello\n";
        let result = apply_offset(srt, -5000).unwrap();
        assert!(result.contains("00:00:00,000 --> 00:00:00,000"));
    }
}
