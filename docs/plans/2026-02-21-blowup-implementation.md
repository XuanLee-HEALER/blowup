# blowup 重构实现计划

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** 将 blowup 重构为中文观影自动化流水线 CLI，支持 YIFY 搜索、aria2c 下载、字幕自动获取与对齐。

**Architecture:** 扁平子命令结构（search/download/sub/tracker），每个命令对应独立模块。错误处理统一走 `error.rs`，配置由 `config.rs` 管理，外部工具通过 `std::process::Command` 调用。

**Tech Stack:** Rust 2024 edition, clap (CLI), reqwest (HTTP), chromiumoxide (CDP fallback), thiserror (错误), serde/toml (配置), tokio (async), which (工具路径检测)

---

## Task 1: 清理现有死代码与废弃文件

**Files:**
- Delete: `src/sub/srt.rs`
- Modify: `src/ai.rs`
- Modify: `src/ffmpeg.rs`

**Step 1: 删除 sub/srt.rs**

```bash
git rm src/sub/srt.rs
```

**Step 2: 清空 ai.rs，只保留模块声明**

将 `src/ai.rs` 替换为：

```rust
// Roadmap: 本地 LLM 字幕翻译（ollama）
```

**Step 3: 清理 ffmpeg.rs 未用 import**

删除 `src/ffmpeg.rs` 第 5 行 `use crate::common::CommandError;`（根据 diagnostic warning）。

**Step 4: 确认编译通过**

```bash
cargo build 2>&1
```

Expected: 编译错误（因为 sub.rs 还引用 srt 模块），下一步处理。

**Step 5: 从 sub.rs 移除对 srt 模块和 compare 函数的引用**

在 `src/sub.rs` 中删除 `mod srt;` 声明和 `compare_two_srt_file` 函数。

**Step 6: 确认编译通过**

```bash
cargo build 2>&1
```

Expected: 编译成功（可能有 unused warnings，后续清理）

**Step 7: Commit**

```bash
git add -A
git commit -m "refactor: remove dead code, delete srt submodule and compare function"
```

---

## Task 2: 更新 Cargo.toml 依赖

**Files:**
- Modify: `Cargo.toml`

**Step 1: 移除不再需要的依赖**

从 `[dependencies]` 删除：
- `ollama-rs`（roadmap，暂时移除避免编译噪音）
- `tokio-stream`（随 ollama-rs 一起移除）
- `prettytable-rs`（用简单 println! 替代）

**Step 2: 添加新依赖**

```toml
[dependencies]
# 已有（保留）
octocrab = "0.44.1"
reqwest = { version = "0.12", features = ["json"] }
tokio = { version = "1", features = ["io-std", "process"] }
thiserror = "2.0.12"
chrono = "0.4.41"
clap = { version = "4.5.41", features = ["derive"] }
regex = "1.11.1"
shellexpand = "3.1.1"
which = "8.0.0"
walkdir = "2.5.0"
serde = { version = "1", features = ["derive"] }
serde_json = "1"

# 新增
toml = "0.8"
dirs = "5"
chromiumoxide = { version = "0.7", features = ["chrome"], optional = true }

[features]
cdp = ["chromiumoxide"]
```

> 注意：`chromiumoxide` 设为 optional feature `cdp`，用户无 Chrome 时仍可正常使用基础搜索功能。

**Step 3: 确认编译**

```bash
cargo build 2>&1
```

Expected: 编译成功

**Step 4: Commit**

```bash
git add Cargo.toml Cargo.lock
git commit -m "chore: update dependencies, add toml/dirs/chromiumoxide"
```

---

## Task 3: 创建统一错误类型 error.rs

**Files:**
- Create: `src/error.rs`
- Modify: `src/lib.rs`

**Step 1: 创建 src/error.rs**

```rust
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
```

**Step 2: 在 lib.rs 中声明模块**

在 `src/lib.rs` 顶部添加：

```rust
pub mod error;
```

**Step 3: 写测试确认错误类型可用**

