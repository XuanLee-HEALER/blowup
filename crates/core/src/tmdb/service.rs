//! TMDB HTTP client — pure functions that accept an explicit
//! `reqwest::Client` and API key. No DB, no file IO (except the
//! poster-download step in `enrich_index_entry`, which also
//! touches `LibraryIndex`). No Tauri coupling.
//!
//! Callers:
//! - `blowup-tauri`'s `commands/tmdb/*` thin wrappers
//! - (future) `blowup-server` HTTP route handlers

use crate::error::TmdbError;
use crate::infra::cache;
use crate::library::index::{EntryMetadata, IndexEntry, LibraryIndex};
use crate::tmdb::model::{
    MovieCreditsEnriched, MovieListItem, SearchFilters, TmdbCastMember, TmdbCrewMember, TmdbGenre,
    TmdbMovie, TmdbMovieCredits,
};
use serde::Deserialize;
use std::collections::HashMap;
use std::path::Path;

// ── Internal deserialization structs ─────────────────────────────

#[derive(Deserialize)]
struct SearchResponse {
    results: Vec<SearchResult>,
}

#[derive(Deserialize)]
struct SearchResult {
    id: u64,
}

#[derive(Deserialize)]
pub(crate) struct MovieDetails {
    pub(crate) title: String,
    #[serde(default)]
    pub(crate) original_title: Option<String>,
    pub(crate) release_date: String,
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

#[derive(Deserialize)]
struct ListResponse {
    results: Vec<ListItem>,
}

#[derive(Deserialize)]
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

#[derive(Deserialize)]
struct GenreListResponse {
    genres: Vec<GenreItem>,
}

#[derive(Deserialize)]
struct GenreItem {
    id: u64,
    name: String,
}

#[derive(Deserialize)]
struct PersonSearchResponse {
    results: Vec<PersonItem>,
}

#[derive(Deserialize)]
struct PersonItem {
    id: u64,
}

#[derive(Deserialize)]
struct CreditsMovieDetailsResponse {
    id: i64,
    title: String,
    original_title: Option<String>,
    release_date: Option<String>,
    overview: Option<String>,
    vote_average: Option<f64>,
    poster_path: Option<String>,
    credits: CreditsResponse,
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

// ── Helpers ──────────────────────────────────────────────────────

pub fn extract_year(release_date: &str) -> String {
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
    // Collect all directors (some films have 2+, e.g. Coen brothers, Wachowskis,
    // Taiwanese GATAO franchise etc.). TMDB returns one crew row per director.
    let directors: Vec<String> = details
        .credits
        .crew
        .iter()
        .filter(|c| c.job == "Director")
        .map(|c| c.name.clone())
        .collect();
    let director = if directors.is_empty() {
        "N/A".to_string()
    } else {
        directors.join(", ")
    };
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
        ("vote_count.gte", "50".to_string()),
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

// ── Service functions ────────────────────────────────────────────

/// Search TMDB by title + fetch details with credits for the first hit.
pub async fn query_tmdb(
    client: &reqwest::Client,
    api_key: &str,
    title: &str,
    year: Option<u32>,
) -> Result<TmdbMovie, TmdbError> {
    if api_key.is_empty() {
        return Err(TmdbError::ApiKeyMissing);
    }

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

/// Search by title, optionally merge with director/person results.
pub async fn search_movies(
    client: &reqwest::Client,
    api_key: &str,
    query: &str,
    filters: &SearchFilters,
) -> Result<Vec<MovieListItem>, String> {
    if api_key.is_empty() {
        return Err("TMDB API key not configured".into());
    }
    let page = filters.page.unwrap_or(1);

    let params: Vec<(&str, String)> = vec![
        ("api_key", api_key.to_string()),
        ("query", query.to_string()),
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
        .filter(|i| {
            let year: Option<u32> = i
                .release_date
                .as_deref()
                .and_then(|d| d.get(..4))
                .and_then(|y| y.parse().ok());
            if let Some(from) = filters.year_from
                && year.is_none_or(|y| y < from)
            {
                return false;
            }
            if let Some(to) = filters.year_to
                && year.is_none_or(|y| y > to)
            {
                return false;
            }
            true
        })
        .map(|i| {
            seen.insert(i.id);
            to_list_item(i)
        })
        .collect();

    // Person search → discover
    let person_resp: Result<PersonSearchResponse, _> = client
        .get("https://api.themoviedb.org/3/search/person")
        .query(&[("api_key", api_key), ("query", query)])
        .send()
        .await
        .map_err(|e| e.to_string())?
        .json()
        .await;

    if let Ok(pr) = person_resp
        && let Some(person) = pr.results.first()
    {
        let mut disc_params = build_discover_params(api_key, filters);
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
pub async fn discover_movies(
    client: &reqwest::Client,
    api_key: &str,
    filters: &SearchFilters,
) -> Result<Vec<MovieListItem>, String> {
    if api_key.is_empty() {
        return Err("TMDB API key not configured".into());
    }
    let params = build_discover_params(api_key, filters);
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

/// Fetch TMDB genre list (frontend caches the result).
pub async fn list_genres(
    client: &reqwest::Client,
    api_key: &str,
) -> Result<Vec<TmdbGenre>, String> {
    if api_key.is_empty() {
        return Err("TMDB API key not configured".into());
    }
    let resp: GenreListResponse = client
        .get("https://api.themoviedb.org/3/genre/movie/list")
        .query(&[("api_key", api_key), ("language", "en-US")])
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

/// Fetch full credits (key crew + top cast) for a single TMDB movie.
pub async fn get_tmdb_movie_credits(
    client: &reqwest::Client,
    api_key: &str,
    tmdb_id: i64,
) -> Result<TmdbMovieCredits, String> {
    let url = format!(
        "https://api.themoviedb.org/3/movie/{}?append_to_response=credits&api_key={}&language=en-US",
        tmdb_id, api_key
    );

    let resp: CreditsMovieDetailsResponse = client
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

/// Fetch credits for one TMDB id, hitting the LRU cache first.
async fn fetch_credits_for_id(
    client: &reqwest::Client,
    api_key: &str,
    id: u64,
) -> Option<MovieCreditsEnriched> {
    if let Some(entry) = cache::credits_get(id) {
        return Some(MovieCreditsEnriched {
            id,
            director: entry.director,
            cast: entry.cast,
        });
    }

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

    let details: CreditsMovieDetailsResponse = resp.json().await.ok()?;

    // Collect all directors; join with ", " so multi-director films don't
    // drop the second name. Matches the normalize_director_name convention.
    let directors: Vec<String> = details
        .credits
        .crew
        .iter()
        .filter(|c| c.job == "Director")
        .map(|c| c.name.clone())
        .collect();
    let director = if directors.is_empty() {
        None
    } else {
        Some(directors.join(", "))
    };

    let cast: Vec<String> = details
        .credits
        .cast
        .iter()
        .take(3)
        .map(|c| c.name.clone())
        .collect();

    cache::credits_put(id, director.clone(), cast.clone());

    Some(MovieCreditsEnriched { id, director, cast })
}

/// Fetch credits for a batch of TMDB IDs, using the cache where possible.
pub async fn enrich_movie_credits(
    client: &reqwest::Client,
    api_key: &str,
    ids: Vec<u64>,
) -> Vec<MovieCreditsEnriched> {
    let mut results = Vec::new();
    for id in ids {
        if let Some(enriched) = fetch_credits_for_id(client, api_key, id).await {
            results.push(enriched);
        }
    }
    results
}

/// Enrich a library index entry with TMDB movie details (+ download poster
/// to the film directory). Returns the updated `IndexEntry`.
///
/// Callers must emit `library:changed` themselves after a successful call.
pub async fn enrich_index_entry(
    client: &reqwest::Client,
    api_key: &str,
    library_root: &Path,
    index: &LibraryIndex,
    tmdb_id: u64,
    force: bool,
) -> Result<IndexEntry, String> {
    let entry = index
        .get_entry(tmdb_id)
        .ok_or_else(|| "索引中未找到该电影".to_string())?;

    // Already enriched — return cached data (unless force refresh)
    if entry.poster_url.is_some() && !force {
        return Ok(entry);
    }

    if api_key.is_empty() {
        return Err("TMDB API key not configured".into());
    }

    let details: MovieDetails = client
        .get(format!("https://api.themoviedb.org/3/movie/{tmdb_id}"))
        .query(&[
            ("api_key", api_key),
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

    // Download poster to local film directory (skip if already exists)
    let film_dir = library_root.join(&entry.path);
    let poster_local = film_dir.join("poster.jpg");

    let poster_url = if poster_local.exists() {
        Some(poster_local.to_string_lossy().to_string())
    } else if let Some(poster_path) = &details.poster_path {
        let url = format!("https://image.tmdb.org/t/p/w300{poster_path}");
        match client.get(&url).send().await {
            Ok(resp) if resp.status().is_success() => match resp.bytes().await {
                Ok(bytes) => {
                    std::fs::create_dir_all(&film_dir).ok();
                    match std::fs::write(&poster_local, &bytes) {
                        Ok(()) => Some(poster_local.to_string_lossy().to_string()),
                        Err(_) => Some(url),
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
    let crew_roles: &[(&str, &str)] = &[
        ("Director", "导演"),
        ("Writer", "编剧"),
        ("Screenplay", "编剧"),
        ("Director of Photography", "摄影"),
        ("Original Music Composer", "配乐"),
        ("Editor", "剪辑"),
        ("Producer", "制片"),
    ];

    let mut credits: HashMap<String, Vec<String>> = HashMap::new();

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

    let year = details
        .release_date
        .split('-')
        .next()
        .and_then(|y| y.parse::<u32>().ok());
    let genres: Vec<String> = details.genres.iter().map(|g| g.name.clone()).collect();

    let meta = EntryMetadata {
        title: Some(details.title),
        year,
        genres: if genres.is_empty() {
            None
        } else {
            Some(genres)
        },
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
    fn parse_movie_details_joins_multiple_directors() {
        let mut details = sample_details();
        details.credits.crew = vec![
            CrewMember {
                job: "Director".to_string(),
                name: "Chiang Jui-chih".to_string(),
            },
            CrewMember {
                job: "Director of Photography".to_string(),
                name: "Yao Hung-i".to_string(),
            },
            CrewMember {
                job: "Director".to_string(),
                name: "Yao Hung-i".to_string(),
            },
        ];
        let movie = parse_movie_details(&details);
        assert_eq!(movie.director, "Chiang Jui-chih, Yao Hung-i");
    }

    #[test]
    fn parse_movie_details_no_director_falls_back_to_na() {
        let mut details = sample_details();
        details.credits.crew = vec![CrewMember {
            job: "Producer".to_string(),
            name: "Carlo Ponti".to_string(),
        }];
        let movie = parse_movie_details(&details);
        assert_eq!(movie.director, "N/A");
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
        let client = reqwest::Client::new();
        let result = query_tmdb(&client, "", "Blow-Up", None).await;
        assert!(matches!(result, Err(TmdbError::ApiKeyMissing)));
    }
}
