use crate::common::normalize_director_name;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::RwLock;

pub const VIDEO_EXTENSIONS: &[&str] = &[
    "mp4", "mkv", "avi", "mov", "ts", "flv", "wmv", "webm", "m4v",
];

const AUDIO_EXTENSIONS: &[&str] = &[
    "mp3", "aac", "flac", "opus", "m4a", "wav", "ogg", "ac3", "dts", "mka",
];

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct IndexEntry {
    pub tmdb_id: u64,
    pub title: String,
    pub director: String,
    pub director_display: String,
    pub year: Option<u32>,
    pub genres: Vec<String>,
    pub path: String,
    pub files: Vec<String>,
    pub added_at: String,
    // TMDB enriched data (lazy-loaded on first view, cached in index JSON)
    #[serde(default)]
    pub poster_url: Option<String>,
    #[serde(default)]
    pub overview: Option<String>,
    #[serde(default)]
    pub rating: Option<f64>,
    /// Credits grouped by role: {"导演": ["name"], "主演": ["a","b"], "编剧": ["c"], ...}
    #[serde(default)]
    pub credits: HashMap<String, Vec<String>>,
    #[serde(default)]
    pub original_title: Option<String>,
    /// Cached media info per video file (keyed by filename).
    #[serde(default)]
    pub media_info: HashMap<String, FileMediaInfo>,
}

/// Cached media probe result for a single video file.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileMediaInfo {
    pub file_size: Option<i64>,
    pub duration_secs: Option<f64>,
    pub format_name: Option<String>,
    pub bit_rate: Option<i64>,
    pub streams: Vec<FileStreamInfo>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileStreamInfo {
    pub index: i64,
    pub codec_type: String,
    pub codec_name: String,
    pub width: Option<i64>,
    pub height: Option<i64>,
    pub frame_rate: Option<String>,
    pub bit_rate: Option<i64>,
    pub channels: Option<i64>,
    pub sample_rate: Option<String>,
    pub language: Option<String>,
    pub title: Option<String>,
}

/// Patch for updating TMDB metadata on an IndexEntry.
pub struct EntryMetadata {
    pub title: Option<String>,
    pub year: Option<u32>,
    pub genres: Option<Vec<String>>,
    pub poster_url: Option<String>,
    pub overview: Option<String>,
    pub rating: Option<f64>,
    pub credits: HashMap<String, Vec<String>>,
    pub original_title: Option<String>,
}

#[derive(Debug, Default, Serialize, Deserialize)]
struct IndexFile {
    #[serde(default)]
    version: u32,
    #[serde(default)]
    entries: Vec<IndexEntry>,
}

pub struct LibraryIndex {
    root: PathBuf,
    data: RwLock<IndexFile>,
}

impl LibraryIndex {
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Load index from disk, or create empty if missing/corrupted.
    pub fn load(root: &Path) -> Self {
        let index_path = root.join(".index.json");
        let data = if index_path.exists() {
            match std::fs::read_to_string(&index_path) {
                Ok(content) => match serde_json::from_str::<IndexFile>(&content) {
                    Ok(idx) => {
                        tracing::info!(entries = idx.entries.len(), "library index loaded");
                        idx
                    }
                    Err(e) => {
                        tracing::warn!(error = %e, "index corrupted, rebuilding");
                        IndexFile::default()
                    }
                },
                Err(e) => {
                    tracing::warn!(error = %e, "failed to read index, creating new");
                    IndexFile::default()
                }
            }
        } else {
            tracing::debug!("no index file found, creating new");
            IndexFile::default()
        };

        Self {
            root: root.to_path_buf(),
            data: RwLock::new(data),
        }
    }

    fn index_path(&self) -> PathBuf {
        self.root.join(".index.json")
    }

