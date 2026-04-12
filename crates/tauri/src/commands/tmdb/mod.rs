mod credits;
mod enrichment;
mod search;

// Re-export all public items (including Tauri-generated __cmd__* items) so
// `commands::tmdb::search_movies` etc. continue to resolve in generate_handler!.
pub use credits::*;
pub use enrichment::*;
pub use search::*;

use crate::error::TmdbError;
use serde::Deserialize;
use serde::Serialize;

// ── Public types shared across submodules ────────────────────────

/// Lightweight result row shown in the search list.
#[derive(Debug, Clone, Serialize)]
pub struct MovieListItem {
    pub id: u64,
    pub title: String,
    pub original_title: String,
    pub year: String, // empty string if unknown
    pub overview: String,
    pub vote_average: f64,
    pub poster_path: Option<String>,
    pub genre_ids: Vec<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub director: Option<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub cast: Vec<String>,
}

#[derive(Debug, serde::Deserialize, Serialize)]
pub struct SearchFilters {
    pub year_from: Option<u32>,
    pub year_to: Option<u32>,
    pub genre_ids: Vec<u64>,
    pub min_rating: Option<f32>,
    pub sort_by: Option<String>, // "popularity.desc" | "vote_average.desc" | "release_date.desc"
    pub page: Option<u32>,
}

#[derive(Debug, Serialize)]
pub struct TmdbGenre {
    pub id: u64,
    pub name: String,
}

// Public output struct

pub struct TmdbMovie {
    pub title: String,
    pub year: String,     // 4-digit year extracted from release_date
    pub genres: String,   // comma-separated genre names
    pub director: String, // first crew member where job == "Director", or "N/A"
    pub actors: String,   // top 3 cast members by order, comma-separated
    pub rating: String,   // vote_average formatted to 1 decimal, e.g. "7.2"
    pub overview: String,
}

impl TmdbMovie {
    pub fn print_info(&self) {
        println!("Title:    {} ({})", self.title, self.year);
        println!("Genre:    {}", self.genres);
        println!("Director: {}", self.director);
        println!("Actors:   {}", self.actors);
        println!("Rating:   {}/10 (TMDB)", self.rating);
        println!("Plot:     {}", self.overview);
        println!();
        println!(
            "💡 搜索种子: blowup search \"{}\" --year {}",
            self.title, self.year
        );
    }
}

// ── Internal deserialization structs (shared across submodules) ──

#[derive(Deserialize)]
pub(crate) struct SearchResponse {
    pub(crate) results: Vec<SearchResult>,
}

#[derive(Deserialize)]
pub(crate) struct SearchResult {
    pub(crate) id: u64,
}

#[derive(Deserialize)]
pub(crate) struct MovieDetails {
    pub(crate) title: String,
    #[serde(default)]
    pub(crate) original_title: Option<String>,
    pub(crate) release_date: String, // "1966-12-18"
    pub(crate) overview: String,
    pub(crate) vote_average: f64,
    #[serde(default)]
    pub(crate) poster_path: Option<String>,
    pub(crate) genres: Vec<Genre>,
    pub(crate) credits: Credits,
}

#[derive(Deserialize)]
pub(crate) struct Genre {
    pub(crate) name: String,
}

#[derive(Deserialize)]
pub(crate) struct Credits {
    pub(crate) crew: Vec<CrewMember>,
    pub(crate) cast: Vec<CastMember>,
}

#[derive(Deserialize)]
pub(crate) struct CrewMember {
    pub(crate) job: String,
    pub(crate) name: String,
}

#[derive(Clone, Deserialize)]
pub(crate) struct CastMember {
    pub(crate) name: String,
    pub(crate) order: u32,
}

#[derive(serde::Deserialize)]
pub(crate) struct ListResponse {
    pub(crate) results: Vec<ListItem>,
}

#[derive(serde::Deserialize)]
pub(crate) struct ListItem {
    pub(crate) id: u64,
    pub(crate) title: String,
    pub(crate) original_title: String,
    pub(crate) release_date: Option<String>,
    pub(crate) overview: String,
    pub(crate) vote_average: f64,
    pub(crate) poster_path: Option<String>,
    pub(crate) genre_ids: Vec<u64>,
}

#[derive(serde::Deserialize)]
pub(crate) struct GenreListResponse {
    pub(crate) genres: Vec<GenreItem>,
}

#[derive(serde::Deserialize)]
pub(crate) struct GenreItem {
    pub(crate) id: u64,
    pub(crate) name: String,
}

#[derive(serde::Deserialize)]
pub(crate) struct PersonSearchResponse {
    pub(crate) results: Vec<PersonItem>,
}

#[derive(serde::Deserialize)]
pub(crate) struct PersonItem {
    pub(crate) id: u64,
}

// ── Shared helpers ──────────────────────────────────────────────

pub(crate) fn extract_year(release_date: &str) -> String {
    // release_date is "YYYY-MM-DD", take first 4 chars
    release_date.chars().take(4).collect()
}

pub(crate) fn parse_movie_details(details: &MovieDetails) -> TmdbMovie {
    let year = extract_year(&details.release_date);
    let genres = details
        .genres
        .iter()
        .map(|g| g.name.as_str())
        .collect::<Vec<_>>()
        .join(", ");
    let director = details
        .credits
        .crew
        .iter()
        .find(|c| c.job == "Director")
        .map(|c| c.name.clone())
        .unwrap_or_else(|| "N/A".to_string());
    let mut cast = details.credits.cast.clone();
    cast.sort_by_key(|c| c.order);
    let actors = cast
        .iter()
        .take(3)
        .map(|c| c.name.as_str())
        .collect::<Vec<_>>()
        .join(", ");
    let rating = format!("{:.1}", details.vote_average);

    TmdbMovie {
        title: details.title.clone(),
        year,
        genres,
        director,
        actors,
        rating,
        overview: details.overview.clone(),
    }
}

