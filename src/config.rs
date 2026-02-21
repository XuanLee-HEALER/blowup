use serde::Deserialize;
use std::path::PathBuf;

#[derive(Debug, Deserialize)]
pub struct Config {
    #[serde(default)]
    pub tools: ToolsConfig,
    #[serde(default)]
    pub search: SearchConfig,
    #[serde(default)]
    pub subtitle: SubtitleConfig,
    #[serde(default)]
    pub opensubtitles: OpenSubtitlesConfig,
}

#[derive(Debug, Deserialize)]
pub struct ToolsConfig {
    #[serde(default = "default_aria2c")]
    pub aria2c: String,
    #[serde(default = "default_alass")]
    pub alass: String,
}

#[derive(Debug, Deserialize)]
pub struct SearchConfig {
    #[serde(default = "default_rate_limit")]
    pub rate_limit_secs: u64,
}

#[derive(Debug, Deserialize)]
pub struct SubtitleConfig {
    #[serde(default = "default_lang")]
    pub default_lang: String,
}

#[derive(Debug, Deserialize)]
pub struct OpenSubtitlesConfig {
    #[serde(default)]
    pub api_key: String,
}

fn default_aria2c() -> String { "aria2c".to_string() }
fn default_alass() -> String { "alass".to_string() }
fn default_rate_limit() -> u64 { 5 }
fn default_lang() -> String { "zh".to_string() }

impl Default for ToolsConfig {
    fn default() -> Self {
        Self { aria2c: default_aria2c(), alass: default_alass() }
    }
}
impl Default for SearchConfig {
    fn default() -> Self { Self { rate_limit_secs: default_rate_limit() } }
}
impl Default for SubtitleConfig {
    fn default() -> Self { Self { default_lang: default_lang() } }
}
impl Default for OpenSubtitlesConfig {
    fn default() -> Self { Self { api_key: String::new() } }
}
impl Default for Config {
    fn default() -> Self {
        Self {
            tools: ToolsConfig::default(),
            search: SearchConfig::default(),
            subtitle: SubtitleConfig::default(),
            opensubtitles: OpenSubtitlesConfig::default(),
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
        assert_eq!(cfg.search.rate_limit_secs, 5);
        assert_eq!(cfg.subtitle.default_lang, "zh");
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
        assert_eq!(cfg.search.rate_limit_secs, 5);
    }
}
