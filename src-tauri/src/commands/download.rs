use crate::error::DownloadError;
use crate::tracker::load_trackers;
use std::path::Path;

pub struct DownloadArgs<'a> {
    pub target: &'a str, // magnet: / URL / .torrent 路径
    pub output_dir: &'a Path,
    pub aria2c_bin: &'a str,
}

pub async fn download(args: DownloadArgs<'_>) -> Result<(), DownloadError> {
    which::which(args.aria2c_bin).map_err(|_| DownloadError::Aria2cNotFound)?;

    let trackers = load_trackers();
    let mut cmd = build_aria2c_command(&args, &trackers);

    let status = cmd
        .status()
        .map_err(|e| DownloadError::Aria2cFailed(e.to_string()))?;
    if !status.success() {
        return Err(DownloadError::Aria2cFailed(format!(
            "aria2c exited with status: {}",
            status
        )));
    }
    Ok(())
}

fn build_aria2c_command(args: &DownloadArgs<'_>, trackers: &[String]) -> std::process::Command {
    let mut cmd = std::process::Command::new(args.aria2c_bin);
    cmd.arg("--dir").arg(args.output_dir);

    if !trackers.is_empty() {
        cmd.arg(format!("--bt-tracker={}", trackers.join(",")));
    }

    cmd.arg(args.target);
    cmd
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn aria2c_command_includes_trackers() {
        let args = DownloadArgs {
            target: "magnet:?xt=test",
            output_dir: Path::new("/tmp"),
            aria2c_bin: "aria2c",
        };
        let trackers = vec!["udp://tracker1.com".to_string()];
        let cmd = build_aria2c_command(&args, &trackers);
        let args_vec: Vec<_> = cmd.get_args().collect();
        let joined: String = args_vec
            .iter()
            .map(|a| a.to_string_lossy())
            .collect::<Vec<_>>()
            .join(" ");
        assert!(joined.contains("udp://tracker1.com"));
        assert!(joined.contains("magnet:?xt=test"));
    }

    #[test]
    fn aria2c_command_no_trackers_when_empty() {
        let args = DownloadArgs {
            target: "magnet:?xt=test",
            output_dir: Path::new("/tmp"),
            aria2c_bin: "aria2c",
        };
        let cmd = build_aria2c_command(&args, &[]);
        let args_vec: Vec<_> = cmd.get_args().collect();
        let joined: String = args_vec
            .iter()
            .map(|a| a.to_string_lossy())
            .collect::<Vec<_>>()
            .join(" ");
        assert!(!joined.contains("bt-tracker"));
    }
}
