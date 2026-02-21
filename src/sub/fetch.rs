use std::path::Path;
use crate::error::SubError;
use crate::config::Config;

pub enum SubSource {
    Assrt,
    OpenSubtitles,
    All,
}

pub struct SubtitleResult {
    pub filename: String,
    pub lang: String,
    pub source: String,
}

pub async fn fetch_subtitle(
    video: &Path,
    lang: &str,
    source: SubSource,
    cfg: &Config,
) -> Result<(), SubError> {
    let query = video
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("")
        .to_string();

    let client = reqwest::Client::new();
    let results = match source {
        SubSource::Assrt => search_assrt(&client, &query, lang).await?,
        SubSource::OpenSubtitles => {
            search_opensubtitles(&client, &query, lang, &cfg.opensubtitles.api_key).await?
        }
        SubSource::All => {
            let mut res = search_assrt(&client, &query, lang).await.unwrap_or_default();
            let os = search_opensubtitles(&client, &query, lang, &cfg.opensubtitles.api_key)
                .await
                .unwrap_or_default();
            res.extend(os);
            res
        }
    };

    if results.is_empty() {
        return Err(SubError::NoSubtitleFound);
    }

    for r in &results {
        println!("[{}] {} ({})", r.source, r.filename, r.lang);
    }
    Ok(())
}

async fn search_assrt(
    client: &reqwest::Client,
    query: &str,
    lang: &str,
) -> Result<Vec<SubtitleResult>, SubError> {
    let resp = client
        .get("https://api.assrt.net/v1/subtitle/search")
        .query(&[("q", query), ("lang", lang)])
        .header("User-Agent", "blowup/0.1 (personal movie tool)")
        .send()
        .await?;

    if !resp.status().is_success() {
        return Err(SubError::NoSubtitleFound);
    }

    let body: serde_json::Value = resp.json().await?;
    parse_assrt_response(&body)
}

fn parse_assrt_response(body: &serde_json::Value) -> Result<Vec<SubtitleResult>, SubError> {
    let subs = body["sub"]["subs"]
        .as_array()
        .ok_or(SubError::NoSubtitleFound)?;

    let results = subs
        .iter()
        .filter_map(|s| {
            Some(SubtitleResult {
                filename: s["filename"].as_str()?.to_string(),
                lang: s["lang"]["desc"].as_str().unwrap_or("zh").to_string(),
                source: "assrt".to_string(),
            })
        })
        .collect();
    Ok(results)
}

async fn search_opensubtitles(
    client: &reqwest::Client,
    query: &str,
    lang: &str,
    api_key: &str,
) -> Result<Vec<SubtitleResult>, SubError> {
    let mut req = client
        .get("https://api.opensubtitles.com/api/v1/subtitles")
        .query(&[("query", query), ("languages", lang)])
        .header("User-Agent", "blowup v0.1")
        .header("Content-Type", "application/json");

    if !api_key.is_empty() {
        req = req.header("Api-Key", api_key);
    }

    let resp = req.send().await?;
    if !resp.status().is_success() {
        return Err(SubError::NoSubtitleFound);
    }

    let body: serde_json::Value = resp.json().await?;
    parse_opensubtitles_response(&body)
}

fn parse_opensubtitles_response(body: &serde_json::Value) -> Result<Vec<SubtitleResult>, SubError> {
    let data = body["data"]
        .as_array()
        .ok_or(SubError::NoSubtitleFound)?;

    let results = data
        .iter()
        .filter_map(|item| {
            let attrs = &item["attributes"];
            Some(SubtitleResult {
                filename: attrs["files"][0]["file_name"].as_str()?.to_string(),
                lang: attrs["language"].as_str().unwrap_or("zh").to_string(),
                source: "opensubtitles".to_string(),
            })
        })
        .collect();
    Ok(results)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn parse_assrt_response_ok() {
        let body = json!({
            "sub": {
                "subs": [
                    {"filename": "Blow-Up.1966.zh.srt", "lang": {"desc": "zh"}}
                ]
            }
        });
        let results = parse_assrt_response(&body).unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].filename, "Blow-Up.1966.zh.srt");
    }

    #[test]
    fn parse_assrt_empty_returns_empty() {
        let body = json!({"sub": {"subs": []}});
        let results = parse_assrt_response(&body).unwrap();
        assert_eq!(results.len(), 0);
    }

    #[test]
    fn parse_opensubtitles_response_ok() {
        let body = json!({
            "data": [
                {
                    "attributes": {
                        "language": "zh",
                        "files": [{"file_name": "blow_up_1966_zh.srt"}]
                    }
                }
            ]
        });
        let results = parse_opensubtitles_response(&body).unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].source, "opensubtitles");
    }
}