    fn save(&self) {
        // Serialize under read lock, then drop lock before disk I/O
        let content = {
            let data = self.data.read().unwrap();
            serde_json::to_string_pretty(&*data)
        };
        match content {
            Ok(content) => {
                if let Some(parent) = self.index_path().parent() {
                    std::fs::create_dir_all(parent).ok();
                }
                if let Err(e) = std::fs::write(self.index_path(), content) {
                    tracing::error!(error = %e, "failed to write index");
                }
            }
            Err(e) => tracing::error!(error = %e, "failed to serialize index"),
        }
    }

    pub fn flush(&self) {
        self.save();
        tracing::info!("library index flushed");
    }

    /// Add or update an entry. Creates directory if needed.
    pub fn add_entry(&self, entry: IndexEntry) -> Result<(), String> {
        // Create directory
        let dir = self.root.join(&entry.path);
        std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;

        let mut data = self.data.write().unwrap();
        data.entries.retain(|e| e.tmdb_id != entry.tmdb_id);
        data.entries.push(entry);
        drop(data);
        self.save();
        Ok(())
    }

    /// Remove entry by tmdb_id.
    pub fn remove_entry(&self, tmdb_id: u64) {
        let mut data = self.data.write().unwrap();
        data.entries.retain(|e| e.tmdb_id != tmdb_id);
        drop(data);
        self.save();
    }

    /// Get entry by tmdb_id.
    pub fn get_entry(&self, tmdb_id: u64) -> Option<IndexEntry> {
        let data = self.data.read().unwrap();
        data.entries.iter().find(|e| e.tmdb_id == tmdb_id).cloned()
    }

    /// Return all entries.
    pub fn list_entries(&self) -> Vec<IndexEntry> {
        let data = self.data.read().unwrap();
        data.entries.clone()
    }

    /// Group entries by normalized director name.
    pub fn list_by_director(&self) -> BTreeMap<String, Vec<IndexEntry>> {
        let data = self.data.read().unwrap();
        let mut map: BTreeMap<String, Vec<IndexEntry>> = BTreeMap::new();
        for entry in &data.entries {
            map.entry(entry.director_display.clone())
                .or_default()
                .push(entry.clone());
        }
        map
    }

    /// Search entries by title, year range, and genres.
    pub fn search(
        &self,
        query: Option<&str>,
        year_from: Option<u32>,
        year_to: Option<u32>,
        genre: Option<&str>,
    ) -> Vec<IndexEntry> {
        let data = self.data.read().unwrap();
        data.entries
            .iter()
            .filter(|e| {
                if let Some(q) = query {
                    let q_lower = q.to_lowercase();
                    if !e.title.to_lowercase().contains(&q_lower)
                        && !e.director_display.to_lowercase().contains(&q_lower)
                    {
                        return false;
                    }
                }
                if let Some(from) = year_from
                    && e.year.is_none_or(|y| y < from)
                {
                    return false;
                }
                if let Some(to) = year_to
                    && e.year.is_none_or(|y| y > to)
                {
                    return false;
                }
                if let Some(g) = genre
                    && !e.genres.iter().any(|eg| eg.eq_ignore_ascii_case(g))
                {
                    return false;
                }
                true
            })
            .cloned()
            .collect()
    }

    /// Update file list for a specific entry by scanning its directory.
    pub fn update_files(&self, tmdb_id: u64) {
        let mut data = self.data.write().unwrap();
        if let Some(entry) = data.entries.iter_mut().find(|e| e.tmdb_id == tmdb_id) {
            let dir = self.root.join(&entry.path);
            entry.files = scan_dir_files(&dir);
        }
        drop(data);
        self.save();
    }

