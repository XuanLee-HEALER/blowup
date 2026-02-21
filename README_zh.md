# blowup

> [Click here for English version](./README.md)

![Version](https://img.shields.io/badge/Version-1.0.0-blue?style=for-the-badge) ![License](https://img.shields.io/badge/License-MIT-darkgreen?style=for-the-badge) ![Crates.io](https://img.shields.io/crates/v/blowup?style=for-the-badge)

> **《放大》[米开朗基罗·安东尼奥尼，1966]**：一位时装摄影师在公园里跟拍两名恋人时，无意间将一桩谋杀摄入镜头。
>
> 我认为迄今为止最好的电影。

---

**blowup** 是一个命令行工具，用于自动化观看外国电影的技术环节——搜索片源、下载、获取并对齐字幕——让你把更多精力放在电影本身上。

## 功能列表

| 命令 | 说明 |
|------|------|
| `blowup search` | 通过标题和年份在 YIFY 搜索电影种子 |
| `blowup download` | 通过 aria2c 下载种子文件或 magnet 链接 |
| `blowup info` | 通过 TMDB 查询电影详情（演员、导演、简介） |
| `blowup sub fetch` | 从 OpenSubtitles 下载字幕 |
| `blowup sub align` | 使用 alass 自动同步字幕时间轴到视频 |
| `blowup sub extract` | 从视频容器中提取内嵌字幕流 |
| `blowup sub list` | 列出视频文件中的所有字幕流 |
| `blowup sub shift` | 将字幕所有时间戳偏移 N 毫秒 |
| `blowup tracker update` | 从远程源更新本地 tracker 列表 |
| `blowup config` | 读写工具配置 |

## 安装

```bash
cargo install blowup
```

**运行时依赖**（需单独安装）：

| 工具 | 用途 | 安装方式 |
|------|------|----------|
| `aria2c` | `download` 命令 | `brew install aria2` / `apt install aria2` |
| `alass` / `alass-cli` | `sub align` 命令 | `brew install alass` / [GitHub releases](https://github.com/kaegi/alass/releases) |
| `ffmpeg` + `ffprobe` | `sub extract`、`sub list` 命令 | `brew install ffmpeg` |

## 快速开始

```bash
# 1. 配置 API Key 和工具路径
blowup config set tmdb.api_key 你的TMDB密钥   # 在 themoviedb.org 免费申请
blowup config set tools.aria2c /usr/local/bin/aria2c
blowup config set tools.alass /usr/local/bin/alass-cli

# 2. 查询电影信息
blowup info "放大" --year 1966

# 3. 搜索片源
blowup search "Blow-Up" --year 1966

# 4. 下载种子
blowup download "https://yts.bz/torrent/download/HASH" --output-dir ~/Movies

# 5. 下载中文字幕
blowup sub fetch ~/Movies/Blow-Up.mp4 --lang zh

# 6. 自动对齐字幕时间轴
blowup sub align ~/Movies/Blow-Up.mp4 ~/Movies/Blow-Up.zh.srt
```

## 命令详解

### `search`

在 YIFY 搜索电影种子。结果按画质（2160p → 1080p → 720p）和做种数量排序。

```
blowup search <查询词> [--year 年份]
```

```
blowup search "寄生虫" --year 2019
1: Parasite (2019) [1080p] seeds=312
   torrent: https://yts.bz/torrent/download/...
```

### `download`

使用 aria2c 下载种子文件 URL 或 magnet 链接。

```
blowup download <目标> [--output-dir 目录]
```

通过 `blowup config set tools.aria2c <路径>` 配置 aria2c 可执行文件路径。

### `info`

从 TMDB 查询电影元数据：简介、类型、导演、主要演员。

```
blowup info <查询词> [--year 年份]
```

需要免费的 TMDB API Key，通过 `blowup config set tmdb.api_key <key>` 设置。

### `sub fetch`

在 OpenSubtitles 搜索并将最佳匹配字幕下载到视频同级目录。

```
blowup sub fetch <视频文件> [--lang 语言代码]
```

默认语言为 `zh`（中文）。字幕保存为 `<视频名>.<lang>.srt`。

支持的语言代码：`zh`（中文）、`en`（英语）、`ja`（日语）、`ko`（韩语）、`fr`（法语）、`de`（德语）、`es`（西班牙语）。

### `sub align`

使用 [alass](https://github.com/kaegi/alass) 自动将字幕时间轴同步到视频。原始字幕保留为 `<名称>.bak.srt`。

```
blowup sub align <视频文件> <字幕文件>
```

通过 `blowup config set tools.alass <路径>` 配置 alass 可执行文件路径。

### `sub extract`

从视频容器中提取内嵌字幕流为 SRT 文件（需要 ffmpeg）。

```
blowup sub extract <视频文件> [--stream 流编号]
```

### `sub list`

列出视频文件中的所有字幕流（需要 ffprobe）。

```
blowup sub list <视频文件>
```

### `sub shift`

将 SRT 文件中所有时间戳偏移固定毫秒数。正值延后，负值提前。

```
blowup sub shift <字幕文件> <偏移毫秒>
```

### `tracker update`

从远程源获取最新的 BitTorrent tracker 列表并保存到本地。

```
blowup tracker update [--source URL]
```

### `config`

读写保存在 `~/.config/blowup/config.toml` 的工具配置。

```
blowup config set <KEY> <VALUE>
blowup config get <KEY>
blowup config list
```

**可用配置项：**

| Key | 类型 | 说明 |
|-----|------|------|
| `tools.aria2c` | 路径 | aria2c 可执行文件路径 |
| `tools.alass` | 路径 | alass 或 alass-cli 可执行文件路径 |
| `tmdb.api_key` | 字符串 | TMDB API Key（在 [themoviedb.org](https://www.themoviedb.org/settings/api) 免费申请） |
| `opensubtitles.api_key` | 字符串 | OpenSubtitles API Key（可选） |
| `subtitle.default_lang` | 字符串 | 默认字幕语言代码（如 `zh`） |
| `search.rate_limit_secs` | 整数 | 搜索请求之间的等待秒数 |

## 许可协议

MIT — 详见 [LICENSE](./LICENSE.txt)。
