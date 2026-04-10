# blowup

> [Click here for English version](./README.md)

![Version](https://img.shields.io/badge/Version-2.0.5-blue?style=for-the-badge) ![License](https://img.shields.io/badge/License-MIT-darkgreen?style=for-the-badge)

> **《放大》[米开朗基罗·安东尼奥尼，1966]**：一位时装摄影师在公园里跟拍两名恋人时，无意间将一桩谋杀摄入镜头。
>
> 我认为迄今为止最好的电影。

---

**blowup** 是一个面向老电影爱好者的一站式桌面应用 — 集个人影片知识库与完整观影工作流（发现、下载、字幕管理、媒体播放）于一体。

通过 TMDB 搜索和发现影片，构建个人知识图谱（条目 + 标签 + 关系），从 YTS 获取种子资源，管理字幕，内置 mpv 播放器直接播放 — 一站到位。

基于 **Tauri v2**（Rust 后端 + SQLite）和 **React 19**（TypeScript 前端）构建，工具链全部开源。

**平台支持：** macOS（Apple Silicon）、Windows、Linux。

### 安装

**macOS（Homebrew）：**

```bash
brew tap XuanLee-HEALER/tap
brew install --cask blowup
```

**其他平台：** 从 [GitHub Releases](https://github.com/XuanLee-HEALER/blowup/releases/latest) 下载。

## 工作流：从发现到播放

blowup 的典型使用流程：

**1. 发现** — 打开**搜索**页面，输入片名或按类型/年份/评分浏览 TMDB。点击结果查看详情、主创和可用种子。

**2. 下载** — 选择画质和要下载的文件。**下载**页面实时追踪进度。下载完成后影片自动出现在**电影库**中。

**3. 字幕** — 进入**暗房**。通过 OpenSubtitles 或射手网（ASSRT）搜索字幕。内嵌字幕轨在下载完成后自动提取。使用内置对齐工具将字幕与音频同步。

**4. 播放** — 回到**电影库**，选择字幕文件 — 为每条字幕分别设置颜色、字号和垂直位置。支持多字幕同时显示（如底部中文 + 顶部英文）。点击播放，内置 mpv 播放器即刻开始。

**5. 知识** — 在 **Wiki** 中记录你的收获。为导演、影片、流派创建条目，用自定义关系连接它们。**知识图谱**页面将你日益丰富的知识网络可视化呈现。

### 功能页面

| 页面 | 说明 |
|------|------|
| **搜索** | 通过 TMDB 搜索和发现电影，支持年份、类型、评分、排序过滤 |
| **Wiki** | 统一知识库 — 一切概念皆为条目，通过标签和关系自由组织 |
| **知识图谱** | D3 力导向关系图，支持有向边和多条关系展示 |
| **影片** | 电影库 — 按导演分组的树形视图，TMDB 数据自动充实，海报/主创/简介，多字幕叠加 |
| **下载** | 下载管理 — YTS 种子搜索、文件选择、暂停/恢复 |
| **暗房** | 字幕工具 + 媒体探测 — 提取、对齐、时间偏移、音频波形 |
| **设置** | 配置 API 密钥、工具路径、同步、音乐播放器 |

### 架构

数据流采用**事件驱动**模式：后端数据变更后发射领域事件（`downloads:changed`、`library:changed`、`entries:changed`、`config:changed`），前端监听并重新拉取数据 — 无轮询。

两个独立数据系统：
- **知识库**（SQLite）：统一条目模型，标签分类，开放式关系
- **电影库**（文件系统 + JSON 索引）：`{根目录}/{导演}/{tmdb_id}/` 目录结构，内存索引配合 TMDB 惰性充实

### 从源码构建

```bash
# 前置要求：Bun、Rust 1.80+、Tauri v2 平台构建工具
# macOS：brew install mpv（需要 libmpv）
# Linux：sudo apt install libmpv-dev libwebkit2gtk-4.1-dev

bun install
just dev    # 开发模式
just build  # 生产构建
just check  # 检查（lint + 类型检查 + 测试）
```

### 运行时依赖

| 工具 | 用途 | 安装方式 |
|------|------|----------|
| `ffmpeg` + `ffprobe` | 字幕提取、媒体探测、库扫描 | `brew install ffmpeg` / `choco install ffmpeg` |

字幕对齐已内置（无需外部 `alass` 程序）。工具路径在启动时自动探测。

### 配置

设置保存在 `{APP_DATA_DIR}/config.toml`，可通过设置页面配置。

| 键 | 类型 | 说明 |
|-----|------|------|
| `tools.ffmpeg` | 路径 | ffmpeg 可执行文件路径 |
| `tmdb.api_key` | 字符串 | TMDB API Key（在 [themoviedb.org](https://www.themoviedb.org/settings/api) 免费申请） |
| `opensubtitles.api_key` | 字符串 | OpenSubtitles API Key（可选） |
| `assrt.token` | 字符串 | 射手网 ASSRT Token（可选） |
| `subtitle.default_lang` | 字符串 | 默认字幕语言（默认：zh） |
| `library.root_dir` | 路径 | 电影库目录（默认：~/Movies/blowup） |
| `download.max_concurrent` | 数字 | 最大并发下载数（默认：3） |

## 合规声明

- **影片资源**来自 [YTS/YIFY](https://yts.mx)，其收录的是版权已过期、进入公共领域的老电影。
- **所有集成工具** — ffmpeg、mpv — 均为开源项目，遵循各自的开源协议。
- **blowup 不存储、托管或分发**任何受版权保护的内容，它是一个帮助用户组织和观看合法获取影片的客户端工具。
- 用户应遵守所在地区的法律法规。

## 许可协议

MIT — 详见 [LICENSE](./LICENSE.txt)。