    /// Rebuild index from directory structure on disk.
    /// Walks {root}/{director_dir}/{id_dir}/ and reconstructs entries.
    /// Only recovers tmdb_id, director, path, and files — other metadata is lost.
    pub fn rebuild_from_disk(&self) {
        let mut entries = Vec::new();
        let root = &self.root;

        let Ok(dir_iter) = std::fs::read_dir(root) else {
            return;
        };

        for director_entry in dir_iter.flatten() {
            if !director_entry.path().is_dir() {
                continue;
            }
            let director_name = director_entry.file_name().to_string_lossy().to_string();
            if director_name.starts_with('.') {
                continue;
            }

            let Ok(movie_iter) = std::fs::read_dir(director_entry.path()) else {
                continue;
            };

            for movie_entry in movie_iter.flatten() {
                if !movie_entry.path().is_dir() {
                    continue;
                }
                let id_str = movie_entry.file_name().to_string_lossy().to_string();
                let Ok(tmdb_id) = id_str.parse::<u64>() else {
                    continue;
                };

                let rel_path = format!("{}/{}", director_name, id_str);
                let files = scan_dir_files(&movie_entry.path());

                entries.push(IndexEntry {
                    tmdb_id,
                    title: format!("Unknown ({})", tmdb_id),
                    director: normalize_director_name(&director_name),
                    director_display: director_name.clone(),
                    year: None,
                    genres: Vec::new(),
                    path: rel_path,
                    files,
                    added_at: chrono::Utc::now().to_rfc3339(),
                    ..Default::default()
                });
            }
        }

        tracing::info!(entries = entries.len(), "index rebuilt from disk");
        let mut data = self.data.write().unwrap();
        data.entries = entries;
        data.version = 1;
        drop(data);
        self.save();
    }

    /// Compute the output directory for a download.
    pub fn compute_download_path(&self, director_raw: &str, tmdb_id: u64) -> PathBuf {
        let dir_name = normalize_director_name(director_raw);
        self.root.join(&dir_name).join(tmdb_id.to_string())
    }

    /// Store cached media info for a video file within an entry.
    pub fn set_file_media_info(
        &self,
        tmdb_id: u64,
        filename: &str,
        info: FileMediaInfo,
    ) -> Option<FileMediaInfo> {
        let mut data = self.data.write().unwrap();
        let result = if let Some(entry) = data.entries.iter_mut().find(|e| e.tmdb_id == tmdb_id) {
            entry.media_info.insert(filename.to_string(), info.clone());
            Some(info)
        } else {
            None
        };
        drop(data);
        self.save();
        result
    }

    /// Update TMDB metadata for an entry and persist. Returns the updated entry.
    pub fn update_entry_metadata(&self, tmdb_id: u64, meta: EntryMetadata) -> Option<IndexEntry> {
        let mut data = self.data.write().unwrap();
        let result = if let Some(entry) = data.entries.iter_mut().find(|e| e.tmdb_id == tmdb_id) {
            if let Some(title) = meta.title {
                entry.title = title;
            }
            if let Some(year) = meta.year {
                entry.year = Some(year);
            }
            if let Some(genres) = meta.genres {
                entry.genres = genres;
            }
            entry.poster_url = meta.poster_url;
            entry.overview = meta.overview;
            entry.rating = meta.rating;
            entry.credits = meta.credits;
            entry.original_title = meta.original_title;
            Some(entry.clone())
        } else {
            None
        };
        drop(data);
        self.save();
        result
    }
}

