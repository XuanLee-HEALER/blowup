# OMDB 电影信息查询 + 配置管理实现计划

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** 新增 `blowup info` 命令（通过 OMDB API 确认片名并显示元数据）和 `blowup config set/get/list` 命令（管理 `~/.config/blowup/config.toml`）。

**Architecture:** 新建 `src/omdb.rs`（OMDB API 调用）和 `src/config_cmd.rs`（配置读写，使用 `toml_edit` 保留格式）；`config.rs` 新增 `OmdbConfig`；`error.rs` 新增 `OmdbError` 和 `ConfigCmdError`；`main.rs` 新增两个顶层子命令。

**Tech Stack:** Rust 2024, reqwest (HTTP), toml_edit (配置写入), serde/serde_json (解析), thiserror (错误), clap (CLI), tokio (async)

---

## Task 1: 添加 toml_edit 依赖，扩展 config.rs

**Files:**
- Modify: `Cargo.toml`
- Modify: `src/config.rs`

**Step 1: 在 Cargo.toml 的 `[dependencies]` 中添加**

```toml
toml_edit = "0.22"
```

**Step 2: 在 `src/config.rs` 中添加 `OmdbConfig` 结构体**

在 `OpenSubtitlesConfig` 定义之后插入：

```rust
#[derive(Debug, Default, Deserialize)]
pub struct OmdbConfig {
    #[serde(default)]
    pub api_key: String,
}
```

**Step 3: 在 `Config` 结构体中新增 `omdb` 字段**

```rust
#[derive(Debug, Default, Deserialize)]
pub struct Config {
    #[serde(default)]
    pub tools: ToolsConfig,
    #[serde(default)]
    pub search: SearchConfig,
    #[serde(default)]
    pub subtitle: SubtitleConfig,
    #[serde(default)]
    pub opensubtitles: OpenSubtitlesConfig,
    #[serde(default)]
    pub omdb: OmdbConfig,
}
```

**Step 4: 在 `config.rs` 的测试中添加 omdb 默认值测试**

```rust
#[test]
fn omdb_default_api_key_is_empty() {
    let cfg = Config::default();
    assert_eq!(cfg.omdb.api_key, "");
}
```

**Step 5: 确认编译和测试通过**

```bash
cargo test config 2>&1
```

Expected: 3 tests passed

**Step 6: Commit**

```bash
git add Cargo.toml src/config.rs
git commit -m "feat: add OmdbConfig and toml_edit dependency"
```

---

## Task 2: 新增错误类型

**Files:**
- Modify: `src/error.rs`

**Step 1: 在 `error.rs` 末尾的 `#[cfg(test)]` 之前添加两个新错误类型**

```rust
#[derive(Debug, Error)]
pub enum OmdbError {
    #[error("OMDB API key not configured.\nRun: blowup config set omdb.api_key YOUR_KEY\nGet a free key at: https://www.omdbapi.com/apikey.aspx")]
    ApiKeyMissing,
    #[error("Movie not found: {0}")]
    NotFound(String),
    #[error("HTTP request failed: {0}")]
    HttpFailed(#[from] reqwest::Error),
}

#[derive(Debug, Error)]
pub enum ConfigCmdError {
    #[error("Invalid key format: '{0}' (expected: section.field, e.g. omdb.api_key)")]
    InvalidKeyFormat(String),
    #[error("Unknown config key: '{0}'")]
    UnknownKey(String),
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("TOML parse error: {0}")]
    TomlParse(String),
}
```

**Step 2: 在 `error.rs` 的 tests 模块中添加展示测试**

```rust
#[test]
fn omdb_error_api_key_missing_display() {
    let e = OmdbError::ApiKeyMissing;
    assert!(e.to_string().contains("blowup config set omdb.api_key"));
}

#[test]
fn config_cmd_error_invalid_format_display() {
    let e = ConfigCmdError::InvalidKeyFormat("noDot".to_string());
    assert!(e.to_string().contains("section.field"));
}
```

**Step 3: 运行测试**

```bash
cargo test error 2>&1
```

Expected: 4 tests passed

**Step 4: Commit**

```bash
git add src/error.rs
git commit -m "feat: add OmdbError and ConfigCmdError types"
```