pub(crate) fn to_list_item(item: ListItem) -> MovieListItem {
    let year = item
        .release_date
        .as_deref()
        .and_then(|d| d.get(..4))
        .unwrap_or("")
        .to_string();
    MovieListItem {
        id: item.id,
        title: item.title,
        original_title: item.original_title,
        year,
        overview: item.overview,
        vote_average: item.vote_average,
        poster_path: item.poster_path,
        genre_ids: item.genre_ids,
        director: None,
        cast: Vec::new(),
    }
}

pub async fn query_tmdb(
    api_key: &str,
    title: &str,
    year: Option<u32>,
) -> Result<TmdbMovie, TmdbError> {
    if api_key.is_empty() {
        return Err(TmdbError::ApiKeyMissing);
    }

    let client = reqwest::Client::new();

    // Step 1: search
    let mut search_params = vec![
        ("api_key", api_key.to_string()),
        ("query", title.to_string()),
        ("language", "en-US".to_string()),
    ];
    if let Some(y) = year {
        search_params.push(("year", y.to_string()));
    }

    let search_resp: SearchResponse = client
        .get("https://api.themoviedb.org/3/search/movie")
        .query(&search_params)
        .header("User-Agent", "blowup/0.1")
        .send()
        .await?
        .json()
        .await?;

    let movie_id = search_resp
        .results
        .first()
        .ok_or_else(|| TmdbError::NotFound(title.to_string()))?
        .id;

    // Step 2: movie details with credits
    let details: MovieDetails = client
        .get(format!("https://api.themoviedb.org/3/movie/{}", movie_id))
        .query(&[
            ("api_key", api_key),
            ("append_to_response", "credits"),
            ("language", "en-US"),
        ])
        .header("User-Agent", "blowup/0.1")
        .send()
        .await?
        .json()
        .await?;

    Ok(parse_movie_details(&details))
}

pub(crate) fn build_discover_params(
    api_key: &str,
    f: &SearchFilters,
) -> Vec<(&'static str, String)> {
    let mut p: Vec<(&'static str, String)> = vec![
        ("api_key", api_key.to_string()),
        ("language", "en-US".to_string()),
        ("page", f.page.unwrap_or(1).to_string()),
        (
            "sort_by",
            f.sort_by
                .clone()
                .unwrap_or_else(|| "vote_average.desc".to_string()),
        ),
        ("vote_count.gte", "50".to_string()), // avoid films with 1 vote
    ];
    if let Some(y) = f.year_from {
        p.push(("primary_release_date.gte", format!("{y}-01-01")));
    }
    if let Some(y) = f.year_to {
        p.push(("primary_release_date.lte", format!("{y}-12-31")));
    }
    if !f.genre_ids.is_empty() {
        let ids: Vec<String> = f.genre_ids.iter().map(|id| id.to_string()).collect();
        p.push(("with_genres", ids.join(",")));
    }
    if let Some(r) = f.min_rating {
        p.push(("vote_average.gte", r.to_string()));
    }
    p
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_details() -> MovieDetails {
        MovieDetails {
            title: "Blow-Up".to_string(),
            original_title: Some("Blowup".to_string()),
            release_date: "1966-12-18".to_string(),
            overview: "A mod London photographer discovers he may have inadvertently photographed a murder.".to_string(),
            vote_average: 7.2,
            poster_path: Some("/abc123.jpg".to_string()),
            genres: vec![
                Genre { name: "Drama".to_string() },
                Genre { name: "Mystery".to_string() },
                Genre { name: "Thriller".to_string() },
            ],
            credits: Credits {
                crew: vec![
                    CrewMember { job: "Director".to_string(), name: "Michelangelo Antonioni".to_string() },
                    CrewMember { job: "Producer".to_string(), name: "Carlo Ponti".to_string() },
                ],
                cast: vec![
                    CastMember { name: "David Hemmings".to_string(), order: 0 },
                    CastMember { name: "Vanessa Redgrave".to_string(), order: 1 },
                    CastMember { name: "Sarah Miles".to_string(), order: 2 },
                    CastMember { name: "Extra Actor".to_string(), order: 3 },
                ],
            },
        }
    }

    #[test]
    fn parse_movie_details_extracts_year() {
        let details = sample_details();
        let movie = parse_movie_details(&details);
        assert_eq!(movie.year, "1966");
    }

    #[test]
    fn parse_movie_details_finds_director() {
        let details = sample_details();
        let movie = parse_movie_details(&details);
        assert_eq!(movie.director, "Michelangelo Antonioni");
    }

    #[test]
    fn parse_movie_details_takes_top_3_actors() {
        let details = sample_details();
        let movie = parse_movie_details(&details);
        assert_eq!(
            movie.actors,
            "David Hemmings, Vanessa Redgrave, Sarah Miles"
        );
    }

    #[test]
    fn parse_movie_details_formats_genres() {
        let details = sample_details();
        let movie = parse_movie_details(&details);
        assert_eq!(movie.genres, "Drama, Mystery, Thriller");
    }

    #[test]
    fn parse_movie_details_formats_rating() {
        let details = sample_details();
        let movie = parse_movie_details(&details);
        assert_eq!(movie.rating, "7.2");
    }

    #[tokio::test]
    async fn api_key_missing_returns_error() {
        let result = query_tmdb("", "Blow-Up", None).await;
        assert!(matches!(result, Err(TmdbError::ApiKeyMissing)));
    }
}