pub fn scan_dir_files(dir: &Path) -> Vec<String> {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    entries
        .flatten()
        .filter_map(|e| {
            let path = e.path();
            if !path.is_file() {
                return None;
            }
            let name = e.file_name().to_string_lossy().to_string();
            // Skip generated overlay files
            if name.starts_with(".blowup_") {
                return None;
            }
            let ext = path.extension()?.to_str()?.to_lowercase();
            if VIDEO_EXTENSIONS.contains(&ext.as_str())
                || AUDIO_EXTENSIONS.contains(&ext.as_str())
                || ext == "srt"
                || ext == "ass"
                || ext == "sub"
                || ext == "idx"
            {
                Some(name)
            } else {
                None
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    fn make_index(dir: &Path) -> LibraryIndex {
        LibraryIndex::load(dir)
    }

    fn sample_entry(tmdb_id: u64, title: &str, director: &str) -> IndexEntry {
        IndexEntry {
            tmdb_id,
            title: title.to_string(),
            director: normalize_director_name(director),
            director_display: director.to_string(),
            year: Some(1966),
            genres: vec!["Drama".to_string()],
            path: format!("{}/{}", normalize_director_name(director), tmdb_id),
            files: Vec::new(),
            added_at: "2026-01-01T00:00:00Z".to_string(),
            ..Default::default()
        }
    }

    #[test]
    fn add_and_get_entry() {
        let dir = tempdir().unwrap();
        let idx = make_index(dir.path());
        idx.add_entry(sample_entry(1052, "Blow-Up", "Michelangelo Antonioni"))
            .unwrap();
        let entry = idx.get_entry(1052).unwrap();
        assert_eq!(entry.title, "Blow-Up");
    }

    #[test]
    fn list_by_director_groups() {
        let dir = tempdir().unwrap();
        let idx = make_index(dir.path());
        idx.add_entry(sample_entry(1, "Film A", "Director X"))
            .unwrap();
        idx.add_entry(sample_entry(2, "Film B", "Director X"))
            .unwrap();
        idx.add_entry(sample_entry(3, "Film C", "Director Y"))
            .unwrap();
        let map = idx.list_by_director();
        assert_eq!(map.get("Director X").unwrap().len(), 2);
        assert_eq!(map.get("Director Y").unwrap().len(), 1);
    }

    #[test]
    fn search_by_title() {
        let dir = tempdir().unwrap();
        let idx = make_index(dir.path());
        idx.add_entry(sample_entry(1, "Blow-Up", "Antonioni"))
            .unwrap();
        idx.add_entry(sample_entry(2, "Persona", "Bergman"))
            .unwrap();
        let results = idx.search(Some("blow"), None, None, None);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].tmdb_id, 1);
    }

    #[test]
    fn search_by_genre() {
        let dir = tempdir().unwrap();
        let idx = make_index(dir.path());
        idx.add_entry(sample_entry(1, "Film", "Dir")).unwrap();
        let results = idx.search(None, None, None, Some("Drama"));
        assert_eq!(results.len(), 1);
        let results = idx.search(None, None, None, Some("Comedy"));
        assert!(results.is_empty());
    }

    #[test]
    fn remove_entry() {
        let dir = tempdir().unwrap();
        let idx = make_index(dir.path());
        idx.add_entry(sample_entry(1, "Film", "Dir")).unwrap();
        idx.remove_entry(1);
        assert!(idx.get_entry(1).is_none());
    }

    #[test]
    fn persistence_across_loads() {
        let dir = tempdir().unwrap();
        {
            let idx = make_index(dir.path());
            idx.add_entry(sample_entry(42, "Test", "Dir")).unwrap();
        }
        let idx2 = make_index(dir.path());
        assert!(idx2.get_entry(42).is_some());
    }

    #[test]
    fn rebuild_from_disk_finds_dirs() {
        let dir = tempdir().unwrap();
        let movie_dir = dir.path().join("Some Director").join("999");
        std::fs::create_dir_all(&movie_dir).unwrap();
        std::fs::write(movie_dir.join("movie.mkv"), b"fake").unwrap();

        let idx = make_index(dir.path());
        idx.rebuild_from_disk();
        let entry = idx.get_entry(999).unwrap();
        assert_eq!(entry.director_display, "Some Director");
        assert!(entry.files.contains(&"movie.mkv".to_string()));
    }

    #[test]
    fn update_files_scans_directory() {
        let dir = tempdir().unwrap();
        let idx = make_index(dir.path());
        let entry = sample_entry(1, "Film", "Dir");
        let movie_dir = dir.path().join(&entry.path);
        std::fs::create_dir_all(&movie_dir).unwrap();
        idx.add_entry(entry).unwrap();

        std::fs::write(movie_dir.join("video.mp4"), b"fake").unwrap();
        std::fs::write(movie_dir.join("sub.srt"), b"fake").unwrap();
        std::fs::write(movie_dir.join("readme.txt"), b"ignore").unwrap();

        idx.update_files(1);
        let entry = idx.get_entry(1).unwrap();
        assert_eq!(entry.files.len(), 2);
        assert!(entry.files.contains(&"video.mp4".to_string()));
        assert!(entry.files.contains(&"sub.srt".to_string()));
    }
}
