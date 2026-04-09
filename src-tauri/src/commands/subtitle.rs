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
use std::sync::{LazyLock, Mutex};
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
    tmdb_id: Option<u64>,
) -> Result<Vec<OsSearchResult>, SubError> {
    let url = format!("{API_BASE}/subtitles");
    tracing::debug!(
        url = %url,
        query = %query,
        lang = %lang,
        tmdb_id = ?tmdb_id,
        "os_search: sending request"
    );
    let mut req = client.get(&url).query(&[("languages", lang)]);
    if let Some(id) = tmdb_id {
        req = req.query(&[("tmdb_id", &id.to_string())]);
    } else {
        req = req.query(&[("query", query)]);
    }
    let resp = req.send().await?;

    let status = resp.status();
    if !status.is_success() {
        let body = resp.text().await.unwrap_or_default();
        tracing::warn!(
            status = %status,
            body = %body,
            "os_search: request failed"
        );
        return Err(SubError::InvalidSrt(format!(
            "OpenSubtitles search failed ({status}): {body}"
        )));
    }

    let body_text = resp.text().await.unwrap_or_default();
    tracing::debug!(
        status = %status,
        body_len = body_text.len(),
        body_preview = %body_text.chars().take(500).collect::<String>(),
        "os_search: response received"
    );
    let search: OsSearchResponse = serde_json::from_str(&body_text)
        .map_err(|e| SubError::InvalidSrt(format!("parse search response: {e}")))?;
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
    tracing::info!(
        video = %video.display(),
        stem = %stem,
        query = %query,
        lang = %lang,
        "subtitle search: searching OpenSubtitles"
    );

    // Search (legacy path — no TMDB ID available)
    let results = os_search(&client, &query, lang, None).await?;
    tracing::info!(
        results = results.len(),
        query = %query,
        "subtitle search: got results"
    );
    if results.is_empty() {
        tracing::warn!(
            query = %query,
            lang = %lang,
            "subtitle search: no results found"
        );
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
            let guard = TOKEN_CACHE.lock().expect("TOKEN_CACHE mutex poisoned");
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
                    *TOKEN_CACHE.lock().expect("TOKEN_CACHE mutex poisoned") =
                        Some((t.clone(), Instant::now()));
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

/// Check if a token looks like a year (1900–2099).
fn is_year(s: &str) -> bool {
    s.len() == 4 && s.starts_with(['1', '2']) && s.chars().all(|c| c.is_ascii_digit())
}

fn clean_query(stem: &str) -> String {
    let s: String = stem
        .chars()
        .map(|c| {
            if matches!(c, '.' | '-' | '[' | ']' | '(' | ')') {
                ' '
            } else {
                c
            }
        })
        .collect();
    let mut out = Vec::new();
    for t in s.split_whitespace() {
        // Stop at resolution/codec/source tags
        if matches!(
            t,
            "1080p"
                | "720p"
                | "480p"
                | "2160p"
                | "BluRay"
                | "BRRip"
                | "WEB"
                | "WEBRip"
                | "WEB-DL"
                | "HDTV"
                | "DVDRip"
                | "x264"
                | "x265"
                | "H264"
                | "H265"
                | "HEVC"
                | "AAC"
                | "AC3"
                | "DTS"
                | "REMUX"
                | "YIFY"
                | "YTS"
        ) {
            break;
        }
        // Skip year tokens — they hurt search relevance
        if is_year(t) {
            continue;
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

static SRT_TIMESTAMP_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(\d{2}):(\d{2}):(\d{2}),(\d{3}) --> (\d{2}):(\d{2}):(\d{2}),(\d{3})")
        .expect("valid SRT timestamp regex")
});

fn apply_offset(content: &str, offset_ms: i64) -> Result<String, SubError> {
    let re = &*SRT_TIMESTAMP_RE;

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

// ── ASSRT (射手网) REST API ──────────────────────────────────────────────

const ASSRT_API_BASE: &str = "https://api.assrt.net/v1";

#[derive(Debug, Deserialize)]
struct AssrtSearchResponse {
    status: i32,
    sub: Option<AssrtSubWrapper>,
}

#[derive(Debug, Deserialize)]
struct AssrtSubWrapper {
    subs: Option<Vec<AssrtSubEntry>>,
}

#[derive(Debug, Deserialize)]
struct AssrtSubEntry {
    id: i64,
    #[serde(default)]
    native_name: Option<String>,
    #[serde(default)]
    videoname: Option<String>,
    #[serde(default)]
    lang: Option<AssrtLang>,
}

#[derive(Debug, Deserialize)]
struct AssrtLang {
    desc: Option<String>,
}

#[derive(Debug, Deserialize)]
struct AssrtDetailResponse {
    sub: Option<AssrtDetailWrapper>,
}

#[derive(Debug, Deserialize)]
struct AssrtDetailWrapper {
    subs: Option<Vec<AssrtDetailEntry>>,
}

#[derive(Debug, Deserialize)]
struct AssrtDetailEntry {
    /// filelist can be a JSON array OR an object (map of index→file).
    /// We accept both via a custom deserializer.
    #[serde(default, deserialize_with = "deserialize_filelist")]
    filelist: Vec<AssrtFile>,
    /// When there's only one file, ASSRT puts url/filename directly on the entry.
    #[serde(default)]
    url: Option<String>,
    #[serde(default)]
    filename: Option<String>,
}

fn deserialize_filelist<'de, D>(deserializer: D) -> Result<Vec<AssrtFile>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    use serde::de;
    use serde_json::Value;

    let val = Value::deserialize(deserializer)?;
    match val {
        Value::Array(arr) => {
            let mut out = Vec::new();
            for v in arr {
                if let Ok(f) = serde_json::from_value::<AssrtFile>(v) {
                    out.push(f);
                }
            }
            Ok(out)
        }
        Value::Object(map) => {
            let mut out = Vec::new();
            for (_k, v) in map {
                if let Ok(f) = serde_json::from_value::<AssrtFile>(v) {
                    out.push(f);
                }
            }
            Ok(out)
        }
        Value::Null => Ok(Vec::new()),
        _ => Err(de::Error::custom(
            "filelist: expected array, object or null",
        )),
    }
}

#[derive(Debug, Deserialize)]
struct AssrtFile {
    url: String,
    f: String,
}

async fn assrt_search(
    token: &str,
    query: &str,
    title: Option<&str>,
    year: Option<u32>,
) -> Result<Vec<SubtitleSearchResult>, String> {
    let url = format!("{ASSRT_API_BASE}/sub/search");
    // Prefer structured title+year over raw filename query
    let search_query = match (title, year) {
        (Some(t), Some(y)) => format!("{t} {y}"),
        (Some(t), None) => t.to_string(),
        _ => query.to_string(),
    };
    tracing::info!(query = %search_query, "assrt_search: searching");

    let resp = reqwest::Client::new()
        .get(&url)
        .query(&[
            ("q", search_query.as_str()),
            ("token", token),
            ("cnt", "5"),
            ("no_muxer", "1"), // Ignore release groups/video specs for better matching
        ])
        .send()
        .await
        .map_err(|e| format!("ASSRT 请求失败: {e}"))?;

    if !resp.status().is_success() {
        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        tracing::warn!(status = %status, body = %body, "assrt_search: request failed");
        return Err(format!("ASSRT 搜索失败 ({status})"));
    }

    let body = resp.text().await.unwrap_or_default();
    tracing::debug!(body_len = body.len(), "assrt_search: response received");

    let parsed: AssrtSearchResponse =
        serde_json::from_str(&body).map_err(|e| format!("ASSRT 响应解析失败: {e}"))?;

    if parsed.status != 0 {
        return Err(format!("ASSRT 返回错误状态: {}", parsed.status));
    }

    let subs = parsed.sub.and_then(|s| s.subs).unwrap_or_default();

    tracing::info!(count = subs.len(), "assrt_search: results");

    Ok(subs
        .into_iter()
        .map(|s| {
            let title = s
                .native_name
                .or(s.videoname)
                .unwrap_or_else(|| format!("#{}", s.id));
            let language = s.lang.and_then(|l| l.desc);
            SubtitleSearchResult {
                source: "assrt".to_string(),
                title,
                language,
                download_count: None,
                download_id: format!("assrt:{}", s.id),
            }
        })
        .collect())
}

async fn assrt_download(token: &str, sub_id: &str, out_path: &Path) -> Result<(), String> {
    let url = format!("{ASSRT_API_BASE}/sub/detail");
    tracing::info!(sub_id = %sub_id, "assrt_download: fetching detail");

    let resp = reqwest::Client::new()
        .get(&url)
        .query(&[("id", sub_id), ("token", token)])
        .send()
        .await
        .map_err(|e| format!("ASSRT 详情请求失败: {e}"))?;

    let body = resp.text().await.unwrap_or_default();
    tracing::debug!(
        body_len = body.len(),
        body_preview = %body.chars().take(800).collect::<String>(),
        "assrt_download: detail response"
    );

    let parsed: AssrtDetailResponse =
        serde_json::from_str(&body).map_err(|e| format!("ASSRT 详情解析失败: {e}"))?;

    let entry = parsed.sub.and_then(|s| s.subs).and_then(|mut v| v.pop());

    let (dl_url, dl_filename) = if let Some(e) = entry {
        tracing::debug!(
            filelist_count = e.filelist.len(),
            has_direct_url = e.url.is_some(),
            "assrt_download: parsed detail"
        );

        if !e.filelist.is_empty() {
            // Pick first SRT/ASS file from filelist, or just the first
            let file = e
                .filelist
                .iter()
                .find(|f| {
                    let lower = f.f.to_lowercase();
                    lower.ends_with(".srt") || lower.ends_with(".ass")
                })
                .unwrap_or(&e.filelist[0]);
            (file.url.clone(), file.f.clone())
        } else if let Some(url) = e.url {
            // Fallback: single-file entry has url/filename directly on the sub
            let fname = e.filename.unwrap_or_else(|| "subtitle.srt".to_string());
            (url, fname)
        } else {
            return Err("ASSRT 未找到可下载的字幕文件".to_string());
        }
    } else {
        return Err("ASSRT 详情中无字幕条目".to_string());
    };

    // ASSRT returns HTML-escaped URLs: &amp; in both raw and percent-encoded forms.
    // Fix percent-encoded HTML entities in the path portion, keeping & in query string intact.
    // %26amp%3B → %26 (literal & in path stays encoded to avoid being parsed as query separator)
    let dl_url = dl_url
        .replace("%26amp%3B", "%26")
        .replace("%26amp;", "%26")
        .replace("&amp;", "&");
    tracing::info!(url = %dl_url, file_name = %dl_filename, "assrt_download: downloading");

    let dl_resp = reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::limited(10))
        .build()
        .unwrap_or_default()
        .get(&dl_url)
        .header("Referer", "https://assrt.net/")
        .header("User-Agent", USER_AGENT)
        .send()
        .await
        .map_err(|e| format!("字幕下载请求失败: {e}"))?;

    tracing::debug!(
        status = %dl_resp.status(),
        "assrt_download: download response"
    );

    if !dl_resp.status().is_success() {
        return Err(format!("字幕下载失败: HTTP {}", dl_resp.status()));
    }

    let bytes = dl_resp
        .bytes()
        .await
        .map_err(|e| format!("字幕下载读取失败: {e}"))?;

    let ext = Path::new(&dl_filename)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("srt");
    let final_path = out_path.with_extension(ext);
    std::fs::write(&final_path, &bytes).map_err(|e| format!("保存字幕失败: {e}"))?;

    tracing::info!(path = %final_path.display(), "assrt_download: saved");
    Ok(())
}

// ── Unified subtitle search ─────────────────────────────────────────────

#[derive(Debug, Clone, Serialize)]
pub struct SubtitleSearchResult {
    pub source: String,
    pub title: String,
    pub language: Option<String>,
    pub download_count: Option<i64>,
    pub download_id: String,
}

fn os_results_to_unified(results: &[OsSearchResult]) -> Vec<SubtitleSearchResult> {
    results
        .iter()
        .flat_map(|r| {
            r.attributes.files.iter().map(|f| SubtitleSearchResult {
                source: "opensubtitles".to_string(),
                title: r
                    .attributes
                    .release
                    .clone()
                    .unwrap_or_else(|| f.file_name.clone()),
                language: None,
                download_count: Some(r.attributes.download_count),
                download_id: format!("os:{}", f.file_id),
            })
        })
        .collect()
}

async fn search_with_priority(
    video: &Path,
    lang: &str,
    title: Option<&str>,
    year: Option<u32>,
    tmdb_id: Option<u64>,
    cfg: &Config,
) -> Result<Vec<SubtitleSearchResult>, String> {
    let stem = video
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("")
        .to_string();
    let query = clean_query(&stem);
    tracing::info!(
        query = %query, lang = %lang,
        title = ?title, year = ?year, tmdb_id = ?tmdb_id,
        "subtitle unified search"
    );

    let is_chinese = matches!(lang, "zh" | "zh-cn" | "zh-tw" | "chi" | "zho");
    let assrt_token = &cfg.assrt.token;
    let os_api_key = &cfg.opensubtitles.api_key;

    let mut results = Vec::new();

    if is_chinese {
        // Chinese: ASSRT first
        if !assrt_token.is_empty() {
            match assrt_search(assrt_token, &query, title, year).await {
                Ok(r) => results.extend(r),
                Err(e) => tracing::warn!(error = %e, "ASSRT search failed, trying OpenSubtitles"),
            }
        }
        if results.len() < 3
            && !os_api_key.is_empty()
            && let Ok(client) = os_client(os_api_key)
        {
            match os_search(&client, &query, lang, tmdb_id).await {
                Ok(r) => results.extend(os_results_to_unified(&r)),
                Err(e) => tracing::warn!(error = %e, "OpenSubtitles fallback failed"),
            }
        }
    } else {
        // Non-Chinese: OpenSubtitles first
        if !os_api_key.is_empty()
            && let Ok(client) = os_client(os_api_key)
        {
            match os_search(&client, &query, lang, tmdb_id).await {
                Ok(r) => results.extend(os_results_to_unified(&r)),
                Err(e) => {
                    tracing::warn!(error = %e, "OpenSubtitles search failed, trying ASSRT")
                }
            }
        }
        if results.len() < 3 && !assrt_token.is_empty() {
            match assrt_search(assrt_token, &query, title, year).await {
                Ok(r) => results.extend(r),
                Err(e) => tracing::warn!(error = %e, "ASSRT fallback failed"),
            }
        }
    }

    results.truncate(3);
    Ok(results)
}

async fn download_by_id(
    video: &Path,
    lang: &str,
    download_id: &str,
    cfg: &Config,
) -> Result<(), String> {
    let out_path = video.with_extension(format!("{lang}.srt"));

    if let Some(file_id_str) = download_id.strip_prefix("os:") {
        // OpenSubtitles download
        let file_id: i64 = file_id_str
            .parse()
            .map_err(|_| "无效的 OpenSubtitles 文件 ID".to_string())?;

        let api_key = &cfg.opensubtitles.api_key;
        if api_key.is_empty() {
            return Err("OpenSubtitles API key 未配置".to_string());
        }
        let client = os_client(api_key).map_err(|e| e.to_string())?;

        // Login if configured
        let username = &cfg.opensubtitles.username;
        let password = &cfg.opensubtitles.password;
        let token = if !username.is_empty() && !password.is_empty() {
            let cached = {
                let guard = TOKEN_CACHE.lock().expect("TOKEN_CACHE mutex poisoned");
                guard
                    .as_ref()
                    .filter(|(_, created)| created.elapsed().as_secs() < TOKEN_TTL_SECS)
                    .map(|(t, _)| t.clone())
            };
            if let Some(t) = cached {
                Some(t)
            } else {
                match os_login(&client, username, password).await {
                    Ok(t) => {
                        *TOKEN_CACHE.lock().expect("TOKEN_CACHE mutex poisoned") =
                            Some((t.clone(), Instant::now()));
                        Some(t)
                    }
                    Err(e) => {
                        tracing::warn!("OS login failed: {e}");
                        None
                    }
                }
            }
        } else {
            None
        };

        let dl = os_download(&client, token.as_deref(), file_id)
            .await
            .map_err(|e| e.to_string())?;
        let resp = reqwest::get(&dl.link)
            .await
            .map_err(|e| format!("下载失败: {e}"))?;
        let bytes = resp.bytes().await.map_err(|e| format!("下载失败: {e}"))?;
        std::fs::write(&out_path, &bytes).map_err(|e| format!("保存失败: {e}"))?;
        tracing::info!(path = %out_path.display(), "OS subtitle downloaded");
        Ok(())
    } else if let Some(sub_id) = download_id.strip_prefix("assrt:") {
        // ASSRT download
        let token = &cfg.assrt.token;
        if token.is_empty() {
            return Err("ASSRT token 未配置".to_string());
        }
        assrt_download(token, sub_id, &out_path).await
    } else {
        Err(format!("未知的下载来源: {download_id}"))
    }
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

#[tauri::command]
pub async fn search_subtitles_cmd(
    video: String,
    lang: String,
    title: Option<String>,
    year: Option<u32>,
    tmdb_id: Option<u64>,
) -> Result<Vec<SubtitleSearchResult>, String> {
    let cfg = crate::config::load_config();
    search_with_priority(
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
    let cfg = crate::config::load_config();
    download_by_id(Path::new(&video), &lang, &download_id, &cfg).await
}

// ── Tests ────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clean_query_strips_quality_and_year() {
        let q = clean_query("Blow-Up.1966.1080p.BluRay.x264.AAC-YTS.MX");
        assert_eq!(q, "Blow Up");
    }

    #[test]
    fn clean_query_keeps_title_only() {
        let q = clean_query("The.Godfather.1972.1080p.BluRay");
        assert_eq!(q, "The Godfather");
    }

    #[test]
    fn clean_query_persona() {
        let q = clean_query("Persona.1966.720p.BluRay.x264-[YTS.AM]");
        assert_eq!(q, "Persona");
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
