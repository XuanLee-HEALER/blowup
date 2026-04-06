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

fn app_data_dir() -> PathBuf {
    APP_DATA_DIR
        .get()
        .cloned()
        .unwrap_or_else(|| {
            dirs::data_dir()
                .unwrap_or_else(|| PathBuf::from("."))
                .join("blowup")
        })
}

#[derive(Debug, Default, Deserialize, Serialize)]
pub struct Config {
    #[serde(default)]
    pub tools: ToolsConfig,
    #[serde(default)]
    pub search: SearchConfig,
    #[serde(default)]
    pub subtitle: SubtitleConfig,
    #[serde(default)]
    pub opensubtitles: OpenSubtitlesConfig,
    #[serde(default)]
    pub tmdb: TmdbConfig,
    #[serde(default)]
    pub library: LibraryConfig,
    #[serde(default)]
    pub music: MusicConfig,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct ToolsConfig {
    #[serde(default = "default_aria2c")]
    pub aria2c: String,
    #[serde(default = "default_alass")]
    pub alass: String,
    #[serde(default = "default_ffmpeg")]
    pub ffmpeg: String,
    #[serde(default = "default_player")]
    pub player: String,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct SearchConfig {
    #[serde(default = "default_rate_limit")]
    pub rate_limit_secs: u64,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct SubtitleConfig {
    #[serde(default = "default_lang")]
    pub default_lang: String,
}

#[derive(Debug, Default, Deserialize, Serialize)]
pub struct OpenSubtitlesConfig {
    #[serde(default)]
    pub api_key: String,
}

#[derive(Debug, Default, Deserialize, Serialize)]
pub struct TmdbConfig {
    #[serde(default)]
    pub api_key: String,
}

#[derive(Debug, Deserialize, Serialize)]
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

fn default_music_mode() -> String {
    "sequential".to_string()
}

fn default_player() -> String {
    "mpv".to_string()
}
fn default_aria2c() -> String {
    "aria2c".to_string()
}
fn default_alass() -> String {
    "alass".to_string()
}
fn default_ffmpeg() -> String {
    "ffmpeg".to_string()
}
fn default_rate_limit() -> u64 {
    5
}
fn default_lang() -> String {
    "zh".to_string()
}
fn default_root_dir() -> String {
    dirs::home_dir()
        .map(|h| {
            h.join("Movies")
                .join("blowup")
                .to_string_lossy()
                .into_owned()
        })
        .unwrap_or_else(|| "~/Movies/blowup".to_string())
}

impl Default for ToolsConfig {
    fn default() -> Self {
        Self {
            aria2c: default_aria2c(),
            alass: default_alass(),
            ffmpeg: default_ffmpeg(),
            player: default_player(),
        }
    }
}
impl Default for SearchConfig {
    fn default() -> Self {
        Self {
            rate_limit_secs: default_rate_limit(),
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
        assert_eq!(cfg.tools.aria2c, "aria2c");
        assert_eq!(cfg.tools.alass, "alass");
        assert_eq!(cfg.tools.ffmpeg, "ffmpeg");
        assert_eq!(cfg.tools.player, "mpv");
        assert_eq!(cfg.search.rate_limit_secs, 5);
        assert_eq!(cfg.subtitle.default_lang, "zh");
    }

    #[test]
    fn tmdb_default_api_key_is_empty() {
        let cfg = Config::default();
        assert_eq!(cfg.tmdb.api_key, "");
    }

    #[test]
    fn parse_partial_toml() {
        let toml = r#"
[tools]
aria2c = "/usr/local/bin/aria2c"
"#;
        let cfg: Config = toml::from_str(toml).unwrap();
        assert_eq!(cfg.tools.aria2c, "/usr/local/bin/aria2c");
        assert_eq!(cfg.tools.alass, "alass");
        assert_eq!(cfg.tools.ffmpeg, "ffmpeg");
        assert_eq!(cfg.search.rate_limit_secs, 5);
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
        assert!(serialized.contains("aria2c"));
        assert!(serialized.contains("ffmpeg"));
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
