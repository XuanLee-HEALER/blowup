use crate::config::Config;
use crate::error::SubError;
use regex::Regex;
use std::io::Read;
use std::path::Path;

pub enum SubSource {
    OpenSubtitles,
    All,
}

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
    let s = stem
        .replace('.', " ")
        .replace('-', " ")
        .replace('[', " ")
        .replace(']', " ");
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
    let pattern = format!(
        r"<name>{member_name}</name><value><string>([^<]+)</string>"
    );
    let re = Regex::new(&pattern).ok()?;
    re.captures(xml)
        .and_then(|c| c.get(1))
        .map(|m| m.as_str().to_string())
}

fn parse_xmlrpc_search_results(xml: &str) -> Result<Vec<SubtitleResult>, SubError> {
    let name_re =
        Regex::new(r"<name>SubFileName</name><value><string>([^<]+)</string>").unwrap();
    let link_re =
        Regex::new(r"<name>SubDownloadLink</name><value><string>([^<]+)</string>").unwrap();

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
    let re = Regex::new(r"/sid-[^/]+").unwrap();
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
        let url = "https://dl.opensubtitles.org/en/download/src-api/vrf-abc/sid-TOK,EN/filead/123.gz";
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
}