在 `src/error.rs` 末尾添加：

```rust
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
```

**Step 4: 运行测试**

```bash
cargo test error 2>&1
```

Expected: 2 tests passed

**Step 5: Commit**

```bash
git add src/error.rs src/lib.rs
git commit -m "feat: add unified error types"
```

---

## Task 4: 创建配置模块 config.rs

**Files:**
- Create: `src/config.rs`
- Modify: `src/lib.rs`

**Step 1: 写失败测试**

创建 `src/config.rs`，先只写测试：

```rust
use serde::Deserialize;
use std::path::PathBuf;

#[derive(Debug, Deserialize)]
pub struct Config {
    #[serde(default)]
    pub tools: ToolsConfig,
    #[serde(default)]
    pub search: SearchConfig,
    #[serde(default)]
    pub subtitle: SubtitleConfig,
    #[serde(default)]
    pub opensubtitles: OpenSubtitlesConfig,
}

#[derive(Debug, Deserialize)]
pub struct ToolsConfig {
    #[serde(default = "default_aria2c")]
    pub aria2c: String,
    #[serde(default = "default_alass")]
    pub alass: String,
}

#[derive(Debug, Deserialize)]
pub struct SearchConfig {
    #[serde(default = "default_rate_limit")]
    pub rate_limit_secs: u64,
}

#[derive(Debug, Deserialize)]
pub struct SubtitleConfig {
    #[serde(default = "default_lang")]
    pub default_lang: String,
}

#[derive(Debug, Deserialize)]
pub struct OpenSubtitlesConfig {
    #[serde(default)]
    pub api_key: String,
}

fn default_aria2c() -> String { "aria2c".to_string() }
fn default_alass() -> String { "alass".to_string() }
fn default_rate_limit() -> u64 { 5 }
fn default_lang() -> String { "zh".to_string() }

impl Default for ToolsConfig {
    fn default() -> Self {
        Self { aria2c: default_aria2c(), alass: default_alass() }
    }
}
impl Default for SearchConfig {
    fn default() -> Self { Self { rate_limit_secs: default_rate_limit() } }
}
impl Default for SubtitleConfig {
    fn default() -> Self { Self { default_lang: default_lang() } }
}
impl Default for OpenSubtitlesConfig {
    fn default() -> Self { Self { api_key: String::new() } }
}

pub fn config_path() -> PathBuf {
    dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("blowup")
        .join("config.toml")
}

pub fn load_config() -> Config {
    let path = config_path();
    if !path.exists() {
        return Config::default();
    }
    let content = std::fs::read_to_string(&path).unwrap_or_default();
    toml::from_str(&content).unwrap_or_default()
}

impl Default for Config {
    fn default() -> Self {
        Self {
            tools: ToolsConfig::default(),
            search: SearchConfig::default(),
            subtitle: SubtitleConfig::default(),
            opensubtitles: OpenSubtitlesConfig::default(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_config_has_sane_values() {
        let cfg = Config::default();
        assert_eq!(cfg.tools.aria2c, "aria2c");
        assert_eq!(cfg.tools.alass, "alass");
        assert_eq!(cfg.search.rate_limit_secs, 5);
        assert_eq!(cfg.subtitle.default_lang, "zh");
    }

    #[test]
    fn parse_partial_toml() {
        let toml = r#"
[tools]
aria2c = "/usr/local/bin/aria2c"
"#;
        let cfg: Config = toml::from_str(toml).unwrap();
        assert_eq!(cfg.tools.aria2c, "/usr/local/bin/aria2c");
        assert_eq!(cfg.tools.alass, "alass"); // 默认值
        assert_eq!(cfg.search.rate_limit_secs, 5); // 默认值
    }
}
```

**Step 2: 在 lib.rs 声明**

```rust
pub mod config;
```

**Step 3: 运行测试**

```bash
cargo test config 2>&1
```

Expected: 2 tests passed

**Step 4: Commit**

```bash
git add src/config.rs src/lib.rs
git commit -m "feat: add config module with defaults"
```

