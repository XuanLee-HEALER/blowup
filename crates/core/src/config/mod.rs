use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::OnceLock;

/// Resolved app data directory, set once during Tauri setup.
static APP_DATA_DIR: OnceLock<PathBuf> = OnceLock::new();

/// Called during Tauri setup to store the resolved app data dir.
/// Config and DB will both live under this directory.
pub fn init_app_data_dir(dir: PathBuf) {
    APP_DATA_DIR.set(dir).ok();
}

pub fn app_data_dir() -> PathBuf {
    APP_DATA_DIR.get().cloned().unwrap_or_else(|| {
        dirs::data_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("blowup")
    })
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct Config {
    #[serde(default)]
    pub tools: ToolsConfig,
    #[serde(default)]
    pub subtitle: SubtitleConfig,
    #[serde(default)]
    pub opensubtitles: OpenSubtitlesConfig,
    #[serde(default)]
    pub assrt: AssrtConfig,
    #[serde(default)]
    pub tmdb: TmdbConfig,
    #[serde(default)]
    pub library: LibraryConfig,
    #[serde(default)]
    pub music: MusicConfig,
    #[serde(default)]
    pub cache: CacheConfig,
    #[serde(default)]
    pub download: DownloadConfig,
    #[serde(default)]
    pub sync: SyncConfig,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ToolsConfig {
    #[serde(default = "default_ffmpeg")]
    pub ffmpeg: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct SubtitleConfig {
    #[serde(default = "default_lang")]
    pub default_lang: String,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct OpenSubtitlesConfig {
    #[serde(default)]
    pub api_key: String,
    #[serde(default)]
    pub username: String,
    #[serde(default)]
    pub password: String,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct AssrtConfig {
    #[serde(default)]
    pub token: String,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct TmdbConfig {
    #[serde(default)]
    pub api_key: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct LibraryConfig {
    #[serde(default = "default_root_dir")]
    pub root_dir: String,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct MusicConfig {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default = "default_music_mode")]
    pub mode: String,
    #[serde(default)]
    pub playlist: Vec<MusicTrack>,
}

#[derive(Debug, Default, Deserialize, Serialize, Clone)]
pub struct MusicTrack {
    pub src: String,
    pub name: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct CacheConfig {
    #[serde(default = "default_cache_max_entries")]
    pub max_entries: usize,
}

impl Default for CacheConfig {
    fn default() -> Self {
        Self {
            max_entries: default_cache_max_entries(),
        }
    }
}

fn default_cache_max_entries() -> usize {
    200
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct DownloadConfig {
    #[serde(default = "default_max_concurrent")]
    pub max_concurrent: usize,
    #[serde(default = "default_enable_dht")]
    pub enable_dht: bool,
    #[serde(default)]
    pub persist_session: bool,
}

impl Default for DownloadConfig {
    fn default() -> Self {
        Self {
            max_concurrent: default_max_concurrent(),
            enable_dht: default_enable_dht(),
            persist_session: false,
        }
    }
}

fn default_max_concurrent() -> usize {
    3
}

fn default_enable_dht() -> bool {
    true
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct SyncConfig {
    #[serde(default)]
    pub endpoint: String,
    #[serde(default)]
    pub bucket: String,
    #[serde(default)]
    pub access_key: String,
    #[serde(default)]
    pub secret_key: String,
}

fn default_music_mode() -> String {
    "sequential".to_string()
}

fn default_ffmpeg() -> String {
    "ffmpeg".to_string()
}
fn default_lang() -> String {
    "zh".to_string()
}
fn default_root_dir() -> String {
    dirs::video_dir()
        .map(|v| v.join("blowup"))
        .or_else(|| dirs::home_dir().map(|h| h.join("Movies").join("blowup")))
        .map(|p| p.to_string_lossy().into_owned())
        .unwrap_or_else(|| "~/Movies/blowup".to_string())
}

impl Default for ToolsConfig {
    fn default() -> Self {
        Self {
            ffmpeg: default_ffmpeg(),
        }
    }
}
impl Default for SubtitleConfig {
    fn default() -> Self {
        Self {
            default_lang: default_lang(),
        }
    }
}
impl Default for LibraryConfig {
    fn default() -> Self {
        Self {
            root_dir: default_root_dir(),
        }
    }
}
impl Default for MusicConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            mode: default_music_mode(),
            playlist: Vec::new(),
        }
    }
}

pub fn config_path() -> PathBuf {
    app_data_dir().join("config.toml")
}

pub fn load_config() -> Config {
    let path = config_path();

    // Migrate from old location if new path doesn't exist yet
    if !path.exists() {
        let old_path = dirs::config_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("blowup")
            .join("config.toml");
        if old_path.exists() {
            if let Some(parent) = path.parent() {
                std::fs::create_dir_all(parent).ok();
            }
            std::fs::copy(&old_path, &path).ok();
        }
    }

    if !path.exists() {
        return Config::default();
    }
    let content = std::fs::read_to_string(&path).unwrap_or_default();
    toml::from_str(&content).unwrap_or_default()
}

/// Try to find `name` via PATH first, then fall back to well-known directories (macOS only).
/// macOS GUI apps don't inherit the shell PATH, so we probe Homebrew/usr paths manually.
fn find_tool(name: &str) -> Option<PathBuf> {
    if let Ok(p) = which::which(name) {
        return Some(p);
    }
    #[cfg(target_os = "macos")]
    for dir in &["/opt/homebrew/bin", "/usr/local/bin", "/usr/bin"] {
        let candidate = PathBuf::from(dir).join(name);
        if candidate.is_file() {
            return Some(candidate);
        }
    }
    None
}

/// Check tool paths in config; if invalid, try to find them in PATH and
/// well-known directories, then update and save.
/// Returns true if any path was changed.
pub fn resolve_tool_paths(config: &mut Config) -> bool {
    let mut changed = false;

    // ffmpeg — resolve to absolute path if not already
    if !PathBuf::from(&config.tools.ffmpeg).is_absolute()
        && let Some(p) = find_tool(&config.tools.ffmpeg).or_else(|| find_tool("ffmpeg"))
    {
        config.tools.ffmpeg = p.to_string_lossy().into_owned();
        changed = true;
    }

    if changed {
        save_config(config).ok();
    }
    changed
}

pub fn save_config(config: &Config) -> Result<(), String> {
    let path = config_path();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let content = toml::to_string_pretty(config).map_err(|e| e.to_string())?;
    std::fs::write(&path, content).map_err(|e| e.to_string())?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_config_has_sane_values() {
        let cfg = Config::default();
        assert_eq!(cfg.tools.ffmpeg, "ffmpeg");
        assert_eq!(cfg.subtitle.default_lang, "zh");
    }

    #[test]
    fn tmdb_default_api_key_is_empty() {
        let cfg = Config::default();
        assert_eq!(cfg.tmdb.api_key, "");
    }

    #[test]
    fn parse_partial_toml() {
        // Old config with alass field should still parse (backward compat)
        let toml = r#"
[tools]
alass = "/usr/local/bin/alass"
"#;
        let cfg: Config = toml::from_str(toml).unwrap();
        assert_eq!(cfg.tools.ffmpeg, "ffmpeg");
    }

    #[test]
    fn library_default_contains_blowup() {
        let cfg = Config::default();
        assert!(cfg.library.root_dir.contains("blowup"));
    }

    #[test]
    fn config_is_serializable() {
        let cfg = Config::default();
        let serialized = toml::to_string(&cfg).unwrap();
        assert!(serialized.contains("ffmpeg"));
        assert!(serialized.contains("max_concurrent"));
    }

    #[test]
    fn music_config_defaults() {
        let cfg = Config::default();
        assert!(!cfg.music.enabled);
        assert_eq!(cfg.music.mode, "sequential");
        assert!(cfg.music.playlist.is_empty());
    }

    #[test]
    fn parse_music_config_from_toml() {
        let toml_str = r#"
[music]
enabled = true
mode = "random"

[[music.playlist]]
src = "/tmp/song.mp3"
name = "Test Song"
"#;
        let cfg: Config = toml::from_str(toml_str).unwrap();
        assert!(cfg.music.enabled);
        assert_eq!(cfg.music.mode, "random");
        assert_eq!(cfg.music.playlist.len(), 1);
        assert_eq!(cfg.music.playlist[0].name, "Test Song");
    }
}
