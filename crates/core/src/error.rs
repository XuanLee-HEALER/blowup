use thiserror::Error;

/// Tagged string errors for adapter-level HTTP status mapping.
///
/// Core service functions keep their ergonomic `Result<T, String>`
/// signature. For errors that should *explicitly* map to a non-500
/// HTTP status, they produce the string via one of these helpers so
/// the axum adapter can do a prefix match instead of a substring
/// match on the human-readable body (which breaks every time a
/// sentence gets reworded).
///
/// The Tauri adapter ignores the prefixes — it just forwards the
/// whole string to the frontend as-is, so the prefix shows up in the
/// UI error banner. That's a feature: it flags at a glance that the
/// error is "expected" (404) vs. "something exploded" (no prefix).
pub mod status {
    pub const NOT_FOUND_PREFIX: &str = "not_found: ";
    pub const BAD_REQUEST_PREFIX: &str = "bad_request: ";

    /// Tag an error string as "404 Not Found" for the HTTP adapter.
    pub fn not_found(msg: impl Into<String>) -> String {
        format!("{}{}", NOT_FOUND_PREFIX, msg.into())
    }

    /// Tag an error string as "400 Bad Request" for the HTTP adapter.
    pub fn bad_request(msg: impl Into<String>) -> String {
        format!("{}{}", BAD_REQUEST_PREFIX, msg.into())
    }
}

#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("Failed to read config file: {0}")]
    ReadFailed(#[from] std::io::Error),
    #[error("Failed to parse config: {0}")]
    ParseFailed(#[from] toml::de::Error),
}

#[derive(Debug, Error)]
pub enum SubError {
    #[error("Subtitle source returned no results")]
    NoSubtitleFound,
    #[error("HTTP request failed: {0}")]
    HttpFailed(#[from] reqwest::Error),
    #[error("alass alignment failed: {0}")]
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
