use std::{
    ffi::OsStr,
    io,
    path::{Path, PathBuf},
    process::ExitStatus,
    result,
};

use thiserror::Error;
use tokio::{fs::File, io::AsyncReadExt, process::Command};
use walkdir::WalkDir;
use which::which;

#[derive(Debug, Error)]
pub enum CommonError {
    #[error("The provided path is incomplete; it's missing the final element")]
    PathMissingFinalError,
    #[error("The provided path contains characters that are not valid UTF-8 encoded")]
    NonUtf8PathError,
    #[error("System IO operation error")]
    IoError,
    #[error("Asynchronous system IO operation error")]
    AsyncIoError(#[from] tokio::io::Error),
    #[error("Retransmit CommandError")]
    CommandError(#[from] CommandError),
}

type Result<T> = result::Result<T, CommonError>;

/// 根据给定的路径 `[ori_path]` 生成一个相同的路径，但是在路径的最后一个元素加上指定的分隔符 `[c]` 和后缀 `[suffix]`
/// # Error
/// * 如果路径没有“最后一个文件“，会返回错误
pub fn same_path_with<P: AsRef<Path>>(ori_path: P, suffix: &str, c: &str) -> Result<PathBuf> {
    let ori_path = ori_path.as_ref();
    let mut new_path = PathBuf::from(ori_path);
    let new_name = ori_path
        .file_stem()
        .ok_or(CommonError::PathMissingFinalError)?
        .to_str()
        .ok_or(CommonError::NonUtf8PathError)?;
    let new_name = format!("{}{}{}", new_name, c, suffix);
    new_path.set_file_name(new_name);
    new_path.set_extension(ori_path.extension().unwrap_or(OsStr::new("")));
    Ok(new_path)
}

/// 查找某个命令的完整路径，如果提供了父目录 `path` ，那么会在父目录下查找，不会进入该目录的其它子目录，否则会在环境变量中查找，使用等效于Unix系统的 `which` 功能
pub fn find_command_path<P: AsRef<Path>>(path: Option<P>, command: &str) -> Option<PathBuf> {
    match path {
        Some(path) => {
            if path.as_ref().is_dir() {
                for entry in WalkDir::new(path).max_depth(1) {
                    let entry = entry.ok()?;
                    if entry.file_name().to_string_lossy() == command {
                        return Some(entry.into_path());
                    }
                }
            }
            None
        }
        None => which(command).ok(),
    }
}

#[derive(Debug, Error)]
pub enum CommandError {
    #[error("Command [{cmd}] failed with status code: {status}. Stderr: {stderr}")]
    CommandFailed {
        cmd: String,
        status: ExitStatus,
        stderr: String,
    },
    #[error(transparent)]
    Io(#[from] io::Error),
}

/// 异步执行指定的命令，传入指定的参数，等待命令执行完成后，返回它的标准输出和标准错误的内容
pub async fn exec_command<S: AsRef<OsStr>>(
    cmd: impl AsRef<Path>,
    options: Option<Vec<S>>,
) -> Result<(String, String)> {
    let mut command = Command::new(cmd.as_ref().as_os_str());
    if let Some(options) = options {
        command.args(options);
    }
    let output = command.output().await.map_err(CommandError::Io)?;

    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();

    if output.status.success() {
        Ok((stdout, stderr))
    } else {
        Err(CommandError::CommandFailed {
            cmd: cmd.as_ref().to_string_lossy().to_string(),
            status: output.status,
            stderr,
        }
        .into())
    }
}

/// 将给定的文件列表读取到字符串中并返回
/// # Error
/// * 如果文件不可读或者路径不存在、类型不是文件会返回系统IO错误
/// # Panic
/// 如果文件数量很多，程序会因为内存占用过高而崩溃
pub async fn read_multiple_file_to_string<P: AsRef<Path> + Send + 'static>(
    files: Vec<P>,
) -> Result<Vec<String>> {
    let file_size = files.len();
    let mut handlers: Vec<_> = Vec::with_capacity(file_size);
    for (idx, file) in files.into_iter().enumerate() {
        handlers.push(tokio::spawn(
            async move { read_file_to_string(idx, file).await },
        ));
    }
    let mut res = vec![Default::default(); file_size];
    for handler in handlers {
        match handler.await {
            Ok(result) => {
                let (idx, text) = result?;
                res[idx] = text
            }
            Err(e) => panic!("Tokio: failed to execute some subtask {}", e),
        }
    }

    Ok(res)
}

async fn read_file_to_string<P: AsRef<Path>>(idx: usize, file: P) -> Result<(usize, String)> {
    const SIZE_LIMIT: u64 = 1024 * 1024;
    let file_path = file.as_ref();
    let meta = std::fs::metadata(file_path).map_err(|_| CommonError::IoError)?;
    if !meta.is_file() || meta.len() > SIZE_LIMIT {
        return Err(CommonError::IoError);
    }
    let mut res = String::new();
    let mut f = File::open(file_path).await?;
    f.read_to_string(&mut res).await?;
    Ok((idx, res))
}

/// Normalize director name(s) for use as a filesystem directory name.
pub fn normalize_director_name(raw: &str) -> String {
    use unicode_normalization::UnicodeNormalization;

    let illegal = ['/', '\\', ':', '*', '?', '"', '<', '>', '|'];

    let mut names: Vec<String> = raw
        .split(',')
        .map(|s| {
            s.nfc()
                .collect::<String>()
                .trim()
                .chars()
                .filter(|c| !illegal.contains(c))
                .collect::<String>()
                .split_whitespace()
                .collect::<Vec<_>>()
                .join(" ")
        })
        .filter(|s| !s.is_empty())
        .collect();

    names.sort();
    names.join(", ")
}

#[cfg(test)]
mod tests {
    use std::{fs, str::FromStr};

    use tempfile::tempdir;

    use super::*;

    #[tokio::test]
    async fn test_read_multiple_file_success() {
        let dir = tempdir().unwrap();
        let file1_path = dir.path().join("file1.txt");
        let file2_path = dir.path().join("file2.txt");

        fs::write(&file1_path, "Hello, world!").unwrap();
        fs::write(&file2_path, "Another file content.").unwrap();

        let files = vec![file1_path.clone(), file2_path.clone()];
        let result = read_multiple_file_to_string(files).await;

        assert!(result.is_ok());
        let contents = result.unwrap();
        assert_eq!(contents.len(), 2);
        assert_eq!(contents[0], "Hello, world!");
        assert_eq!(contents[1], "Another file content.");
    }

    #[tokio::test]
    async fn test_read_multiple_file_with_non_existent_file() {
        let dir = tempdir().unwrap();
        let file1_path = dir.path().join("file1.txt");
        let nonexistent_path = PathBuf::from("/path/to/a/nonexistent/file");

        fs::write(&file1_path, "Existing file.").unwrap();

        let files = vec![file1_path.clone(), nonexistent_path.clone()];
        let result = read_multiple_file_to_string(files).await;

        assert!(result.is_err());
        match result.unwrap_err() {
            CommonError::IoError => assert!(true),
            _ => panic!("Expected IoError"),
        }
    }

    #[tokio::test]
    async fn test_read_multiple_file_with_directory() {
        let dir = tempdir().unwrap();
        let file1_path = dir.path().join("file1.txt");
        let directory_path = dir.path().join("my_dir");
        fs::create_dir(&directory_path).unwrap();

        fs::write(&file1_path, "Existing file.").unwrap();

        let files = vec![file1_path.clone(), directory_path.clone()];
        let result = read_multiple_file_to_string(files).await;

        assert!(result.is_err());
        if let Err(CommonError::IoError) = result {
            // 不同的操作系统返回的错误类型可能不同
            // Unix-like 系统通常返回 IsADirectory，Windows 可能会返回 PermissionDenied
            assert!(true);
        } else {
            panic!("Expected an IoError");
        }
    }

    #[tokio::test]
    async fn test_read_empty_file_list() {
        let files: Vec<PathBuf> = Vec::new();
        let result = read_multiple_file_to_string(files).await;

        assert!(result.is_ok());
        assert!(result.unwrap().is_empty());
    }

    #[test]
    fn test_same_path_with() {
        assert!(same_path_with("/", "f", "_").is_err());
        assert_eq!(
            same_path_with("/root", "f", "_").unwrap(),
            PathBuf::from_str("/root_f").unwrap()
        );
        assert_eq!(
            same_path_with("/root/test", "f", "_").unwrap(),
            PathBuf::from_str("/root/test_f").unwrap()
        );
        assert_eq!(
            same_path_with("/root/test.rs", "f", "_").unwrap(),
            PathBuf::from_str("/root/test_f.rs").unwrap()
        );
    }

    #[test]
    fn test_command_found_in_specified_path() {
        // 创建临时目录
        let temp_dir = tempdir().expect("Failed to create temp dir");
        let bin_dir = temp_dir.path().join("bin");
        fs::create_dir(&bin_dir).expect("Failed to create bin dir");

        // 在临时目录中创建模拟的命令文件
        let command_path = bin_dir.join("my-tool");
        fs::write(&command_path, "I am a tool").expect("Failed to create command file");

        // 调用函数进行测试
        let result = find_command_path(Some(&bin_dir), "my-tool");

        // 验证结果，应该找到文件路径
        assert_eq!(result, Some(command_path));
    }

    #[test]
    fn test_command_not_found_in_specified_path() {
        // 创建临时目录
        let temp_dir = tempdir().expect("Failed to create temp dir");
        let bin_dir = temp_dir.path().join("bin");
        fs::create_dir(&bin_dir).expect("Failed to create bin dir");

        // 在临时目录中创建另一个文件，但不是我们要找的
        let another_file = bin_dir.join("another-tool");
        fs::write(&another_file, "I am another tool").expect("Failed to create file");

        // 调用函数进行测试，查找一个不存在的命令
        let result = find_command_path(Some(&bin_dir), "non-existent-tool");

        // 验证结果，应该返回 None
        assert_eq!(result, None);
    }

    #[test]
    fn test_specified_path_is_invalid() {
        // 创建一个不存在的路径
        let invalid_path = PathBuf::from("this/path/does/not/exist/123");

        // 调用函数进行测试
        let result = find_command_path(Some(&invalid_path), "some-command");

        // 验证结果，应该返回 None，因为路径无效
        assert_eq!(result, None);
    }

    #[test]
    fn test_no_path_specified_and_command_exists() {
        // 此测试依赖于系统环境变量 PATH，因此需要找一个
        // 几乎在所有系统上都存在的命令，比如 "ls"
        let result = find_command_path::<&str>(None, "ls");

        // 验证结果，应该找到路径
        assert!(result.is_some());
    }

    #[test]
    fn test_no_path_specified_and_command_does_not_exist() {
        // 找一个几乎不可能在任何系统上存在的命令
        let non_existent_command = "a-very-unique-command-that-will-not-exist";
        let result = find_command_path::<&str>(None, non_existent_command);

        // 验证结果，应该返回 None
        assert_eq!(result, None);
    }

    #[tokio::test]
    async fn test_exec_command_success() {
        // 跨平台命令，打印 "hello world" 到标准输出
        let cmd = if cfg!(target_os = "windows") {
            "cmd.exe"
        } else {
            "echo"
        };
        let options = if cfg!(target_os = "windows") {
            vec!["/c", "echo hello world"]
        } else {
            vec!["hello world"]
        };

        let result = exec_command(cmd, Some(options)).await;

        // 验证结果，应该成功且标准输出正确
        assert!(result.is_ok());
        let (stdout, stderr) = result.unwrap();
        assert_eq!(stdout.trim(), "hello world");
        assert_eq!(stderr, "");
    }

    #[tokio::test]
    async fn test_exec_command_success_without_options() {
        // 跨平台命令，无参数执行，例如 "ls" 或 "dir"
        let cmd = if cfg!(target_os = "windows") {
            "dir"
        } else {
            "ls"
        };
        let result = exec_command(cmd, None::<Vec<&str>>).await;

        // 验证结果，应该成功，且有输出但无标准错误
        assert!(result.is_ok());
        let (stdout, stderr) = result.unwrap();
        assert!(!stdout.is_empty());
        assert!(stderr.is_empty());
    }

    #[tokio::test]
    async fn test_exec_command_failure() {
        // 跨平台命令，返回非零退出码
        let cmd = if cfg!(target_os = "windows") {
            "cmd.exe"
        } else {
            "sh"
        };
        let options = if cfg!(target_os = "windows") {
            // cmd.exe /c exit 1 是在 Windows 上返回非零退出码的标准方式
            vec!["/c", "exit 1"]
        } else {
            // sh -c "exit 1" 是在 Unix-like 系统上返回非零退出码的标准方式
            vec!["-c", "exit 1"]
        };

        let result = exec_command(cmd, Some(options)).await;

        // 验证结果，应该返回 CommandError::CommandFailed
        assert!(result.is_err());
        if let Err(CommonError::CommandError(CommandError::CommandFailed {
            cmd: _,
            status,
            stderr,
        })) = result
        {
            assert_ne!(status.code(), Some(0));
            assert_eq!(stderr, ""); // 此命令通常不产生标准错误
        } else {
            panic!("Expected CommandFailed error, but got a different error.");
        }
    }

    #[tokio::test]
    async fn test_exec_command_not_found() {
        // 确保这个命令几乎不可能在任何系统上存在
        let cmd = "a-non-existent-program-for-testing";
        let result = exec_command(cmd, None::<Vec<&str>>).await;

        // 验证结果，应该返回一个 io::Error，通常是 "No such file or directory"
        assert!(result.is_err());
        if let Err(CommonError::CommandError(CommandError::Io(e))) = result {
            assert_eq!(e.kind(), std::io::ErrorKind::NotFound);
        } else {
            panic!("Expected an Io error, but got a different error.");
        }
    }

    #[test]
    fn normalize_single_director() {
        assert_eq!(
            super::normalize_director_name("Stanley Kubrick"),
            "Stanley Kubrick"
        );
    }

    #[test]
    fn normalize_multiple_directors_sorted() {
        assert_eq!(
            super::normalize_director_name("Joel Coen, Ethan Coen"),
            "Ethan Coen, Joel Coen"
        );
    }

    #[test]
    fn normalize_strips_illegal_chars() {
        assert_eq!(
            super::normalize_director_name("Jean-Luc Godard: A Life"),
            "Jean-Luc Godard A Life"
        );
    }

    #[test]
    fn normalize_trims_and_collapses_spaces() {
        assert_eq!(
            super::normalize_director_name("  Stanley   Kubrick  "),
            "Stanley Kubrick"
        );
    }

    #[test]
    fn normalize_empty_segments_filtered() {
        assert_eq!(
            super::normalize_director_name("Stanley Kubrick, , "),
            "Stanley Kubrick"
        );
    }
}