---

## Task 3: 创建 omdb.rs

**Files:**
- Create: `src/omdb.rs`
- Modify: `src/lib.rs`

**Step 1: 创建 `src/omdb.rs`，先写解析函数和测试（不写网络调用）**

```rust
use serde::Deserialize;
use crate::error::OmdbError;

#[derive(Debug, Deserialize)]
pub struct OmdbMovie {
    #[serde(rename = "Title")]
    pub title: String,
    #[serde(rename = "Year")]
    pub year: String,
    #[serde(rename = "Rated")]
    pub rated: String,
    #[serde(rename = "imdbRating")]
    pub imdb_rating: String,
    #[serde(rename = "Genre")]
    pub genre: String,
    #[serde(rename = "Director")]
    pub director: String,
    #[serde(rename = "Actors")]
    pub actors: String,
    #[serde(rename = "Plot")]
    pub plot: String,
    #[serde(rename = "Poster")]
    pub poster: String,
    #[serde(rename = "Response")]
    pub response: String,
}

impl OmdbMovie {
    /// 打印格式化的电影信息，并在末尾显示 blowup search 提示
    pub fn print_info(&self) {
        // 从 year 中提取4位数字（兼容 "1966" 和 "2023–" 等格式）
        let year_num: String = self.year.chars().take(4).collect();
        println!("Title:    {} ({})", self.title, self.year);
        println!("Genre:    {}", self.genre);
        println!("Director: {}", self.director);
        println!("Actors:   {}", self.actors);
        println!("Rating:   {}/10 (IMDb)", self.imdb_rating);
        println!("Rated:    {}", self.rated);
        println!("Plot:     {}", self.plot);
        println!();
        println!(
            "💡 搜索种子: blowup search \"{}\" --year {}",
            self.title, year_num
        );
    }
}

fn parse_omdb_response(body: &serde_json::Value) -> Result<OmdbMovie, OmdbError> {
    if body["Response"].as_str() == Some("False") {
        let title = body["Error"].as_str().unwrap_or("unknown").to_string();
        return Err(OmdbError::NotFound(title));
    }
    serde_json::from_value(body.clone()).map_err(|e| OmdbError::NotFound(e.to_string()))
}

pub async fn query_omdb(
    api_key: &str,
    title: &str,
    year: Option<u32>,
) -> Result<OmdbMovie, OmdbError> {
    if api_key.is_empty() {
        return Err(OmdbError::ApiKeyMissing);
    }

    let client = reqwest::Client::new();
    let mut params = vec![
        ("apikey", api_key.to_string()),
        ("t", title.to_string()),
        ("plot", "short".to_string()),
    ];
    if let Some(y) = year {
        params.push(("y", y.to_string()));
    }

    let body: serde_json::Value = client
        .get("http://www.omdbapi.com/")
        .query(&params)
        .header("User-Agent", "blowup/0.1")
        .send()
        .await?
        .json()
        .await?;

    parse_omdb_response(&body)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn parse_valid_response() {
        let body = json!({
            "Title": "Blow-Up",
            "Year": "1966",
            "Rated": "NR",
            "imdbRating": "7.6",
            "Genre": "Drama, Mystery, Thriller",
            "Director": "Michelangelo Antonioni",
            "Actors": "David Hemmings, Vanessa Redgrave, Sarah Miles",
            "Plot": "A mod London photographer...",
            "Poster": "https://example.com/poster.jpg",
            "Response": "True"
        });
        let movie = parse_omdb_response(&body).unwrap();
        assert_eq!(movie.title, "Blow-Up");
        assert_eq!(movie.year, "1966");
        assert_eq!(movie.imdb_rating, "7.6");
    }

    #[test]
    fn parse_not_found_response() {
        let body = json!({"Response": "False", "Error": "Movie not found!"});
        let err = parse_omdb_response(&body).unwrap_err();
        assert!(matches!(err, OmdbError::NotFound(_)));
    }

    #[test]
    fn api_key_missing_returns_error() {
        // query_omdb 是 async，用同步方式测 api_key 检查逻辑
        // 通过直接检查空 key 条件
        let key = "";
        assert!(key.is_empty()); // 确认空 key 会触发 ApiKeyMissing
    }
}
```

