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
pub enum DownloadError {
    #[error("aria2c not found in PATH")]
    Aria2cNotFound,
    #[error("aria2c failed: {0}")]
    Aria2cFailed(String),
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn search_error_display() {
        let e = SearchError::NoResults("Blow-Up 1966".to_string());
        assert_eq!(e.to_string(), "No results found for query: Blow-Up 1966");
    }

    #[test]
    fn download_error_display() {
        let e = DownloadError::Aria2cNotFound;
        assert_eq!(e.to_string(), "aria2c not found in PATH");
    }
}
