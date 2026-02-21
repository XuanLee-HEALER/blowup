use crate::error::OmdbError;
use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct OmdbMovie {
    #[serde(rename = "Title")]
    pub title: String,
    #[serde(rename = "Year")]
    pub year: String,
    #[serde(rename = "Rated")]
    pub rated: String,
    #[serde(rename = "imdbRating")]
    pub imdb_rating: String,
    #[serde(rename = "Genre")]
    pub genre: String,
    #[serde(rename = "Director")]
    pub director: String,
    #[serde(rename = "Actors")]
    pub actors: String,
    #[serde(rename = "Plot")]
    pub plot: String,
    #[serde(rename = "Poster")]
    pub poster: String,
    #[serde(rename = "Response")]
    pub response: String,
}

impl OmdbMovie {
    /// 打印格式化的电影信息，并在末尾显示 blowup search 提示
    pub fn print_info(&self) {
        // 从 year 中提取4位数字（兼容 "1966" 和 "2023–" 等格式）
        let year_num: String = self.year.chars().take(4).collect();
        println!("Title:    {} ({})", self.title, self.year);
        println!("Genre:    {}", self.genre);
        println!("Director: {}", self.director);
        println!("Actors:   {}", self.actors);
        println!("Rating:   {}/10 (IMDb)", self.imdb_rating);
        println!("Rated:    {}", self.rated);
        println!("Plot:     {}", self.plot);
        println!();
        println!(
            "💡 搜索种子: blowup search \"{}\" --year {}",
            self.title, year_num
        );
    }
}

fn parse_omdb_response(body: &serde_json::Value) -> Result<OmdbMovie, OmdbError> {
    if body["Response"].as_str() == Some("False") {
        let title = body["Error"].as_str().unwrap_or("unknown").to_string();
        return Err(OmdbError::NotFound(title));
    }
    serde_json::from_value(body.clone()).map_err(|e| OmdbError::NotFound(e.to_string()))
}

pub async fn query_omdb(
    api_key: &str,
    title: &str,
    year: Option<u32>,
) -> Result<OmdbMovie, OmdbError> {
    if api_key.is_empty() {
        return Err(OmdbError::ApiKeyMissing);
    }

    let client = reqwest::Client::new();
    let mut params = vec![
        ("apikey", api_key.to_string()),
        ("t", title.to_string()),
        ("plot", "short".to_string()),
    ];
    if let Some(y) = year {
        params.push(("y", y.to_string()));
    }

    let body: serde_json::Value = client
        .get("http://www.omdbapi.com/")
        .query(&params)
        .header("User-Agent", "blowup/0.1")
        .send()
        .await?
        .json()
        .await?;

    parse_omdb_response(&body)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn parse_valid_response() {
        let body = json!({
            "Title": "Blow-Up",
            "Year": "1966",
            "Rated": "NR",
            "imdbRating": "7.6",
            "Genre": "Drama, Mystery, Thriller",
            "Director": "Michelangelo Antonioni",
            "Actors": "David Hemmings, Vanessa Redgrave, Sarah Miles",
            "Plot": "A mod London photographer...",
            "Poster": "https://example.com/poster.jpg",
            "Response": "True"
        });
        let movie = parse_omdb_response(&body).unwrap();
        assert_eq!(movie.title, "Blow-Up");
        assert_eq!(movie.year, "1966");
        assert_eq!(movie.imdb_rating, "7.6");
    }

    #[test]
    fn parse_not_found_response() {
        let body = json!({"Response": "False", "Error": "Movie not found!"});
        let err = parse_omdb_response(&body).unwrap_err();
        assert!(matches!(err, OmdbError::NotFound(_)));
    }

    #[tokio::test]
    async fn api_key_missing_returns_error() {
        let result = query_omdb("", "Blow-Up", None).await;
        assert!(matches!(result, Err(OmdbError::ApiKeyMissing)));
    }
}
