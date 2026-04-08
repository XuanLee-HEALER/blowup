use serde::{Deserialize, Serialize};

// ── Types for get_tmdb_movie_credits ────────────────────────────

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

// ── Internal deserialization structs ────────────────────────────

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

// ── Commands ────────────────────────────────────────────────────

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

// ── Enrich credits (with cache) ─────────────────────────────────

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