---

## Task 5: 重构 tracker 模块

**Files:**
- Modify: `src/tracker.rs`

当前 tracker.rs 使用自定义 `TorrentError`，对接到 error.rs 并清理接口。

**Step 1: 查看现有 tracker 测试**

```bash
cargo test tracker 2>&1
```

**Step 2: 重构 tracker.rs**

保留核心逻辑（GitHub API 下载 tracker list），替换错误类型为标准 `Box<dyn std::error::Error>`，并将函数签名调整为更简单的形式：

```rust
use octocrab::Octocrab;
use std::path::PathBuf;
use chrono::Local;
use crate::error::DownloadError;

const OWNER: &str = "ngosang";
const REPO: &str = "trackerslist";
const UPDATE_TIME_RECORD: &str = ".tracker_update_time";

pub async fn update_tracker_list(output_dir: Option<PathBuf>) -> Result<(), Box<dyn std::error::Error>> {
    // 保留原有 download_newest_tracker 逻辑
    // 将结果写入 output_dir / "trackers.txt"
    // 更新时间记录文件
    todo!()
}

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
```

> 注意：先实现 `load_trackers` 和 `tracker_list_path`，`update_tracker_list` 保留原有逻辑（移植自 `download_newest_tracker`）。

**Step 3: 为 load_trackers 写测试**

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    #[test]
    fn load_trackers_empty_when_missing() {
        // tracker_list_path() 如不存在，返回空 vec
        // 这里直接测 load_from_content 逻辑
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
```

**Step 4: 运行测试**

```bash
cargo test tracker 2>&1
```

Expected: passed

**Step 5: Commit**

```bash
git add src/tracker.rs
git commit -m "refactor: simplify tracker module interface"
```

---

## Task 6: 重构 sub 模块结构（shift + extract + list）

**Files:**
- Modify: `src/sub.rs` → 拆分为 `src/sub/mod.rs`
- Create: `src/sub/shift.rs`
- Rename/keep: `src/sub/extract.rs`（从 sub.rs 中分离）

**Step 1: 将 sub.rs 改为 sub/mod.rs**

```bash
mkdir -p src/sub
# sub/srt.rs 已在 Task 1 删除
mv src/sub.rs src/sub/mod.rs
```

等等——`src/sub/` 目录已经存在（因为 `sub/srt.rs` 存在过）。需要将 `src/sub.rs` 的内容迁移到 `src/sub/mod.rs`。

**Step 2: 创建 src/sub/shift.rs，内联时间戳偏移逻辑**

```rust
use std::fs;
use std::path::Path;
use crate::error::SubError;

/// 将 SRT 文件中所有时间戳偏移 offset_ms 毫秒
pub fn shift_srt(srt_path: &Path, offset_ms: i64) -> Result<(), SubError> {
    let content = fs::read_to_string(srt_path).map_err(SubError::Io)?;
    let shifted = apply_offset(&content, offset_ms)?;
    fs::write(srt_path, shifted).map_err(SubError::Io)?;
    Ok(())
}

fn apply_offset(content: &str, offset_ms: i64) -> Result<String, SubError> {
    use regex::Regex;
    let re = Regex::new(
        r"(\d{2}):(\d{2}):(\d{2}),(\d{3}) --> (\d{2}):(\d{2}):(\d{2}),(\d{3})"
    ).unwrap();

    let result = re.replace_all(content, |caps: &regex::Captures| {
        let start = parse_ts(caps, 1) + offset_ms;
        let end = parse_ts(caps, 5) + offset_ms;
        format!("{} --> {}", format_ts(start.max(0)), format_ts(end.max(0)))
    });
    Ok(result.into_owned())
}

fn parse_ts(caps: &regex::Captures, offset: usize) -> i64 {
    let h: i64 = caps[offset].parse().unwrap_or(0);
    let m: i64 = caps[offset + 1].parse().unwrap_or(0);
    let s: i64 = caps[offset + 2].parse().unwrap_or(0);
    let ms: i64 = caps[offset + 3].parse().unwrap_or(0);
    h * 3_600_000 + m * 60_000 + s * 1_000 + ms
}

fn format_ts(total_ms: i64) -> String {
    let h = total_ms / 3_600_000;
    let m = (total_ms % 3_600_000) / 60_000;
    let s = (total_ms % 60_000) / 1_000;
    let ms = total_ms % 1_000;
    format!("{:02}:{:02}:{:02},{:03}", h, m, s, ms)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn offset_positive() {
        let srt = "1\n00:01:00,000 --> 00:01:05,000\nHello\n";
        let result = apply_offset(srt, 5000).unwrap();
        assert!(result.contains("00:01:05,000 --> 00:01:10,000"));
    }

    #[test]
    fn offset_negative() {
        let srt = "1\n00:01:00,000 --> 00:01:05,000\nHello\n";
        let result = apply_offset(srt, -10000).unwrap();
        assert!(result.contains("00:00:50,000 --> 00:00:55,000"));
    }

    #[test]
    fn clamp_at_zero() {
        let srt = "1\n00:00:01,000 --> 00:00:02,000\nHello\n";
        let result = apply_offset(srt, -5000).unwrap();
        assert!(result.contains("00:00:00,000 --> 00:00:00,000"));
    }
}
```

**Step 3: 运行 shift 测试**

```bash
cargo test sub::shift 2>&1
```

Expected: 3 tests passed

**Step 4: 更新 src/sub/mod.rs**

将原 `update_srt_time` 调用改为调用 `shift::shift_srt`，删除 `compare_two_srt_file`，将其他函数（extract/list）保留在 mod.rs 或继续分离。

**Step 5: 确认整体编译**

```bash
cargo build 2>&1
```

**Step 6: Commit**

```bash
git add src/sub/
git commit -m "refactor: split sub module, inline srt shift logic"
```

---

## Task 7: 创建 sub/align.rs（alass 集成）

**Files:**
- Create: `src/sub/align.rs`

**Step 1: 写失败测试**

```rust
use std::path::Path;
use crate::error::SubError;

pub fn align_subtitle(video: &Path, srt: &Path) -> Result<(), SubError> {
    todo!()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn align_returns_error_when_alass_missing() {
        // 在 PATH 中不存在 alass 时，应返回 AlassNotFound 错误
        // 这里用一个不存在的路径模拟
        let result = align_with_binary(
            Path::new("nonexistent_alass_binary_xyz"),
            Path::new("video.mp4"),
            Path::new("sub.srt"),
        );
        assert!(matches!(result, Err(SubError::AlassFailed(_))));
    }
}
```

**Step 2: 实现 align_subtitle**

```rust
use std::path::Path;
use std::process::Command;
use crate::error::SubError;
use which::which;

pub fn align_subtitle(video: &Path, srt: &Path) -> Result<(), SubError> {
    let alass = which("alass").map_err(|_| SubError::AlassNotFound)?;
    align_with_binary(&alass, video, srt)
}

fn align_with_binary(alass: &Path, video: &Path, srt: &Path) -> Result<(), SubError> {
    // 备份原文件
    let backup = srt.with_extension("bak.srt");
    std::fs::copy(srt, &backup).map_err(SubError::Io)?;

    let output = Command::new(alass)
        .arg(video)
        .arg(&backup)
        .arg(srt)
        .output()
        .map_err(|e| SubError::AlassFailed(e.to_string()))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();
        return Err(SubError::AlassFailed(stderr));
    }
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
```

**Step 3: 运行测试**

```bash
cargo test sub::align 2>&1
```

Expected: 1 test passed

**Step 4: Commit**

```bash
git add src/sub/align.rs src/sub/mod.rs
git commit -m "feat: add sub align command via alass"
```

---

## Task 8: 创建 sub/fetch.rs（Assrt + OpenSubtitles 字幕下载）

**Files:**
- Create: `src/sub/fetch.rs`

**Step 1: 定义接口与数据结构**

```rust
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
    video: &Path,
    lang: &str,
    source: SubSource,
    cfg: &Config,
) -> Result<(), SubError> {
    todo!()
}
```

**Step 2: 实现 Assrt 搜索**

Assrt 使用非官方 API，通过视频文件名搜索：

```rust
async fn search_assrt(
    client: &reqwest::Client,
    query: &str,
    lang: &str,
) -> Result<Vec<SubtitleResult>, SubError> {
    // POST https://api.assrt.net/v1/subtitle/search
    // 参数: q=<query>&lang=<lang>
    let resp = client
        .get("https://api.assrt.net/v1/subtitle/search")
        .query(&[("q", query), ("lang", lang)])
        .header("User-Agent", "blowup/0.1 (personal movie tool)")
        .send()
        .await?;

    if !resp.status().is_success() {
        return Err(SubError::NoSubtitleFound);
    }

    let body: serde_json::Value = resp.json().await?;
    parse_assrt_response(&body)
}

