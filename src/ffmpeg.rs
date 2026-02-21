use std::{ffi::OsStr, path::Path, result};

use thiserror::Error;

use crate::common::{CommonError, exec_command, find_command_path};

pub enum FfmpegTool {
    Ffmpeg,
    Ffprobe,
}

impl FfmpegTool {
    pub async fn exec_with_options(
        &self,
        path: Option<impl AsRef<Path>>,
        options: Option<Vec<impl AsRef<OsStr>>>,
    ) -> Result<(String, String)> {
        let exec_path = match self {
            FfmpegTool::Ffmpeg => {
                find_command_path(path, FFMPEG_CLI).ok_or(FfmpegError::FfmpegNotFound)?
            }
            FfmpegTool::Ffprobe => {
                find_command_path(path, FFPROBE_CLI).ok_or(FfmpegError::FfmpegNotFound)?
            }
        };
        exec_command(exec_path, options).await.map_err(|e| e.into())
    }
}

#[cfg(target_family = "unix")]
const FFMPEG_CLI: &str = "ffmpeg";
#[cfg(target_family = "unix")]
const FFPROBE_CLI: &str = "ffprobe";
#[cfg(target_family = "windows")]
const FFMPEG_CLI: &str = "ffmpeg.exe";
#[cfg(target_family = "windows")]
const FFPROBE_CLI: &str = "ffprobe.exe";

#[derive(Debug, Error)]
pub enum FfmpegError {
    #[error("ffmpeg cli is not found")]
    FfmpegNotFound,
    #[error(transparent)]
    CmdExecError(CommonError),
}

impl From<CommonError> for FfmpegError {
    fn from(value: CommonError) -> Self {
        Self::CmdExecError(value)
    }
}

pub type Result<T> = result::Result<T, FfmpegError>;
