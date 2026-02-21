use crate::error::SubError;
use std::path::Path;
use std::process::Command;
use which::which;

/// 使用 alass 自动对齐字幕时间轴
pub fn align_subtitle(video: &Path, srt: &Path) -> Result<(), SubError> {
    let alass = which("alass").map_err(|_| SubError::AlassNotFound)?;
    align_with_binary(&alass, video, srt)
}

fn align_with_binary(alass: &Path, video: &Path, srt: &Path) -> Result<(), SubError> {
    let backup = srt.with_extension("bak.srt");

    // 先尝试运行 alass，只有 binary 可执行时才做备份
    let run_result = Command::new(alass)
        .arg(video)
        .arg(srt)
        .arg(&backup)
        .output()
        .map_err(|e| SubError::AlassFailed(e.to_string()))?;

    if !run_result.status.success() {
        let stderr = String::from_utf8_lossy(&run_result.stderr).to_string();
        return Err(SubError::AlassFailed(stderr));
    }

    // alass 成功后将输出（backup）覆盖回原路径
    std::fs::copy(&backup, srt).map_err(SubError::Io)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn align_returns_error_when_alass_missing() {
        let result = align_with_binary(
            Path::new("nonexistent_alass_binary_xyz"),
            Path::new("video.mp4"),
            Path::new("sub.srt"),
        );
        assert!(matches!(result, Err(SubError::AlassFailed(_))));
    }
}
