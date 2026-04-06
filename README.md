# blowup

> [中文版本点此链接](./README_zh.md)

![Version](https://img.shields.io/badge/Version-2.0.0-blue?style=for-the-badge) ![License](https://img.shields.io/badge/License-MIT-darkgreen?style=for-the-badge)

> **Blow-Up [Michelangelo Antonioni, 1966]**: A fashion photographer unknowingly captures a death on film after following two lovers in a park.
>
> The best movie I've seen so far.

---

**blowup** is a desktop application for the Chinese film-watching pipeline: TMDB discovery, torrent search & download, subtitle management, personal film knowledge base, and media playback.

Built with **Tauri v2** (Rust backend) and **React 19** (TypeScript frontend).

### Pages

| Page | Description |
|------|-------------|
| **Search** | Search and discover films via TMDB with filters (year, genre, rating, sort) |
| **People** | Knowledge base: directors, actors, crew — filmographies, wiki, relations |
| **Genres** | Knowledge base: hierarchical genre tree with film/person associations |
| **Graph** | D3 force-directed relationship graph of people and films |
| **My Library** | Local file management — scan directories, link files to films, stats |
| **Downloads** | Download manager — YTS torrent search, aria2c integration, queue tracking |
| **Subtitles** | Subtitle tools — OpenSubtitles fetch, alass alignment, extraction, time shift |
| **Media** | Media probe (ffprobe) and external player launch (mpv / system default) |
| **Settings** | Configuration for API keys, tool paths, music player |

### Building from source

```bash
# Prerequisites: Node.js 20+, Rust 1.80+, platform build tools for Tauri v2

# Install frontend dependencies
npm install

# Development mode
npm run tauri dev

# Production build
npm run tauri build
```

### Runtime dependencies

| Tool | Required for | Install |
|------|-------------|---------|
| `aria2c` | Downloads | `brew install aria2` / `apt install aria2` / `choco install aria2` |
| `alass` / `alass-cli` | Subtitle alignment | `brew install alass` / [GitHub releases](https://github.com/kaegi/alass/releases) |
| `ffmpeg` + `ffprobe` | Subtitle extraction, media probe | `brew install ffmpeg` / `choco install ffmpeg` |
| `mpv` (optional) | Media playback | `brew install mpv` / `choco install mpv` |

### Configuration

Settings are stored at `~/.config/blowup/config.toml`. Configure via the Settings page or:

```bash
# TMDB API key (free at themoviedb.org)
# Set via Settings page → TMDB section
```

| Key | Type | Description |
|-----|------|-------------|
| `tools.aria2c` | path | Path to aria2c binary |
| `tools.alass` | path | Path to alass binary |
| `tools.ffmpeg` | path | Path to ffmpeg binary |
| `tools.player` | path | Path to media player (default: mpv) |
| `tmdb.api_key` | string | TMDB API key |
| `opensubtitles.api_key` | string | OpenSubtitles API key (optional) |
| `subtitle.default_lang` | string | Default subtitle language (default: zh) |
| `library.root_dir` | path | Download/library directory (default: ~/Movies/blowup) |

## License

MIT — see [LICENSE](./LICENSE.txt).