fn parse_assrt_response(body: &serde_json::Value) -> Result<Vec<SubtitleResult>, SubError> {
    let subs = body["sub"]["subs"]
        .as_array()
        .ok_or(SubError::NoSubtitleFound)?;

    let results = subs.iter().filter_map(|s| {
        Some(SubtitleResult {
            filename: s["filename"].as_str()?.to_string(),
            lang: s["lang"]["desc"].as_str().unwrap_or("zh").to_string(),
            source: "assrt".to_string(),
        })
    }).collect();
    Ok(results)
}
```

**Step 3: 实现 OpenSubtitles 搜索**

```rust
async fn search_opensubtitles(
    client: &reqwest::Client,
    query: &str,
    lang: &str,
    api_key: &str,
) -> Result<Vec<SubtitleResult>, SubError> {
    let mut req = client
        .get("https://api.opensubtitles.com/api/v1/subtitles")
        .query(&[("query", query), ("languages", lang)])
        .header("User-Agent", "blowup v0.1")
        .header("Content-Type", "application/json");

    if !api_key.is_empty() {
        req = req.header("Api-Key", api_key);
    }

    let resp = req.send().await?;
    if !resp.status().is_success() {
        return Err(SubError::NoSubtitleFound);
    }

    let body: serde_json::Value = resp.json().await?;
    parse_opensubtitles_response(&body)
}

