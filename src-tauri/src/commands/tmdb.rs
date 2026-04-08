use crate::error::TmdbError;
use serde::Deserialize;
use serde::Serialize;

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
    #[serde(default)]
    original_title: Option<String>,
    release_date: String, // "1966-12-18"
    overview: String,
    vote_average: f64,
    #[serde(default)]
    poster_path: Option<String>,
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

#[derive(serde::Deserialize)]
struct ListResponse {
    results: Vec<ListItem>,
}

#[derive(serde::Deserialize)]
struct ListItem {
    id: u64,
    title: String,
    original_title: String,
    release_date: Option<String>,
    overview: String,
    vote_average: f64,
    poster_path: Option<String>,
    genre_ids: Vec<u64>,
}

#[derive(serde::Deserialize)]
struct GenreListResponse {
    genres: Vec<GenreItem>,
}

#[derive(serde::Deserialize)]
struct GenreItem {
    id: u64,
    name: String,
}

#[derive(serde::Deserialize)]
struct PersonSearchResponse {
    results: Vec<PersonItem>,
}

#[derive(serde::Deserialize)]
struct PersonItem {
    id: u64,
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

fn to_list_item(item: ListItem) -> MovieListItem {
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

fn build_discover_params(api_key: &str, f: &SearchFilters) -> Vec<(&'static str, String)> {
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

/// Search by title, optionally merge with director results.
#[tauri::command]
pub async fn search_movies(
    api_key: String,
    query: String,
    filters: SearchFilters,
) -> std::result::Result<Vec<MovieListItem>, String> {
    if api_key.is_empty() {
        return Err("TMDB API key not configured".into());
    }
    let client = reqwest::Client::new();
    let page = filters.page.unwrap_or(1);

    // ① Title search
    let params: Vec<(&str, String)> = vec![
        ("api_key", api_key.clone()),
        ("query", query.clone()),
        ("page", page.to_string()),
    ];
    let title_resp: ListResponse = client
        .get("https://api.themoviedb.org/3/search/movie")
        .query(&params)
        .send()
        .await
        .map_err(|e| e.to_string())?
        .json()
        .await
        .map_err(|e| e.to_string())?;

    let mut seen: std::collections::HashSet<u64> = std::collections::HashSet::new();
    let mut results: Vec<MovieListItem> = title_resp
        .results
        .into_iter()
        .map(|i| {
            seen.insert(i.id);
            to_list_item(i)
        })
        .collect();

    // ② Person search → discover
    let person_resp: Result<PersonSearchResponse, _> = client
        .get("https://api.themoviedb.org/3/search/person")
        .query(&[("api_key", &api_key), ("query", &query)])
        .send()
        .await
        .map_err(|e| e.to_string())?
        .json()
        .await;

    if let Ok(pr) = person_resp
        && let Some(person) = pr.results.first()
    {
        let mut disc_params = build_discover_params(&api_key, &filters);
        disc_params.push(("with_people", person.id.to_string()));
        let disc_resp: Result<ListResponse, _> = client
            .get("https://api.themoviedb.org/3/discover/movie")
            .query(&disc_params)
            .send()
            .await
            .map_err(|e| e.to_string())?
            .json()
            .await;
        if let Ok(dr) = disc_resp {
            for item in dr.results {
                if seen.insert(item.id) {
                    results.push(to_list_item(item));
                }
            }
        }
    }

    Ok(results)
}

/// Pure filter-based discovery (no text query).
#[tauri::command]
pub async fn discover_movies(
    api_key: String,
    filters: SearchFilters,
) -> std::result::Result<Vec<MovieListItem>, String> {
    if api_key.is_empty() {
        return Err("TMDB API key not configured".into());
    }
    let client = reqwest::Client::new();
    let params = build_discover_params(&api_key, &filters);
    let resp: ListResponse = client
        .get("https://api.themoviedb.org/3/discover/movie")
        .query(&params)
        .send()
        .await
        .map_err(|e| e.to_string())?
        .json()
        .await
        .map_err(|e| e.to_string())?;
    Ok(resp.results.into_iter().map(to_list_item).collect())
}

/// Fetch TMDB genre list (call once and cache in frontend).
#[tauri::command]
pub async fn list_genres(api_key: String) -> std::result::Result<Vec<TmdbGenre>, String> {
    if api_key.is_empty() {
        return Err("TMDB API key not configured".into());
    }
    let client = reqwest::Client::new();
    let resp: GenreListResponse = client
        .get("https://api.themoviedb.org/3/genre/movie/list")
        .query(&[("api_key", api_key.as_str()), ("language", "en-US")])
        .send()
        .await
        .map_err(|e| e.to_string())?
        .json()
        .await
        .map_err(|e| e.to_string())?;
    Ok(resp
        .genres
        .into_iter()
        .map(|g| TmdbGenre {
            id: g.id,
            name: g.name,
        })
        .collect())
}

#[derive(Serialize)]
pub struct TmdbCrewMember {
    pub id: i64,
    pub name: String,
    pub job: String,
    pub department: String,
}

#[derive(Serialize)]
pub struct TmdbCastMember {
    pub id: i64,
    pub name: String,
    pub character: String,
}

#[derive(Serialize)]
pub struct TmdbMovieCredits {
    pub tmdb_id: i64,
    pub title: String,
    pub original_title: Option<String>,
    pub year: Option<i64>,
    pub overview: Option<String>,
    pub vote_average: Option<f64>,
    pub poster_path: Option<String>,
    pub crew: Vec<TmdbCrewMember>,
    pub cast: Vec<TmdbCastMember>,
}

#[derive(Deserialize)]
struct CreditsResponse {
    crew: Vec<CrewItem>,
    cast: Vec<CastItem>,
}

#[derive(Deserialize)]
struct CrewItem {
    id: i64,
    name: String,
    job: String,
    department: String,
}

#[derive(Deserialize)]
struct CastItem {
    id: i64,
    name: String,
    character: String,
}

#[derive(Deserialize)]
struct MovieDetailsResponse {
    id: i64,
    title: String,
    original_title: Option<String>,
    release_date: Option<String>,
    overview: Option<String>,
    vote_average: Option<f64>,
    poster_path: Option<String>,
    credits: CreditsResponse,
}

#[tauri::command]
pub async fn get_tmdb_movie_credits(
    api_key: String,
    tmdb_id: i64,
) -> Result<TmdbMovieCredits, String> {
    let client = reqwest::Client::new();
    let url = format!(
        "https://api.themoviedb.org/3/movie/{}?append_to_response=credits&api_key={}&language=en-US",
        tmdb_id, api_key
    );

    let resp: MovieDetailsResponse = client
        .get(&url)
        .send()
        .await
        .map_err(|e| e.to_string())?
        .json()
        .await
        .map_err(|e| e.to_string())?;

    let year = resp
        .release_date
        .as_deref()
        .and_then(|d| d.get(..4))
        .and_then(|y| y.parse::<i64>().ok());

    let key_jobs = [
        "Director",
        "Director of Photography",
        "Original Music Composer",
        "Editor",
        "Screenplay",
        "Writer",
    ];
    let crew: Vec<TmdbCrewMember> = resp
        .credits
        .crew
        .into_iter()
        .filter(|m| key_jobs.contains(&m.job.as_str()))
        .map(|m| TmdbCrewMember {
            id: m.id,
            name: m.name,
            job: m.job,
            department: m.department,
        })
        .collect();

    let cast: Vec<TmdbCastMember> = resp
        .credits
        .cast
        .into_iter()
        .take(5)
        .map(|m| TmdbCastMember {
            id: m.id,
            name: m.name,
            character: m.character,
        })
        .collect();

    Ok(TmdbMovieCredits {
        tmdb_id: resp.id,
        title: resp.title,
        original_title: resp.original_title,
        year,
        overview: resp.overview,
        vote_average: resp.vote_average,
        poster_path: resp.poster_path,
        crew,
        cast,
    })
}

// ── Enrich credits (with cache) ─────────────────────────────

#[derive(Debug, Serialize)]
pub struct MovieCreditsEnriched {
    pub id: u64,
    pub director: Option<String>,
    pub cast: Vec<String>,
}

/// Fetch credits for a batch of TMDB IDs, using cache where possible.
async fn fetch_credits_for_id(
    client: &reqwest::Client,
    api_key: &str,
    id: u64,
) -> Option<MovieCreditsEnriched> {
    // Check cache first
    if let Some(entry) = crate::cache::credits_get(id) {
        return Some(MovieCreditsEnriched {
            id,
            director: entry.director,
            cast: entry.cast,
        });
    }

    // Fetch from TMDB
    tracing::debug!(tmdb_id = id, "fetching credits from TMDB API");
    let url = format!("https://api.themoviedb.org/3/movie/{id}");
    let resp = client
        .get(&url)
        .query(&[
            ("api_key", api_key),
            ("append_to_response", "credits"),
            ("language", "en-US"),
        ])
        .header("User-Agent", "blowup/0.1")
        .send()
        .await
        .ok()?;

    let details: MovieDetailsResponse = resp.json().await.ok()?;

    let director = details
        .credits
        .crew
        .iter()
        .find(|c| c.job == "Director")
        .map(|c| c.name.clone());

    let cast: Vec<String> = details
        .credits
        .cast
        .iter()
        .take(3)
        .map(|c| c.name.clone())
        .collect();

    // Write to cache
    crate::cache::credits_put(id, director.clone(), cast.clone());

    Some(MovieCreditsEnriched { id, director, cast })
}

#[tauri::command]
pub async fn enrich_movie_credits(
    api_key: String,
    ids: Vec<u64>,
) -> Result<Vec<MovieCreditsEnriched>, String> {
    let client = reqwest::Client::new();
    let mut results = Vec::new();
    for id in ids {
        if let Some(enriched) = fetch_credits_for_id(&client, &api_key, id).await {
            results.push(enriched);
        }
    }
    Ok(results)
}

// ── Enrich index entry with TMDB data (lazy load) ──────────────────────

/// Fetch TMDB movie details by ID and cache in the library index.
/// Returns the updated IndexEntry. If already enriched, returns immediately.
#[tauri::command]
pub async fn enrich_index_entry(
    tmdb_id: u64,
    force: Option<bool>,
    index: tauri::State<'_, crate::library_index::LibraryIndex>,
) -> Result<crate::library_index::IndexEntry, String> {
    let entry = index
        .get_entry(tmdb_id)
        .ok_or_else(|| "索引中未找到该电影".to_string())?;

    // Already enriched — return cached data (unless force refresh)
    if entry.poster_url.is_some() && !force.unwrap_or(false) {
        return Ok(entry);
    }

    let cfg = crate::config::load_config();
    let api_key = &cfg.tmdb.api_key;
    if api_key.is_empty() {
        return Err("TMDB API key not configured".into());
    }

    let client = reqwest::Client::new();
    let details: MovieDetails = client
        .get(format!("https://api.themoviedb.org/3/movie/{tmdb_id}"))
        .query(&[
            ("api_key", api_key.as_str()),
            ("append_to_response", "credits"),
            ("language", "en-US"),
        ])
        .header("User-Agent", "blowup/2.0")
        .send()
        .await
        .map_err(|e| e.to_string())?
        .json()
        .await
        .map_err(|e| e.to_string())?;

    // Download poster to local film directory (skip if file already exists)
    let root_dir = shellexpand::tilde(&cfg.library.root_dir).to_string();
    let film_dir = std::path::Path::new(&root_dir).join(&entry.path);
    let poster_local = film_dir.join("poster.jpg");

    let poster_url = if poster_local.exists() {
        // Already downloaded — use local path
        Some(poster_local.to_string_lossy().to_string())
    } else if let Some(poster_path) = &details.poster_path {
        let url = format!("https://image.tmdb.org/t/p/w300{poster_path}");
        match client.get(&url).send().await {
            Ok(resp) if resp.status().is_success() => match resp.bytes().await {
                Ok(bytes) => {
                    std::fs::create_dir_all(&film_dir).ok();
                    match std::fs::write(&poster_local, &bytes) {
                        Ok(()) => Some(poster_local.to_string_lossy().to_string()),
                        Err(_) => Some(url), // fallback to remote URL
                    }
                }
                Err(_) => Some(url),
            },
            _ => None,
        }
    } else {
        None
    };

    // Build credits map: role → [names]
    // Crew roles we care about (TMDB job → Chinese label)
    let crew_roles: &[(&str, &str)] = &[
        ("Director", "导演"),
        ("Writer", "编剧"),
        ("Screenplay", "编剧"),
        ("Director of Photography", "摄影"),
        ("Original Music Composer", "配乐"),
        ("Editor", "剪辑"),
        ("Producer", "制片"),
    ];

    let mut credits: std::collections::HashMap<String, Vec<String>> =
        std::collections::HashMap::new();

    for crew in &details.credits.crew {
        for &(job, label) in crew_roles {
            if crew.job == job {
                let entry = credits.entry(label.to_string()).or_default();
                if !entry.contains(&crew.name) {
                    entry.push(crew.name.clone());
                }
            }
        }
    }

    // Cast (top 6 by billing order)
    let mut cast_sorted = details.credits.cast.clone();
    cast_sorted.sort_by_key(|c| c.order);
    let cast_names: Vec<String> = cast_sorted.iter().take(6).map(|c| c.name.clone()).collect();
    if !cast_names.is_empty() {
        credits.insert("主演".to_string(), cast_names);
    }

    let meta = crate::library_index::EntryMetadata {
        poster_url,
        overview: Some(details.overview),
        rating: Some(details.vote_average),
        credits,
        original_title: details.original_title,
    };

    index
        .update_entry_metadata(tmdb_id, meta)
        .ok_or_else(|| "更新后未找到索引条目".to_string())
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
