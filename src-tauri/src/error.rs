use thiserror::Error;

#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("Failed to read config file: {0}")]
    ReadFailed(#[from] std::io::Error),
    #[error("Failed to parse config: {0}")]
    ParseFailed(#[from] toml::de::Error),
}

#[derive(Debug, Error)]
pub enum SearchError {
    #[error("HTTP request failed: {0}")]
    HttpFailed(#[from] reqwest::Error),
    #[error("No results found for query: {0}")]
    NoResults(String),
    #[error("CDP browser not available: {0}")]
    CdpUnavailable(String),
}

#[derive(Debug, Error)]
pub enum SubError {
    #[error("Subtitle source returned no results")]
    NoSubtitleFound,
    #[error("HTTP request failed: {0}")]
    HttpFailed(#[from] reqwest::Error),
    #[error("alass not found in PATH")]
    AlassNotFound,
    #[error("alass failed: {0}")]
    AlassFailed(String),
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Invalid SRT format: {0}")]
    InvalidSrt(String),
}

#[derive(Debug, Error)]
pub enum TmdbError {
    #[error(
        "TMDB API key not configured.\nRun: blowup config set tmdb.api_key YOUR_KEY\nGet a free key at: https://www.themoviedb.org/settings/api"
    )]
    ApiKeyMissing,
    #[error("Movie not found: {0}")]
    NotFound(String),
    #[error("Failed to parse TMDB response: {0}")]
    ParseFailed(String),
    #[error("HTTP request failed: {0}")]
    HttpFailed(#[from] reqwest::Error),
}

#[derive(Debug, Error)]
pub enum ConfigCmdError {
    #[error("Invalid key format: '{0}' (expected: section.field, e.g. tmdb.api_key)")]
    InvalidKeyFormat(String),
    #[error("Unknown config key: '{0}'")]
    UnknownKey(String),
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("TOML parse error: {0}")]
    TomlParse(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn search_error_display() {
        let e = SearchError::NoResults("Blow-Up 1966".to_string());
        assert_eq!(e.to_string(), "No results found for query: Blow-Up 1966");
    }

    #[test]
    fn tmdb_error_api_key_missing_display() {
        let e = TmdbError::ApiKeyMissing;
        assert!(e.to_string().contains("blowup config set tmdb.api_key"));
    }

    #[test]
    fn config_cmd_error_invalid_format_display() {
        let e = ConfigCmdError::InvalidKeyFormat("noDot".to_string());
        assert!(e.to_string().contains("section.field"));
    }
}
