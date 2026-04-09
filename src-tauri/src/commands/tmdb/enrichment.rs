use super::MovieDetails;
use tauri::Emitter;

/// Fetch TMDB movie details by ID and cache in the library index.
/// Returns the updated IndexEntry. If already enriched, returns immediately.
#[tauri::command]
pub async fn enrich_index_entry(
    app: tauri::AppHandle,
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

    let year = details
        .release_date
        .split('-')
        .next()
        .and_then(|y| y.parse::<u32>().ok());
    let genres: Vec<String> = details.genres.iter().map(|g| g.name.clone()).collect();

    let meta = crate::library_index::EntryMetadata {
        title: Some(details.title),
        year,
        genres: if genres.is_empty() { None } else { Some(genres) },
        poster_url,
        overview: Some(details.overview),
        rating: Some(details.vote_average),
        credits,
        original_title: details.original_title,
    };

    let result = index
        .update_entry_metadata(tmdb_id, meta)
        .ok_or_else(|| "更新后未找到索引条目".to_string());
    if result.is_ok() {
        if let Err(e) = app.emit("library:changed", ()) {
            tracing::warn!(error = %e, "failed to emit library:changed");
        }
    }
    result
}