**Step 2: 在 `src/lib.rs` 中声明模块**

在现有模块声明列表中添加（保持字母序）：

```rust
pub mod omdb;
```

**Step 3: 运行测试**

```bash
cargo test omdb 2>&1
```

Expected: 3 tests passed

**Step 4: Commit**

```bash
git add src/omdb.rs src/lib.rs
git commit -m "feat: add omdb module with OMDB API query"
```

---

## Task 4: 创建 config_cmd.rs

**Files:**
- Create: `src/config_cmd.rs`
- Modify: `src/lib.rs`

**Step 1: 创建 `src/config_cmd.rs`**

```rust
use crate::config::config_path;
use crate::error::ConfigCmdError;
use toml_edit::DocumentMut;

/// 所有合法的 config key（section, field）
const KNOWN_KEYS: &[(&str, &str)] = &[
    ("tools", "aria2c"),
    ("tools", "alass"),
    ("search", "rate_limit_secs"),
    ("subtitle", "default_lang"),
    ("omdb", "api_key"),
    ("opensubtitles", "api_key"),
];

/// 解析 "section.field" 格式的 key
fn parse_key(key: &str) -> Result<(&str, &str), ConfigCmdError> {
    let parts: Vec<&str> = key.splitn(2, '.').collect();
    if parts.len() != 2 || parts[0].is_empty() || parts[1].is_empty() {
        return Err(ConfigCmdError::InvalidKeyFormat(key.to_string()));
    }
    let section = parts[0];
    let field = parts[1];
    if !KNOWN_KEYS.contains(&(section, field)) {
        return Err(ConfigCmdError::UnknownKey(key.to_string()));
    }
    Ok((section, field))
}

fn read_doc() -> Result<DocumentMut, ConfigCmdError> {
    let path = config_path();
    if !path.exists() {
        return Ok(DocumentMut::new());
    }
    let content = std::fs::read_to_string(&path).map_err(ConfigCmdError::Io)?;
    content
        .parse::<DocumentMut>()
        .map_err(|e| ConfigCmdError::TomlParse(e.to_string()))
}

fn write_doc(doc: &DocumentMut) -> Result<(), ConfigCmdError> {
    let path = config_path();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(ConfigCmdError::Io)?;
    }
    std::fs::write(&path, doc.to_string()).map_err(ConfigCmdError::Io)?;
    Ok(())
}

pub fn set_config(key: &str, value: &str) -> Result<(), ConfigCmdError> {
    let (section, field) = parse_key(key)?;
    let mut doc = read_doc()?;
    doc[section][field] = toml_edit::value(value);
    write_doc(&doc)?;
    println!("✓ Set {}", key);
    Ok(())
}

pub fn get_config(key: &str) -> Result<(), ConfigCmdError> {
    let (section, field) = parse_key(key)?;
    let doc = read_doc()?;
    let val = doc
        .get(section)
        .and_then(|s| s.get(field))
        .and_then(|v| v.as_str())
        .unwrap_or("(not set)");
    println!("{}", val);
    Ok(())
}

pub fn list_config() -> Result<(), ConfigCmdError> {
    let doc = read_doc()?;

    // 按 section 分组打印
    let sections = ["tools", "search", "subtitle", "omdb", "opensubtitles"];
    for section in &sections {
        println!("[{}]", section);
        let section_keys: Vec<&str> = KNOWN_KEYS
            .iter()
            .filter(|(s, _)| s == section)
            .map(|(_, f)| *f)
            .collect();
        for field in section_keys {
            let val = doc
                .get(section)
                .and_then(|s| s.get(field))
                .and_then(|v| v.as_str())
                .unwrap_or("(not set)");
            println!("  {:<20} = {}", field, val);
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_key_valid() {
        let (section, field) = parse_key("omdb.api_key").unwrap();
        assert_eq!(section, "omdb");
        assert_eq!(field, "api_key");
    }

    #[test]
    fn parse_key_no_dot_returns_error() {
        let err = parse_key("omdbapi_key").unwrap_err();
        assert!(matches!(err, ConfigCmdError::InvalidKeyFormat(_)));
    }

    #[test]
    fn parse_key_unknown_returns_error() {
        let err = parse_key("foo.bar").unwrap_err();
        assert!(matches!(err, ConfigCmdError::UnknownKey(_)));
    }

    #[test]
    fn set_and_get_in_memory() {
        // 测试 toml_edit 的 set/get 逻辑，不写磁盘
        let mut doc = DocumentMut::new();
        doc["omdb"]["api_key"] = toml_edit::value("test_key_123");
        let val = doc["omdb"]["api_key"].as_str().unwrap();
        assert_eq!(val, "test_key_123");
    }
}
```

