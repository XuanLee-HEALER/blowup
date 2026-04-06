use crate::error::DownloadError;
use crate::config::load_config;
use super::tracker::load_trackers;
use std::path::Path;
use serde::Serialize;
use sqlx::SqlitePool;

#[derive(Serialize, sqlx::FromRow)]
pub struct DownloadRecord {
    pub id: i64,
    pub film_id: Option<i64>,
    pub title: String,
    pub quality: Option<String>,
    pub target: String,
    pub output_dir: String,
    pub status: String,
    pub pid: Option<i64>,
    pub file_path: Option<String>,
    pub error_message: Option<String>,
    pub started_at: String,
    pub completed_at: Option<String>,
}

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

#[tauri::command]
pub async fn download_target(
    target: String,
    output_dir: String,
    aria2c_bin: String,
) -> std::result::Result<(), String> {
    let path = std::path::PathBuf::from(&output_dir);
    download(DownloadArgs {
        target: &target,
        output_dir: &path,
        aria2c_bin: &aria2c_bin,
    })
    .await
    .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn start_download(
    title: String,
    target: String,
    quality: Option<String>,
    film_id: Option<i64>,
    pool: tauri::State<'_, SqlitePool>,
) -> Result<i64, String> {
    let config = load_config();
    let aria2c_bin = config.tools.aria2c.clone();
    let output_dir = config.library.root_dir.clone();

    which::which(&aria2c_bin)
        .map_err(|_| "aria2c 未找到，请在设置中配置 aria2c 路径".to_string())?;

    std::fs::create_dir_all(&output_dir).map_err(|e| e.to_string())?;

    let trackers = load_trackers();

    let mut cmd = tokio::process::Command::new(&aria2c_bin);
    cmd.arg("--dir").arg(&output_dir);
    cmd.arg("--seed-time=0");
    if !trackers.is_empty() {
        cmd.arg(format!("--bt-tracker={}", trackers.join(",")));
    }
    cmd.arg(&target);

    let mut child = cmd.spawn().map_err(|e| e.to_string())?;
    let pid = child.id().map(|p| p as i64);

    let result = sqlx::query(
        "INSERT INTO downloads (film_id, title, quality, target, output_dir, status, pid)
         VALUES (?, ?, ?, ?, ?, 'downloading', ?)",
    )
    .bind(film_id)
    .bind(&title)
    .bind(&quality)
    .bind(&target)
    .bind(&output_dir)
    .bind(pid)
    .execute(pool.inner())
    .await
    .map_err(|e| e.to_string())?;

    let download_id = result.last_insert_rowid();

    let pool_clone = pool.inner().clone();
    tokio::spawn(async move {
        let wait_result = child.wait().await;

        let current_status: Option<String> = sqlx::query_scalar(
            "SELECT status FROM downloads WHERE id = ?",
        )
        .bind(download_id)
        .fetch_optional(&pool_clone)
        .await
        .ok()
        .flatten();

        if current_status.as_deref() == Some("cancelled") {
            return;
        }

        match wait_result {
            Ok(status) if status.success() => {
                let _ = sqlx::query(
                    "UPDATE downloads SET status = 'completed', completed_at = datetime('now'), pid = NULL WHERE id = ?",
                )
                .bind(download_id)
                .execute(&pool_clone)
                .await;
            }
            Ok(status) => {
                let msg = format!("aria2c exited with code {}", status.code().unwrap_or(-1));
                let _ = sqlx::query(
                    "UPDATE downloads SET status = 'failed', completed_at = datetime('now'), pid = NULL, error_message = ? WHERE id = ?",
                )
                .bind(&msg)
                .bind(download_id)
                .execute(&pool_clone)
                .await;
            }
            Err(e) => {
                let _ = sqlx::query(
                    "UPDATE downloads SET status = 'failed', completed_at = datetime('now'), pid = NULL, error_message = ? WHERE id = ?",
                )
                .bind(e.to_string())
                .bind(download_id)
                .execute(&pool_clone)
                .await;
            }
        }
    });

    Ok(download_id)
}

#[tauri::command]
pub async fn list_downloads(
    pool: tauri::State<'_, SqlitePool>,
) -> Result<Vec<DownloadRecord>, String> {
    sqlx::query_as::<_, DownloadRecord>(
        "SELECT id, film_id, title, quality, target, output_dir, status, pid,
                file_path, error_message, started_at, completed_at
         FROM downloads ORDER BY started_at DESC",
    )
    .fetch_all(pool.inner())
    .await
    .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn cancel_download(
    id: i64,
    pool: tauri::State<'_, SqlitePool>,
) -> Result<(), String> {
    let pid: Option<i64> = sqlx::query_scalar(
        "SELECT pid FROM downloads WHERE id = ? AND status = 'downloading'",
    )
    .bind(id)
    .fetch_optional(pool.inner())
    .await
    .map_err(|e| e.to_string())?
    .flatten();

    if let Some(pid) = pid {
        kill_process(pid as u32);
    }

    sqlx::query(
        "UPDATE downloads SET status = 'cancelled', completed_at = datetime('now'), pid = NULL WHERE id = ?",
    )
    .bind(id)
    .execute(pool.inner())
    .await
    .map_err(|e| e.to_string())?;

    Ok(())
}

#[tauri::command]
pub async fn delete_download_record(
    id: i64,
    pool: tauri::State<'_, SqlitePool>,
) -> Result<(), String> {
    sqlx::query("DELETE FROM downloads WHERE id = ?")
        .bind(id)
        .execute(pool.inner())
        .await
        .map_err(|e| e.to_string())?;
    Ok(())
}

fn kill_process(pid: u32) {
    #[cfg(target_family = "windows")]
    {
        let _ = std::process::Command::new("taskkill")
            .args(["/PID", &pid.to_string(), "/F"])
            .status();
    }
    #[cfg(not(target_family = "windows"))]
    {
        let _ = std::process::Command::new("kill")
            .args(["-TERM", &pid.to_string()])
            .status();
    }
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

    #[tokio::test]
    async fn test_download_record_crud() {
        let pool = sqlx::SqlitePool::connect(":memory:").await.unwrap();
        sqlx::migrate!("./migrations").run(&pool).await.unwrap();

        sqlx::query(
            "INSERT INTO downloads (title, target, output_dir, status)
             VALUES (?, ?, ?, 'downloading')",
        )
        .bind("Test Film")
        .bind("magnet:?xt=test")
        .bind("/tmp/downloads")
        .execute(&pool)
        .await
        .unwrap();

        let records: Vec<super::DownloadRecord> = sqlx::query_as(
            "SELECT id, film_id, title, quality, target, output_dir, status, pid,
                    file_path, error_message, started_at, completed_at
             FROM downloads ORDER BY started_at DESC",
        )
        .fetch_all(&pool)
        .await
        .unwrap();

        assert_eq!(records.len(), 1);
        assert_eq!(records[0].title, "Test Film");
        assert_eq!(records[0].status, "downloading");
    }

    #[tokio::test]
    async fn test_cancel_sets_status() {
        let pool = sqlx::SqlitePool::connect(":memory:").await.unwrap();
        sqlx::migrate!("./migrations").run(&pool).await.unwrap();

        sqlx::query(
            "INSERT INTO downloads (title, target, output_dir, status, pid)
             VALUES (?, ?, ?, 'downloading', ?)",
        )
        .bind("Test Film")
        .bind("magnet:?xt=test")
        .bind("/tmp")
        .bind(99999_i64)
        .execute(&pool)
        .await
        .unwrap();

        sqlx::query(
            "UPDATE downloads SET status = 'cancelled', completed_at = datetime('now'), pid = NULL WHERE id = 1",
        )
        .execute(&pool)
        .await
        .unwrap();

        let status: String = sqlx::query_scalar("SELECT status FROM downloads WHERE id = 1")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(status, "cancelled");
    }

    #[tokio::test]
    async fn test_delete_download_record() {
        let pool = sqlx::SqlitePool::connect(":memory:").await.unwrap();
        sqlx::migrate!("./migrations").run(&pool).await.unwrap();

        sqlx::query(
            "INSERT INTO downloads (title, target, output_dir, status)
             VALUES (?, ?, ?, 'completed')",
        )
        .bind("Done Film")
        .bind("magnet:?xt=done")
        .bind("/tmp")
        .execute(&pool)
        .await
        .unwrap();

        sqlx::query("DELETE FROM downloads WHERE id = 1")
            .execute(&pool)
            .await
            .unwrap();

        let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM downloads")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(count, 0);
    }
}
