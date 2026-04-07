// src-tauri/src/commands/subtitle.rs
//
// Merged from _sub_fetch.rs, _sub_align.rs, _sub_shift.rs, _sub_mod.rs

// ── Imports ────────────────────────────────────────────────────────────────
use crate::config::Config;
use crate::error::SubError;
use crate::ffmpeg::{FfmpegError, FfmpegTool};
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::fs;
use std::io::Read;
use std::path::Path;
use std::process::Command;
use which::which;

// ── SubSource (from _sub_fetch.rs) ─────────────────────────────────────────
pub enum SubSource {
    OpenSubtitles,
    All,
}

// ── fetch_subtitle and helpers (from _sub_fetch.rs) ────────────────────────
const XMLRPC_URL: &str = "https://api.opensubtitles.org/xml-rpc";
const USER_AGENT: &str = "blowup v0.1";

pub async fn fetch_subtitle(
    video: &Path,
    lang: &str,
    _source: SubSource,
    _cfg: &Config,
) -> Result<(), SubError> {
    let stem = video
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("")
        .to_string();

    let query = clean_query(&stem);
    let os_lang = to_opensubtitles_lang(lang);

    let client = reqwest::Client::new();

    let token = xmlrpc_login(&client).await?;
    let results = xmlrpc_search(&client, &token, &query, os_lang).await?;

    if results.is_empty() {
        return Err(SubError::NoSubtitleFound);
    }

    let best = &results[0];
    let out_path = video.with_extension(format!("{lang}.srt"));
    download_subtitle(&client, &best.download_url, &out_path).await?;
    println!("Saved subtitle: {}", out_path.display());
    println!("Source file:    {}", best.filename);
    Ok(())
}

struct SubtitleResult {
    filename: String,
    download_url: String,
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

fn to_opensubtitles_lang(lang: &str) -> &str {
    match lang {
        "zh" | "zh-CN" | "chs" => "chi",
        "en" => "eng",
        "ja" => "jpn",
        "ko" => "kor",
        "fr" => "fre",
        "de" => "ger",
        "es" => "spa",
        other => other,
    }
}

async fn xmlrpc_login(client: &reqwest::Client) -> Result<String, SubError> {
    let body = r#"<?xml version="1.0"?><methodCall><methodName>LogIn</methodName><params><param><value><string></string></value></param><param><value><string></string></value></param><param><value><string>en</string></value></param><param><value><string>blowup v0.1</string></value></param></params></methodCall>"#;

    let resp = client
        .post(XMLRPC_URL)
        .header("User-Agent", USER_AGENT)
        .header("Content-Type", "text/xml")
        .body(body)
        .send()
        .await?;

    let text = resp.text().await?;
    extract_xmlrpc_string(&text, "token")
        .ok_or_else(|| SubError::InvalidSrt("OpenSubtitles login: no token in response".into()))
}

async fn xmlrpc_search(
    client: &reqwest::Client,
    token: &str,
    query: &str,
    lang: &str,
) -> Result<Vec<SubtitleResult>, SubError> {
    let body = format!(
        r#"<?xml version="1.0"?><methodCall><methodName>SearchSubtitles</methodName><params><param><value><string>{token}</string></value></param><param><value><array><data><value><struct><member><name>sublanguageid</name><value><string>{lang}</string></value></member><member><name>query</name><value><string>{query}</string></value></member></struct></value></data></array></value></param></params></methodCall>"#
    );

    let resp = client
        .post(XMLRPC_URL)
        .header("User-Agent", USER_AGENT)
        .header("Content-Type", "text/xml")
        .body(body)
        .send()
        .await?;

    let text = resp.text().await?;
    parse_xmlrpc_search_results(&text)
}

fn extract_xmlrpc_string(xml: &str, member_name: &str) -> Option<String> {
    let pattern = format!(r"<name>{member_name}</name><value><string>([^<]+)</string>");
    let re = Regex::new(&pattern).ok()?;
    re.captures(xml)
        .and_then(|c| c.get(1))
        .map(|m| m.as_str().to_string())
}

fn parse_xmlrpc_search_results(xml: &str) -> Result<Vec<SubtitleResult>, SubError> {
    let name_re = Regex::new(r"<name>SubFileName</name><value><string>([^<]+)</string>")
        .expect("valid regex");
    let link_re = Regex::new(r"<name>SubDownloadLink</name><value><string>([^<]+)</string>")
        .expect("valid regex");

    let names: Vec<&str> = name_re
        .captures_iter(xml)
        .filter_map(|c| c.get(1).map(|m| m.as_str()))
        .collect();
    let links: Vec<&str> = link_re
        .captures_iter(xml)
        .filter_map(|c| c.get(1).map(|m| m.as_str()))
        .collect();

    let results = names
        .into_iter()
        .zip(links)
        .filter(|(name, _)| name.ends_with(".srt"))
        .map(|(name, link)| SubtitleResult {
            filename: name.to_string(),
            download_url: link.to_string(),
        })
        .collect();

    Ok(results)
}

fn strip_session_from_url(url: &str) -> String {
    // OpenSubtitles embeds the session token as a path segment like /sid-TOKEN/
    // which triggers a restricted "VIP only" download; remove it.
    let re = Regex::new(r"/sid-[^/]+").expect("valid regex");
    re.replace(url, "").to_string()
}

async fn download_subtitle(
    client: &reqwest::Client,
    url: &str,
    out_path: &Path,
) -> Result<(), SubError> {
    let clean_url = strip_session_from_url(url);
    let resp = client
        .get(&clean_url)
        .header("User-Agent", USER_AGENT)
        .send()
        .await?;

    if !resp.status().is_success() {
        return Err(SubError::NoSubtitleFound);
    }

    let bytes = resp.bytes().await?;

    let content = if bytes.starts_with(&[0x1f, 0x8b]) {
        let mut gz = flate2::read::GzDecoder::new(&bytes[..]);
        let mut out = Vec::new();
        gz.read_to_end(&mut out)
            .map_err(|e| SubError::InvalidSrt(e.to_string()))?;
        out
    } else {
        bytes.to_vec()
    };

    std::fs::write(out_path, &content).map_err(SubError::Io)?;
    Ok(())
}

// ── align_subtitle and helpers (from _sub_align.rs) ────────────────────────

/// 使用 alass 自动对齐字幕时间轴
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

