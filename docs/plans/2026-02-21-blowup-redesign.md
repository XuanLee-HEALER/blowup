# blowup 项目重新设计

**日期**: 2026-02-21  
**状态**: 已批准

## 目标

将 blowup 从一个零散的字幕/tracker 工具，重新定位为**中文观影自动化流水线 CLI**，专注于老电影（YIFY 压制源）的全流程体验：从搜索片源到字幕就位。

## CLI 命令结构

```
blowup
├── search <movie-name> [--year <year>] [--source yify|all]
│       搜索片源，输出 magnet 链接或 .torrent 路径供用户选择
│
├── download <magnet|url|.torrent>
│       通过 aria2c 下载，自动附加本地 tracker list
│
├── sub
│   ├── fetch <video-file> [--lang zh|en] [--source assrt|opensubtitles|all]
│   │       按文件 hash 或片名从 Assrt/OpenSubtitles 下载字幕
│   ├── align <video-file> <srt-file>
│   │       调用 alass 自动对齐字幕时间轴
│   ├── extract <video-file> [--stream <index>]
│   │       用 ffmpeg 从视频容器提取内嵌字幕流
│   ├── list <video-file>
│   │       用 ffprobe 列出视频中所有字幕流
│   └── shift <srt-file> <offset>
│           手动偏移字幕时间戳
│
└── tracker
    └── update [--source <url>]
            从远程源更新本地 tracker 列表
```

### Roadmap（本次不实现）

```
blowup transcribe <video-file>   # Whisper 音频转文字
blowup translate <srt-file>      # 本地小模型翻译字幕
```

## 模块架构

```
src/
├── main.rs          # CLI 入口，Commands 枚举
├── lib.rs           # 模块声明
├── error.rs         # 用 thiserror 定义各模块错误类型（新增）
├── config.rs        # 配置文件读写（新增）
├── search.rs        # 片源搜索（新增）
├── download.rs      # aria2c 集成（新增）
├── tracker.rs       # tracker 列表管理（保留，轻度重构）
├── ffmpeg.rs        # ffmpeg/ffprobe 封装（保留，清理 warning）
├── ai.rs            # 清空实现，保留模块声明（roadmap 翻译用）
└── sub/
    ├── mod.rs       # sub 子命令路由
    ├── fetch.rs     # Assrt + OpenSubtitles 字幕下载（新增）
    ├── align.rs     # alass 集成（新增）
    └── shift.rs     # 时间戳偏移，内联简单解析逻辑
```

**删除**：`srt.rs`（原有详细 SRT 操作逻辑，功能缩减为仅时间戳偏移，内联到 sub/shift.rs）

## 关键实现细节

### search.rs — 两级搜索策略

1. **优先**：直接 HTTP 请求（`reqwest`）调用 YIFY 公开 JSON API（`yts.mx/api/v2/list_movies.json`）
2. **Fallback**：HTTP 被拦截时，用 `chromiumoxide` 启动 headless Chrome 做 CDP 抓取

速率限制：默认搜索间隔 5 秒（可通过 config 调整），尊重网站限制，不并发请求。  
结果按 quality（1080p > 720p）和 seed 数排序展示，供用户选择。

### download.rs — aria2c 集成

- 通过 `std::process::Command` 调用 `aria2c`
- 自动从本地 tracker 缓存文件读取并附加 tracker list
- 支持 magnet 链接、`.torrent` 文件、直链 URL

### sub/fetch.rs — 字幕下载

- 搜索顺序：Assrt → OpenSubtitles（`--source` 参数可指定）
- Assrt：用文件 hash 搜索，命中率高；OpenSubtitles：用 IMDB ID 或片名
- 下载结果保存为与视频同名：`movie.mp4` → `movie.zh.srt`

### sub/align.rs — 自动字幕对齐

- 调用 `alass <video-file> <srt-file> <output-srt>`
- 对齐前自动备份原文件为 `*.bak.srt`

### config.rs — 最小化配置

路径：`~/.config/blowup/config.toml`

```toml
[tools]
aria2c = "aria2c"
alass = "alass"

[search]
rate_limit_secs = 5

[subtitle]
default_lang = "zh"

[opensubtitles]
api_key = ""   # 可选
```

### error.rs — 错误处理

- 每个模块用 `thiserror` 定义自己的错误类型
- CLI 层统一 match，输出人类可读错误信息 + 非零退出码

## 外部依赖

| 工具 | 用途 | 必须 |
|------|------|------|
| `aria2c` | 种子/magnet 下载 | 是 |
| `ffmpeg` | 字幕流提取 | 是 |
| `ffprobe` | 字幕流列举 | 是 |
| `alass` | 字幕自动对齐 | 是 |
| Chrome/Chromium | CDP fallback 搜索 | 推荐 |
| `whisper` CLI | 音频转文字 | Roadmap |

## 现有代码处理

| 文件 | 处置 |
|------|------|
| `tracker.rs` | 保留，轻度重构接口 |
| `ffmpeg.rs` | 保留，清理 unused import warning |
| `srt.rs` | **删除** |
| `ai.rs` | 清空死代码，保留模块声明 |
| `src/torrent.rs` | 已删除（git status 显示 D） |
| `sub/` (现有) | 重构，移除 compare，保留 list/extract/shift |