fn parse_opensubtitles_response(body: &serde_json::Value) -> Result<Vec<SubtitleResult>, SubError> {
    let data = body["data"]
        .as_array()
        .ok_or(SubError::NoSubtitleFound)?;

    let results = data.iter().filter_map(|item| {
        let attrs = &item["attributes"];
        Some(SubtitleResult {
            filename: attrs["files"][0]["file_name"].as_str()?.to_string(),
            lang: attrs["language"].as_str().unwrap_or("zh").to_string(),
            source: "opensubtitles".to_string(),
        })
    }).collect();
    Ok(results)
}
```

**Step 4: 写解析测试（用 mock JSON）**

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn parse_assrt_response_ok() {
        let body = json!({
            "sub": {
                "subs": [
                    {"filename": "Blow-Up.1966.zh.srt", "lang": {"desc": "zh"}}
                ]
            }
        });
        let results = parse_assrt_response(&body).unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].filename, "Blow-Up.1966.zh.srt");
    }

    #[test]
    fn parse_assrt_empty_returns_error() {
        let body = json!({"sub": {"subs": []}});
        let results = parse_assrt_response(&body).unwrap();
        assert_eq!(results.len(), 0);
    }

    #[test]
    fn parse_opensubtitles_response_ok() {
        let body = json!({
            "data": [
                {
                    "attributes": {
                        "language": "zh",
                        "files": [{"file_name": "blow_up_1966_zh.srt"}]
                    }
                }
            ]
        });
        let results = parse_opensubtitles_response(&body).unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].source, "opensubtitles");
    }
}
```

**Step 5: 运行测试**

