use serde::{Deserialize, Serialize};
use std::path::PathBuf;

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
}

#[derive(Debug, Deserialize, Serialize)]
pub struct ToolsConfig {
    #[serde(default = "default_aria2c")]
    pub aria2c: String,
    #[serde(default = "default_alass")]
    pub alass: String,
    #[serde(default = "default_ffmpeg")]
    pub ffmpeg: String,
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

pub fn config_path() -> PathBuf {
    dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("blowup")
        .join("config.toml")
}

pub fn load_config() -> Config {
    let path = config_path();
    if !path.exists() {
        return Config::default();
    }
    let content = std::fs::read_to_string(&path).unwrap_or_default();
    toml::from_str(&content).unwrap_or_default()
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
}
