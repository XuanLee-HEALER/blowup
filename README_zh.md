# blowup

> [Click here for English version](./README.md)

![Version](https://img.shields.io/badge/Version-2.0.1-blue?style=for-the-badge) ![License](https://img.shields.io/badge/License-MIT-darkgreen?style=for-the-badge)

> **《放大》[米开朗基罗·安东尼奥尼，1966]**：一位时装摄影师在公园里跟拍两名恋人时，无意间将一桩谋杀摄入镜头。
>
> 我认为迄今为止最好的电影。

---

**blowup** 是一个面向老电影爱好者的一站式桌面终端 — 集个人影片知识库与完整观影工作流（字幕管理、剪辑辅助）于一体。

通过 TMDB 搜索和发现影片，构建个人影人/影片/流派知识图谱，从 YTS 获取公共领域影片资源，管理字幕，探测和播放媒体 — 一站到位。

基于 **Tauri v2**（Rust 后端）和 **React 19**（TypeScript 前端）构建，工具链全部开源。

**平台支持：** macOS 和 Windows 为第一优先级，Linux 暂时搁置。

### 功能页面

| 页面 | 说明 |
|------|------|
| **搜索** | 通过 TMDB 搜索和发现电影，支持年份、类型、评分、排序过滤 |
| **影人** | 知识库：导演、演员、剧组 — 影片列表、Wiki、人物关系 |
| **流派** | 知识库：层级类型树，关联影片和影人 |
| **关系图** | D3 力导向图，可视化影人与影片的关联网络 |
| **我的库** | 本地文件管理 — 目录扫描、文件与影片关联、统计 |
| **下载** | 下载管理 — YTS 种子搜索、aria2c 集成、队列追踪 |
| **字幕** | 字幕工具 — OpenSubtitles 搜索、alass 对齐、提取、时间偏移 |
| **媒体** | 媒体探测（ffprobe）与外部播放器启动（mpv / 系统默认） |
| **设置** | 配置 API 密钥、工具路径、音乐播放器 |

### 从源码构建

```bash
# 前置要求：Node.js 20+、Rust 1.80+、Tauri v2 平台构建工具

# 安装前端依赖
bun install

# 开发模式
bun run tauri dev

# 生产构建
bun run tauri build
```

### 运行时依赖

| 工具 | 用途 | 安装方式 |
|------|------|----------|
| `aria2c` | 下载 | `brew install aria2` / `apt install aria2` / `choco install aria2` |
| `alass` / `alass-cli` | 字幕对齐 | `brew install alass` / [GitHub releases](https://github.com/kaegi/alass/releases) |
| `ffmpeg` + `ffprobe` | 字幕提取、媒体探测 | `brew install ffmpeg` / `choco install ffmpeg` |
| `mpv`（可选） | 媒体播放 | `brew install mpv` / `choco install mpv` |

### 配置

设置保存在 `~/.config/blowup/config.toml`，可通过设置页面或直接编辑文件：

| Key | 类型 | 说明 |
|-----|------|------|
| `tools.aria2c` | 路径 | aria2c 可执行文件路径 |
| `tools.alass` | 路径 | alass 可执行文件路径 |
| `tools.ffmpeg` | 路径 | ffmpeg 可执行文件路径 |
| `tools.player` | 路径 | 媒体播放器路径（默认：mpv） |
| `tmdb.api_key` | 字符串 | TMDB API Key（在 [themoviedb.org](https://www.themoviedb.org/settings/api) 免费申请） |
| `opensubtitles.api_key` | 字符串 | OpenSubtitles API Key（可选） |
| `subtitle.default_lang` | 字符串 | 默认字幕语言（默认：zh） |
| `library.root_dir` | 路径 | 下载/库目录（默认：~/Movies/blowup） |

## 合规声明

- **影片资源**来自 [YTS/YIFY](https://yts.mx)，其收录的是版权已过期、进入公共领域的老电影。
- **所有集成工具** — aria2c、ffmpeg、alass、mpv — 均为开源项目，遵循各自的开源协议。
- **blowup 不存储、托管或分发**任何受版权保护的内容，它是一个帮助用户组织和观看合法获取影片的客户端工具。
- 用户应遵守所在地区的法律法规。

## 许可协议

MIT — 详见 [LICENSE](./LICENSE.txt)。
