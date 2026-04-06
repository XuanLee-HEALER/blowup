use crate::error::TmdbError;
use serde::Deserialize;

// Internal deserialization structs (private)

#[derive(Deserialize)]
struct SearchResponse {
    results: Vec<SearchResult>,
}

#[derive(Deserialize)]
struct SearchResult {
    id: u64,
}

#[derive(Deserialize)]
struct MovieDetails {
    title: String,
    release_date: String, // "1966-12-18"
    overview: String,
    vote_average: f64,
    genres: Vec<Genre>,
    credits: Credits,
}

#[derive(Deserialize)]
struct Genre {
    name: String,
}

#[derive(Deserialize)]
struct Credits {
    crew: Vec<CrewMember>,
    cast: Vec<CastMember>,
}

#[derive(Deserialize)]
struct CrewMember {
    job: String,
    name: String,
}

#[derive(Clone, Deserialize)]
struct CastMember {
    name: String,
    order: u32,
}

// Public output struct

pub struct TmdbMovie {
    pub title: String,
    pub year: String,    // 4-digit year extracted from release_date
    pub genres: String,  // comma-separated genre names
    pub director: String, // first crew member where job == "Director", or "N/A"
    pub actors: String,  // top 3 cast members by order, comma-separated
    pub rating: String,  // vote_average formatted to 1 decimal, e.g. "7.2"
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

fn extract_year(release_date: &str) -> String {
    // release_date is "YYYY-MM-DD", take first 4 chars
    release_date.chars().take(4).collect()
}

fn parse_movie_details(details: &MovieDetails) -> TmdbMovie {
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
        .get(format!(
            "https://api.themoviedb.org/3/movie/{}",
            movie_id
        ))
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

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_details() -> MovieDetails {
        MovieDetails {
            title: "Blow-Up".to_string(),
            release_date: "1966-12-18".to_string(),
            overview: "A mod London photographer discovers he may have inadvertently photographed a murder.".to_string(),
            vote_average: 7.2,
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
        assert_eq!(movie.actors, "David Hemmings, Vanessa Redgrave, Sarah Miles");
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