    // 先尝试运行 alass，只有 binary 可执行时才做备份
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

    // alass 成功后将输出（backup）覆盖回原路径
    std::fs::copy(&backup, srt).map_err(SubError::Io)?;
    Ok(())
}

// ── shift_srt and helpers (from _sub_shift.rs) ─────────────────────────────

/// 将 SRT 文件中所有时间戳偏移 offset_ms 毫秒
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

// ── extract_sub_srt, list_all_subtitle_stream, structs (from _sub_mod.rs) ──

/// 将 file 视频容器中的指定字幕流以 srt 文件格式提取到 sub 路径中。
/// stream 为 None 时提取第一个字幕流（0:s:0）。
pub async fn extract_sub_srt(
    file: impl AsRef<Path>,
    stream: Option<u32>,
) -> Result<(), FfmpegError> {
    let stream_idx = stream.unwrap_or(0);
    let map_spec = format!("0:s:{}", stream_idx);
    let file_str = file.as_ref().to_str().unwrap_or("");
    // 输出到同目录下的 .srt 文件
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

/// 视频流的顶层结构体，用于解析 ffprobe 的 JSON 输出。
#[derive(Debug, Deserialize, Serialize)]
struct FfprobeOutput {
    streams: Vec<FfprobeStream>,
}

/// 单个流的详细信息
#[derive(Debug, Deserialize, Serialize)]
struct FfprobeStream {
    index: u32,
    codec_type: String,
    codec_name: String,
    start_time: String,
    duration_ts: u32,
    tags: Option<FfprobeTags>,
}

/// 流的标签信息
#[derive(Debug, Deserialize, Serialize)]
struct FfprobeTags {
    language: Option<String>,
    title: Option<String>,
}

/// 最终返回给调用者的字幕流信息结构体
#[derive(Debug, Clone, Serialize)]
pub struct SubtitleStreamInfo {
    pub index: u32,
    pub codec_name: String,
    pub duration: u32,
    pub language: Option<String>,
    pub title: Option<String>,
}

/// 列出视频文件中所有的字幕流信息并返回。
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

// ── Tauri commands ──────────────────────────────────────────────────────────

#[tauri::command]
pub async fn fetch_subtitle_cmd(
    video: String,
    lang: String,
    _api_key: String,
) -> std::result::Result<(), String> {
    let cfg = crate::config::load_config();
    let video_path = std::path::Path::new(&video);
    fetch_subtitle(video_path, &lang, SubSource::All, &cfg)
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

// ── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // Tests from _sub_fetch.rs
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
    fn lang_mapping_zh() {
        assert_eq!(to_opensubtitles_lang("zh"), "chi");
        assert_eq!(to_opensubtitles_lang("en"), "eng");
        assert_eq!(to_opensubtitles_lang("ja"), "jpn");
    }

    #[test]
    fn extract_xmlrpc_token() {
        let xml = r#"<member><name>token</name><value><string>abc123</string></value></member>"#;
        assert_eq!(extract_xmlrpc_string(xml, "token"), Some("abc123".into()));
    }

    #[test]
    fn strip_session_removes_sid_segment() {
        let url =
            "https://dl.opensubtitles.org/en/download/src-api/vrf-abc/sid-TOK,EN/filead/123.gz";
        let clean = strip_session_from_url(url);
        assert_eq!(
            clean,
            "https://dl.opensubtitles.org/en/download/src-api/vrf-abc/filead/123.gz"
        );
    }

    #[test]
    fn strip_session_noop_when_no_sid() {
        let url = "https://dl.opensubtitles.org/en/download/src-api/vrf-abc/filead/123.gz";
        assert_eq!(strip_session_from_url(url), url);
    }

    #[test]
    fn parse_xmlrpc_search_single_result() {
        let xml = r#"<member><name>SubFileName</name><value><string>Blow-Up.srt</string></value></member><member><name>SubDownloadLink</name><value><string>https://example.com/sub.gz</string></value></member>"#;
        let results = parse_xmlrpc_search_results(xml).unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].filename, "Blow-Up.srt");
    }

    // Tests from _sub_align.rs
    #[test]
    fn align_returns_error_when_alass_missing() {
        let result = align_with_binary(
            Path::new("nonexistent_alass_binary_xyz"),
            Path::new("video.mp4"),
            Path::new("sub.srt"),
        );
        assert!(matches!(result, Err(SubError::AlassFailed(_))));
    }

    // Tests from _sub_shift.rs
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