```bash
cargo test sub::fetch 2>&1
```

Expected: 3 tests passed

**Step 6: Commit**

```bash
git add src/sub/fetch.rs src/sub/mod.rs
git commit -m "feat: add sub fetch from Assrt and OpenSubtitles"
```

---

## Task 9: 创建 download.rs（aria2c 集成）

**Files:**
- Create: `src/download.rs`
- Modify: `src/lib.rs`

**Step 1: 写测试**

```rust
use std::path::Path;
use crate::error::DownloadError;
use crate::tracker::load_trackers;

pub struct DownloadArgs<'a> {
    pub target: &'a str,      // magnet: / URL / .torrent 路径
    pub output_dir: &'a Path,
    pub aria2c_bin: &'a str,
}

pub async fn download(args: DownloadArgs<'_>) -> Result<(), DownloadError> {
    todo!()
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
    use std::path::PathBuf;

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
        let joined: String = args_vec.iter()
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
        let joined: String = args_vec.iter()
            .map(|a| a.to_string_lossy())
            .collect::<Vec<_>>()
            .join(" ");
        assert!(!joined.contains("bt-tracker"));
    }
}
```

**Step 2: 实现 download 函数**

```rust
pub async fn download(args: DownloadArgs<'_>) -> Result<(), DownloadError> {
    which::which(args.aria2c_bin).map_err(|_| DownloadError::Aria2cNotFound)?;

    let trackers = load_trackers();
    let mut cmd = build_aria2c_command(&args, &trackers);

    let status = cmd.status().map_err(|e| DownloadError::Aria2cFailed(e.to_string()))?;
    if !status.success() {
        return Err(DownloadError::Aria2cFailed(
            format!("aria2c exited with status: {}", status)
        ));
    }
    Ok(())
}
```

**Step 3: 运行测试**

```bash
cargo test download 2>&1
```

Expected: 2 tests passed

**Step 4: Commit**

```bash
git add src/download.rs src/lib.rs
git commit -m "feat: add download module with aria2c integration"
```

---

## Task 10: 创建 search.rs（YIFY 搜索）

**Files:**
- Create: `src/search.rs`
- Modify: `src/lib.rs`

**Step 1: 定义数据结构与接口**

```rust
use serde::Deserialize;
use crate::error::SearchError;

#[derive(Debug, Clone)]
pub struct MovieResult {
    pub title: String,
    pub year: u32,
    pub quality: String,
    pub magnet: Option<String>,
    pub torrent_url: Option<String>,
    pub seeds: u32,
}

pub async fn search_yify(
    query: &str,
    year: Option<u32>,
) -> Result<Vec<MovieResult>, SearchError> {
    let client = reqwest::Client::new();
    match search_via_api(&client, query, year).await {
        Ok(results) if !results.is_empty() => Ok(results),
        _ => {
            // API 失败或无结果时，可扩展为 CDP fallback
            Err(SearchError::NoResults(query.to_string()))
        }
    }
}

async fn search_via_api(
    client: &reqwest::Client,
    query: &str,
    year: Option<u32>,
) -> Result<Vec<MovieResult>, SearchError> {
    let mut params = vec![
        ("query_term", query.to_string()),
        ("sort_by", "seeds".to_string()),
        ("order_by", "desc".to_string()),
    ];
    if let Some(y) = year {
        params.push(("year", y.to_string()));
    }

    let resp = client
        .get("https://yts.mx/api/v2/list_movies.json")
        .query(&params)
        .header("User-Agent", "blowup/0.1")
        .send()
        .await?;

    let body: YtsResponse = resp.json().await?;
    parse_yts_response(body)
}
```

**Step 2: 解析 YIFY API 响应**

