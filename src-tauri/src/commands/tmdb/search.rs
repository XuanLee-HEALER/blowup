use super::{
    GenreListResponse, ListResponse, MovieListItem, PersonSearchResponse, SearchFilters, TmdbGenre,
    build_discover_params, to_list_item,
};

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
        .filter(|i| {
            // TMDB /search/movie doesn't support date range params,
            // so filter client-side to respect year filters
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
