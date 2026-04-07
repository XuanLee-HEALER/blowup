use std::path::PathBuf;

const TRACKER_URL: &str =
    "https://raw.githubusercontent.com/ngosang/trackerslist/master/trackers_all.txt";

pub fn tracker_list_path() -> PathBuf {
    dirs::data_local_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("blowup")
        .join("trackers.txt")
}

pub fn load_trackers() -> Vec<String> {
    let path = tracker_list_path();
    if !path.exists() {
        return vec![];
    }
    std::fs::read_to_string(path)
        .unwrap_or_default()
        .lines()
        .filter(|l| !l.trim().is_empty())
        .map(String::from)
        .collect()
}

/// Download latest tracker list from GitHub and save locally.
pub async fn update_tracker_list() -> Result<(), String> {
    let text = reqwest::get(TRACKER_URL)
        .await
        .map_err(|e| e.to_string())?
        .text()
        .await
        .map_err(|e| e.to_string())?;

    let path = tracker_list_path();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    std::fs::write(&path, &text).map_err(|e| e.to_string())?;

    tracing::info!(
        trackers = text.lines().filter(|l| !l.trim().is_empty()).count(),
        "tracker list updated"
    );
    Ok(())
}

#[tauri::command]
pub async fn update_trackers() -> Result<(), String> {
    update_tracker_list().await
}

#[cfg(test)]
mod tests {
    #[test]
    fn load_trackers_parses_content() {
        let content = "udp://tracker1.com\nudp://tracker2.com\n\n";
        let trackers: Vec<String> = content
            .lines()
            .filter(|l| !l.trim().is_empty())
            .map(String::from)
            .collect();
        assert_eq!(trackers.len(), 2);
        assert_eq!(trackers[0], "udp://tracker1.com");
    }
}