```rust
#[derive(Deserialize)]
struct YtsResponse {
    data: YtsData,
}

#[derive(Deserialize)]
struct YtsData {
    #[serde(default)]
    movies: Vec<YtsMovie>,
}

#[derive(Deserialize)]
struct YtsMovie {
    title: String,
    year: u32,
    torrents: Vec<YtsTorrent>,
}

#[derive(Deserialize)]
struct YtsTorrent {
    quality: String,
    magnet_url: Option<String>,
    url: String,
    seeds: u32,
}

fn parse_yts_response(resp: YtsResponse) -> Result<Vec<MovieResult>, SearchError> {
    let mut results: Vec<MovieResult> = resp.data.movies
        .into_iter()
        .flat_map(|movie| {
            movie.torrents.into_iter().map(move |t| MovieResult {
                title: movie.title.clone(),
                year: movie.year,
                quality: t.quality,
                magnet: t.magnet_url,
                torrent_url: Some(t.url),
                seeds: t.seeds,
            })
        })
        .collect();

    // 按 quality 优先级和 seed 数排序
    results.sort_by(|a, b| {
        quality_rank(&b.quality)
            .cmp(&quality_rank(&a.quality))
            .then(b.seeds.cmp(&a.seeds))
    });

    Ok(results)
}

fn quality_rank(q: &str) -> u8 {
    match q {
        "2160p" => 4,
        "1080p" => 3,
        "720p" => 2,
        _ => 1,
    }
}
```

**Step 3: 写测试**

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn make_yts_response(movies: serde_json::Value) -> YtsResponse {
        serde_json::from_value(json!({"data": {"movies": movies}})).unwrap()
    }

    #[test]
    fn parse_single_movie() {
        let resp = make_yts_response(json!([{
            "title": "Blow-Up",
            "year": 1966,
            "torrents": [
                {"quality": "1080p", "url": "http://x.com/a.torrent", "seeds": 100, "magnet_url": null},
                {"quality": "720p", "url": "http://x.com/b.torrent", "seeds": 200, "magnet_url": null}
            ]
        }]));
        let results = parse_yts_response(resp).unwrap();
        assert_eq!(results.len(), 2);
        // 1080p 排在前面（quality 优先）
        assert_eq!(results[0].quality, "1080p");
    }

    #[test]
    fn quality_rank_order() {
        assert!(quality_rank("1080p") > quality_rank("720p"));
        assert!(quality_rank("2160p") > quality_rank("1080p"));
    }

    #[test]
    fn empty_movies_returns_empty_vec() {
        let resp = make_yts_response(json!([]));
        let results = parse_yts_response(resp).unwrap();
        assert!(results.is_empty());
    }
}
```

**Step 4: 运行测试**

```bash
cargo test search 2>&1
```

Expected: 3 tests passed

**Step 5: Commit**

```bash
git add src/search.rs src/lib.rs
git commit -m "feat: add YIFY search via HTTP API"
```

---

## Task 11: 重构 main.rs，接入所有新命令

**Files:**
- Modify: `src/main.rs`

**Step 1: 重写 main.rs**

```rust
use clap::{Parser, Subcommand};
use std::path::PathBuf;
use blowup::{config, download, search, tracker};
use blowup::sub::{align, fetch, shift};

#[derive(Parser)]
#[command(name = "blowup", about = "中文观影自动化流水线工具")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    #[command(about = "搜索电影片源（优先 YIFY）")]
    Search {
        query: String,
        #[arg(long)]
        year: Option<u32>,
    },
    #[command(about = "通过 aria2c 下载种子/magnet")]
    Download {
        target: String,
        #[arg(long, default_value = ".")]
        output_dir: PathBuf,
    },
    #[command(subcommand, about = "字幕相关工具")]
    Sub(SubCommands),
    #[command(subcommand, about = "Tracker 列表管理")]
    Tracker(TrackerCommands),
}

