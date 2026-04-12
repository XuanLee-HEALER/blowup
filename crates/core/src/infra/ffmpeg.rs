use std::{ffi::OsStr, path::Path, result};

use thiserror::Error;
use tokio::process::Command;

use crate::infra::common::{CommonError, exec_command, find_command_path};

pub enum FfmpegTool {
    Ffmpeg,
    Ffprobe,
}

impl FfmpegTool {
    fn binary_name(&self) -> &'static str {
        match self {
            FfmpegTool::Ffmpeg => FFMPEG_CLI,
            FfmpegTool::Ffprobe => FFPROBE_CLI,
        }
    }

    pub async fn exec_with_options(
        &self,
        path: Option<impl AsRef<Path>>,
        options: Option<Vec<impl AsRef<OsStr>>>,
    ) -> Result<(String, String)> {
        let exec_path =
            find_command_path(path, self.binary_name()).ok_or(FfmpegError::FfmpegNotFound)?;
        exec_command(exec_path, options).await.map_err(|e| e.into())
    }

    /// Run ffmpeg/ffprobe and return stdout as raw bytes (instead of a
    /// UTF-8 string). Use this when the stdout stream contains binary
    /// data — e.g. `ffmpeg -f f32le -` piping raw PCM samples.
    pub async fn exec_binary_output(&self, args: &[&str]) -> Result<Vec<u8>> {
        let exec_path = find_command_path::<&Path>(None, self.binary_name())
            .ok_or(FfmpegError::FfmpegNotFound)?;

        let output = Command::new(exec_path.as_os_str())
            .args(args)
            .output()
            .await
            .map_err(|e| FfmpegError::BinaryExec(format!("spawn ffmpeg failed: {e}")))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr).to_string();
            return Err(FfmpegError::BinaryExec(format!(
                "ffmpeg exited with {}: {stderr}",
                output.status
            )));
        }
        Ok(output.stdout)
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
    #[error("{0}")]
    BinaryExec(String),
}

impl From<CommonError> for FfmpegError {
    fn from(value: CommonError) -> Self {
        Self::CmdExecError(value)
    }
}

pub type Result<T> = result::Result<T, FfmpegError>;
