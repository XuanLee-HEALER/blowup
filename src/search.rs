use crate::error::SearchError;
use serde::Deserialize;

#[derive(Debug, Clone)]
pub struct MovieResult {
    pub title: String,
    pub year: u32,
    pub quality: String,
    pub magnet: Option<String>,
    pub torrent_url: Option<String>,
    pub seeds: u32,
}

pub async fn search_yify(query: &str, year: Option<u32>) -> Result<Vec<MovieResult>, SearchError> {
    let client = reqwest::Client::new();
    match search_via_api(&client, query, year).await {
        Ok(results) if !results.is_empty() => Ok(results),
        _ => Err(SearchError::NoResults(query.to_string())),
    }
}

async fn search_via_api(
    client: &reqwest::Client,
    query: &str,
    year: Option<u32>,
) -> Result<Vec<MovieResult>, SearchError> {
    let mut params = vec![
        ("query_term", query.to_string()),
        ("sort_by", "seeds".to_string()),
        ("order_by", "desc".to_string()),
    ];
    if let Some(y) = year {
        params.push(("year", y.to_string()));
    }

    let resp = client
        .get("https://yts.mx/api/v2/list_movies.json")
        .query(&params)
        .header("User-Agent", "blowup/0.1")
        .send()
        .await?;

    let body: YtsResponse = resp.json().await?;
    parse_yts_response(body)
}

#[derive(Deserialize)]
struct YtsResponse {
    data: YtsData,
}

#[derive(Deserialize)]
struct YtsData {
    #[serde(default)]
    movies: Vec<YtsMovie>,
}

#[derive(Deserialize)]
struct YtsMovie {
    title: String,
    year: u32,
    torrents: Vec<YtsTorrent>,
}

#[derive(Deserialize)]
struct YtsTorrent {
    quality: String,
    #[serde(rename = "url")]
    url: String,
    seeds: u32,
    #[serde(default)]
    magnet_url: Option<String>,
}

fn parse_yts_response(resp: YtsResponse) -> Result<Vec<MovieResult>, SearchError> {
    let mut results: Vec<MovieResult> = resp
        .data
        .movies
        .into_iter()
        .flat_map(|movie| {
            let title = movie.title.clone();
            let year = movie.year;
            movie.torrents.into_iter().map(move |t| MovieResult {
                title: title.clone(),
                year,
                quality: t.quality,
                magnet: t.magnet_url,
                torrent_url: Some(t.url),
                seeds: t.seeds,
            })
        })
        .collect();

    results.sort_by(|a, b| {
        quality_rank(&b.quality)
            .cmp(&quality_rank(&a.quality))
            .then(b.seeds.cmp(&a.seeds))
    });

    Ok(results)
}

fn quality_rank(q: &str) -> u8 {
    match q {
        "2160p" => 4,
        "1080p" => 3,
        "720p" => 2,
        _ => 1,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn make_yts_response(movies: serde_json::Value) -> YtsResponse {
        serde_json::from_value(json!({"data": {"movies": movies}})).unwrap()
    }

    #[test]
    fn parse_single_movie() {
        let resp = make_yts_response(json!([{
            "title": "Blow-Up",
            "year": 1966,
            "torrents": [
                {"quality": "1080p", "url": "http://x.com/a.torrent", "seeds": 100},
                {"quality": "720p",  "url": "http://x.com/b.torrent", "seeds": 200}
            ]
        }]));
        let results = parse_yts_response(resp).unwrap();
        assert_eq!(results.len(), 2);
        assert_eq!(results[0].quality, "1080p");
    }

    #[test]
    fn quality_rank_order() {
        assert!(quality_rank("1080p") > quality_rank("720p"));
        assert!(quality_rank("2160p") > quality_rank("1080p"));
    }

    #[test]
    fn empty_movies_returns_empty_vec() {
        let resp = make_yts_response(json!([]));
        let results = parse_yts_response(resp).unwrap();
        assert!(results.is_empty());
    }
}
