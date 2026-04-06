use chrono::{DateTime, Local};
use std::path::PathBuf;
use tokio::{
    fs::File,
    io::{AsyncReadExt, AsyncWriteExt},
};

const TIME_FMT: &str = "%Y-%m-%d %H:%M:%S %z";
const OWNER: &str = "ngosang";
const REPO: &str = "trackerslist";

pub fn tracker_list_path() -> PathBuf {
    dirs::data_local_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("blowup")
        .join("trackers.txt")
}

pub fn load_trackers() -> Vec<String> {
    let path = tracker_list_path();
    if !path.exists() {
        return vec![];
    }
    std::fs::read_to_string(path)
        .unwrap_or_default()
        .lines()
        .filter(|l| !l.trim().is_empty())
        .map(String::from)
        .collect()
}

/// 从 GitHub 下载最新 tracker 列表并保存到本地。
/// source 为可选的远程 URL，默认使用 ngosang/trackerslist。
pub async fn update_tracker_list(_source: Option<String>) -> anyhow::Result<()> {
    let req_path = format!("/repos/{}/{}/contents/trackers_all.txt", OWNER, REPO);
    let github = octocrab::instance();

    let content = github._get(&req_path).await?;
    let last_modified = content
        .headers()
        .get("last-modified")
        .ok_or_else(|| anyhow::anyhow!("missing last-modified header"))?
        .to_str()
        .map_err(|_| anyhow::anyhow!("invalid last-modified header"))?;
    let last_modified = DateTime::parse_from_rfc2822(last_modified)
        .map_err(|e| anyhow::anyhow!("failed to parse last-modified: {}", e))?
        .with_timezone(&Local);

    let update_record = tracker_list_path().with_file_name("tracker_update_time");

    if matches!(read_update_time(&update_record).await, Ok(t) if last_modified <= t) {
        return Ok(());
    }

    // 下载 tracker 内容
    let file_content: octocrab::models::repos::Content = github.get(&req_path, None::<&()>).await?;
    let text = file_content
        .decoded_content()
        .ok_or_else(|| anyhow::anyhow!("failed to decode tracker content"))?;

    // 确保目录存在
    let tracker_path = tracker_list_path();
    if let Some(parent) = tracker_path.parent() {
        tokio::fs::create_dir_all(parent).await?;
    }

    File::create(&tracker_path)
        .await?
        .write_all(text.as_bytes())
        .await?;

    // 更新时间记录
    tokio::fs::write(
        &update_record,
        format!("{}\n", last_modified.format(TIME_FMT)),
    )
    .await?;

    Ok(())
}

async fn read_update_time(path: &std::path::Path) -> anyhow::Result<DateTime<Local>> {
    if !path.is_file() {
        anyhow::bail!("no update record");
    }
    let mut buf = String::new();
    File::open(path).await?.read_to_string(&mut buf).await?;
    let last_record = buf
        .lines()
        .rev()
        .find(|l| !l.trim().is_empty())
        .ok_or_else(|| anyhow::anyhow!("empty update record"))?;
    Ok(DateTime::parse_from_str(last_record, TIME_FMT)?.with_timezone(&Local))
}

#[tauri::command]
pub async fn update_trackers(source: Option<String>) -> std::result::Result<(), String> {
    update_tracker_list(source).await.map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    #[test]
    fn load_trackers_parses_content() {
        let content = "udp://tracker1.com\nudp://tracker2.com\n\n";
        let trackers: Vec<String> = content
            .lines()
            .filter(|l| !l.trim().is_empty())
            .map(String::from)
            .collect();
        assert_eq!(trackers.len(), 2);
        assert_eq!(trackers[0], "udp://tracker1.com");
    }
}
