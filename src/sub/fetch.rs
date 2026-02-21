// Task 8: subtitle fetch from Assrt/OpenSubtitles (to be implemented)
use std::path::Path;
use crate::error::SubError;
use crate::config::Config;

pub enum SubSource {
    Assrt,
    OpenSubtitles,
    All,
}

pub struct SubtitleResult {
    pub filename: String,
    pub lang: String,
    pub source: String,
}

pub async fn fetch_subtitle(
    _video: &Path,
    _lang: &str,
    _source: SubSource,
    _cfg: &Config,
) -> Result<(), SubError> {
    todo!()
}