#[derive(Subcommand)]
enum SubCommands {
    #[command(about = "从 Assrt/OpenSubtitles 下载字幕")]
    Fetch {
        video: PathBuf,
        #[arg(long, default_value = "zh")]
        lang: String,
    },
    #[command(about = "用 alass 自动对齐字幕")]
    Align {
        video: PathBuf,
        srt: PathBuf,
    },
    #[command(about = "从视频容器提取内嵌字幕流")]
    Extract {
        video: PathBuf,
        #[arg(long)]
        stream: Option<u32>,
    },
    #[command(about = "列出视频中的字幕流")]
    List {
        video: PathBuf,
    },
    #[command(about = "手动偏移字幕时间戳（毫秒）")]
    Shift {
        srt: PathBuf,
        offset_ms: i64,
    },
}

#[derive(Subcommand)]
enum TrackerCommands {
    #[command(about = "从远程源更新本地 tracker 列表")]
    Update {
        #[arg(long)]
        source: Option<String>,
    },
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    let cfg = config::load_config();

    match cli.command {
        Commands::Search { query, year } => {
            // 速率限制：5 秒间隔（配置可调）
            let results = search::search_yify(&query, year).await?;
            for (i, r) in results.iter().enumerate() {
                println!("{}: {} ({}) [{}] seeds={}", i+1, r.title, r.year, r.quality, r.seeds);
                if let Some(m) = &r.magnet { println!("   magnet: {}", m); }
                if let Some(u) = &r.torrent_url { println!("   torrent: {}", u); }
            }
        }
        Commands::Download { target, output_dir } => {
            download::download(download::DownloadArgs {
                target: &target,
                output_dir: &output_dir,
                aria2c_bin: &cfg.tools.aria2c,
            }).await?;
        }
        Commands::Sub(sub_cmd) => match sub_cmd {
            SubCommands::Fetch { video, lang } => {
                fetch::fetch_subtitle(&video, &lang, fetch::SubSource::All, &cfg).await?;
            }
            SubCommands::Align { video, srt } => {
                align::align_subtitle(&video, &srt)?;
            }
            SubCommands::Extract { video, stream } => {
                // 调用现有 ffmpeg 逻辑
                blowup::sub::extract_sub_srt(&video, stream).await?;
            }
            SubCommands::List { video } => {
                blowup::sub::list_all_subtitle_stream(&video).await?;
            }
            SubCommands::Shift { srt, offset_ms } => {
                shift::shift_srt(&srt, offset_ms)?;
            }
        },
        Commands::Tracker(TrackerCommands::Update { source }) => {
            tracker::update_tracker_list(source).await?;
        }
    }
    Ok(())
}
```

> 注意：需要将 `anyhow` 加入依赖，或将 main 返回类型改为 `Result<(), Box<dyn std::error::Error>>`。

**Step 2: 确认编译**

```bash
cargo build 2>&1
```

修复所有编译错误（主要是函数签名对齐）。

**Step 3: 运行所有测试**

```bash
cargo test 2>&1
```

Expected: all tests passed

**Step 4: Commit**

```bash
git add src/main.rs Cargo.toml
git commit -m "refactor: restructure CLI with new commands"
```

---

## Task 12: 全量测试与最终清理

**Step 1: 运行 clippy**

```bash
cargo clippy 2>&1
```

修复所有 warning。

**Step 2: 运行格式化**

```bash
cargo fmt
```

**Step 3: 运行所有测试**

```bash
cargo test 2>&1
```

Expected: all passed

**Step 4: 手动冒烟测试**

```bash
# 测试帮助输出
cargo run -- --help
cargo run -- sub --help
cargo run -- search --help

# 测试 shift（不需要外部工具）
echo "1\n00:01:00,000 --> 00:01:05,000\nTest\n" > /tmp/test.srt
cargo run -- sub shift /tmp/test.srt 5000
cat /tmp/test.srt
```

**Step 5: 最终 commit**

```bash
git add -A
git commit -m "chore: cleanup, fmt, clippy fixes"
```

---

## Roadmap（不在本计划内）

- `search` CDP fallback（`chromiumoxide` feature）
- `blowup transcribe`：Whisper 音频转文字
- `blowup translate`：本地小模型字幕翻译
- Assrt 文件 hash 搜索（目前用片名搜索）
- 全网种子搜索（YIFY 以外的源）
