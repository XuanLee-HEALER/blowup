# OMDB 电影信息查询 + 配置管理 设计文档

**日期：** 2026-02-21
**状态：** 已确认，待实现

---

## 背景

用户在搜索种子时，片名拼写不规范会导致 YIFY 搜索结果不准确。通过引入 OMDB API 作为独立的"电影信息确认"步骤，可以：

1. 标准化片名（如 "blow up 1966" → "Blow-Up"）
2. 自动补全年份
3. 展示元数据（评分、导演、主演、简介等）
4. 引导用户用准确信息去搜索种子

此功能设计为独立命令，不改变现有 `search` 命令的行为。

---

## 新增命令

### `blowup info <query>`

查询电影信息，需要 OMDB API key 已配置。

```
$ blowup info "blow up"

Title:    Blow-Up (1966)
Genre:    Drama, Mystery, Thriller
Director: Michelangelo Antonioni
Actors:   David Hemmings, Vanessa Redgrave, Sarah Miles
Rating:   7.6/10 (IMDb)
Rated:    NR
Plot:     A mod London photographer discovers he may have inadvertently
          photographed a murder.

💡 搜索种子: blowup search "Blow-Up" --year 1966
```

若未配置 API key：

```
Error: OMDB API key not configured.
Run: blowup config set omdb.api_key YOUR_KEY
Get a free key at: https://www.omdbapi.com/apikey.aspx
```

### `blowup config set <key> <value>`

将配置项写入 `~/.config/blowup/config.toml`，使用 `toml_edit` 保留文件格式和注释。

```
$ blowup config set omdb.api_key YOUR_KEY
✓ Set omdb.api_key
```

### `blowup config get <key>`

读取并打印指定配置项的值。

```
$ blowup config get omdb.api_key
YOUR_KEY

$ blowup config get tools.aria2c
aria2c
```

### `blowup config list`

打印所有已知配置项及当前值（未设置显示 `(not set)`）。

```
$ blowup config list
[tools]
  aria2c             = aria2c
  alass              = alass
[search]
  rate_limit_secs    = 5
[subtitle]
  default_lang       = zh
[omdb]
  api_key            = sk-xxxxx
[opensubtitles]
  api_key            = (not set)
```

---

## 架构

### 新增文件

| 文件 | 职责 |
|------|------|
| `src/omdb.rs` | OMDB API 调用、响应解析、`OmdbMovie` 结构体 |
| `src/config_cmd.rs` | `set/get/list` 命令逻辑，读写 toml |

### 修改文件

| 文件 | 改动 |
|------|------|
| `src/config.rs` | 新增 `OmdbConfig { api_key: String }`，`Config` 加 `omdb` 字段 |
| `src/error.rs` | 新增 `OmdbError`、`ConfigCmdError` |
| `src/lib.rs` | 声明 `omdb`、`config_cmd` 模块 |
| `src/main.rs` | 新增 `Info` 和 `Config` 顶层子命令 |
| `Cargo.toml` | 新增 `toml_edit` 依赖 |

---

## 数据结构

### `OmdbMovie`

```rust
pub struct OmdbMovie {
    pub title: String,        // 标准化片名
    pub year: String,         // 年份（字符串，兼容剧集 "2023–" 格式）
    pub rated: String,        // 分级，如 "R"、"NR"
    pub imdb_rating: String,  // 如 "7.6"
    pub genre: String,        // 如 "Drama, Thriller"
    pub director: String,
    pub actors: String,       // 前 2-3 位主演，逗号分隔
    pub plot: String,         // 简短剧情
    pub poster: String,       // 海报 URL（仅展示，不下载）
}
```

### `OmdbConfig`（新增到 config.rs）

```rust
#[derive(Debug, Default, Deserialize)]
pub struct OmdbConfig {
    #[serde(default)]
    pub api_key: String,
}
```

---

## 错误类型

### `OmdbError`

```rust
pub enum OmdbError {
    ApiKeyMissing,           // 未配置 API key，附带配置提示
    NotFound(String),        // OMDB 找不到该片名
    HttpFailed(reqwest::Error),
}
```

### `ConfigCmdError`

```rust
pub enum ConfigCmdError {
    InvalidKeyFormat(String), // 格式不是 section.field
    UnknownKey(String),       // 不在白名单中的 key
    Io(std::io::Error),
    TomlParse(String),
}
```

---

## OMDB API

- **Endpoint：** `http://www.omdbapi.com/`
- **参数：** `apikey`、`t`（片名）、`plot=short`
- **可选参数：** `y`（年份，用于消歧义）
- **认证：** 免费 key，注册后获取（1000次/天）

key 格式 `section.field` 白名单（硬编码在 `config_cmd.rs`）：

```
tools.aria2c
tools.alass
search.rate_limit_secs
subtitle.default_lang
omdb.api_key
opensubtitles.api_key
```

---

## 配置文件写入

使用 `toml_edit` crate 保留已有注释和格式。流程：

1. 若文件不存在，创建目录和空文件
2. 用 `toml_edit::DocumentMut` 解析现有内容
3. 修改对应 `section.field`
4. 序列化写回文件

---

## Roadmap（不在本次范围内）

- `blowup config unset <key>` — 删除某个配置项
- OMDB 搜索结果缓存（避免重复请求）
- `blowup info` 支持 IMDb ID 直接查询（`--imdb tt0060176`）