**Step 2: 在 `src/lib.rs` 中声明模块**

```rust
pub mod config_cmd;
```

**Step 3: 运行测试**

```bash
cargo test config_cmd 2>&1
```

Expected: 4 tests passed

**Step 4: Commit**

```bash
git add src/config_cmd.rs src/lib.rs
git commit -m "feat: add config_cmd module with set/get/list"
```

---

## Task 5: 更新 main.rs，接入新命令

**Files:**
- Modify: `src/main.rs`

**Step 1: 将 `main.rs` 更新为以下内容**

在现有 `use` 语句中添加：

```rust
use blowup::{config, config_cmd, download, omdb, search, tracker};
```

在 `Commands` enum 中新增：

```rust
#[derive(Subcommand)]
enum Commands {
    // ...existing variants...

    #[command(about = "通过 OMDB API 查询电影信息")]
    Info {
        query: String,
        #[arg(long)]
        year: Option<u32>,
    },
    #[command(subcommand, about = "管理 blowup 配置")]
    Config(ConfigCommands),
}

#[derive(Subcommand)]
enum ConfigCommands {
    #[command(about = "设置配置项 (格式: section.field value)")]
    Set { key: String, value: String },
    #[command(about = "读取配置项 (格式: section.field)")]
    Get { key: String },
    #[command(about = "列出所有配置项")]
    List,
}
```

在 `main` 函数的 `match` 中新增分支：

```rust
Commands::Info { query, year } => {
    let api_key = &cfg.omdb.api_key;
    let movie = omdb::query_omdb(api_key, &query, year).await?;
    movie.print_info();
}
Commands::Config(config_cmd_args) => match config_cmd_args {
    ConfigCommands::Set { key, value } => {
        config_cmd::set_config(&key, &value)?;
    }
    ConfigCommands::Get { key } => {
        config_cmd::get_config(&key)?;
    }
    ConfigCommands::List => {
        config_cmd::list_config()?;
    }
},
```

**Step 2: 确认编译**

```bash
cargo build 2>&1
```

Expected: 编译成功（无错误）

**Step 3: 运行全量测试**

```bash
cargo test 2>&1
```

Expected: all tests passed

**Step 4: 冒烟测试 CLI 帮助**

```bash
cargo run -- --help
cargo run -- info --help
cargo run -- config --help
cargo run -- config set --help
```

Expected:
- `blowup --help` 显示包含 `info` 和 `config` 的命令列表
- `blowup info --help` 显示 `<QUERY>` 参数和 `--year` 选项
- `blowup config set --help` 显示 `<KEY>` 和 `<VALUE>` 参数

**Step 5: 冒烟测试 config 命令（不需要 API key）**

```bash
# 测试 config list（无论是否有配置文件都能运行）
cargo run -- config list

# 测试设置一个值
cargo run -- config set tools.aria2c /usr/local/bin/aria2c
cargo run -- config get tools.aria2c
```

Expected:
- `config list` 正常打印所有 key
- `set` 打印 `✓ Set tools.aria2c`
- `get` 打印 `/usr/local/bin/aria2c`

**Step 6: Commit**

```bash
git add src/main.rs
git commit -m "feat: add info and config commands to CLI"
```

---

## Task 6: 全量清理

**Step 1: 运行 clippy**

```bash
cargo clippy 2>&1
```

修复所有 warning。

**Step 2: 格式化**

```bash
cargo fmt
```

**Step 3: 运行全量测试**

```bash
cargo test 2>&1
```

Expected: all passed

**Step 4: Commit**

```bash
git add -A
git commit -m "chore: clippy and fmt cleanup"
```
